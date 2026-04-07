use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};
use std::io;

// ── State ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum MenuItem {
    EncryptDecrypt,
    Preview,
    Vault,
}

impl MenuItem {
    const ALL: &'static [MenuItem] = &[
        MenuItem::EncryptDecrypt,
        MenuItem::Preview,
        MenuItem::Vault,
    ];

    fn label(self) -> &'static str {
        match self {
            MenuItem::EncryptDecrypt => "Encrypt / Decrypt",
            MenuItem::Preview => "Preview",
            MenuItem::Vault => "Vault",
        }
    }
}

#[derive(PartialEq)]
enum Screen {
    Menu,
    Page(MenuItem),
}

struct App {
    screen: Screen,
    list_state: ListState,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App { screen: Screen::Menu, list_state }
    }

    fn selected_item(&self) -> MenuItem {
        MenuItem::ALL[self.list_state.selected().unwrap_or(0)]
    }

    fn move_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if i == 0 { MenuItem::ALL.len() - 1 } else { i - 1 }));
    }

    fn move_down(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % MenuItem::ALL.len()));
    }

    fn enter(&mut self) {
        self.screen = Screen::Page(self.selected_item());
    }

    fn back(&mut self) {
        self.screen = Screen::Menu;
    }
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| {
            match &app.screen {
                Screen::Menu => draw_menu(frame, &mut app.list_state),
                Screen::Page(item) => draw_page(frame, *item),
            }
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match &app.screen {
                Screen::Menu => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Enter => app.enter(),
                    _ => {}
                },
                Screen::Page(_) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => app.back(),
                    _ => {}
                },
            }
        }
    }
}

// ── Drawing ────────────────────────────────────────────────────────────────

const ACCENT: Color = Color::Rgb(130, 100, 220);
const DIM: Color = Color::Rgb(90, 90, 110);

fn draw_menu(frame: &mut ratatui::Frame, list_state: &mut ListState) {
    let area = frame.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " pnd-cli ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    frame.render_widget(outer, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("pnd", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" — password & note depot", Style::default().fg(Color::White)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(title, inner[0]);

    let items: Vec<ListItem> = MenuItem::ALL
        .iter()
        .map(|m| ListItem::new(format!("  {}  ", m.label())))
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, inner[2], list_state);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("↑↓ / jk", Style::default().fg(DIM)),
        Span::styled("  navigate    ", Style::default().fg(DIM)),
        Span::styled("Enter", Style::default().fg(DIM)),
        Span::styled("  select    ", Style::default().fg(DIM)),
        Span::styled("q", Style::default().fg(DIM)),
        Span::styled("  quit", Style::default().fg(DIM)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(hint, inner[3]);
}

fn draw_page(frame: &mut ratatui::Frame, item: MenuItem) {
    let area = frame.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" {} ", item.label()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    frame.render_widget(block, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body = Paragraph::new(Line::from(Span::styled(
        "coming soon",
        Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(body, inner[0]);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Esc / Backspace / q", Style::default().fg(DIM)),
        Span::styled("  back", Style::default().fg(DIM)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(hint, inner[1]);
}
