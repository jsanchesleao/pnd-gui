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
use std::{io, path::PathBuf, time::Duration};

mod crypto;
mod file_browser;
mod pages;

use file_browser::{FileBrowser, FileBrowserEvent, FileBrowserTarget};
use pages::enc_dec::OpStatus;

// ── Palette ────────────────────────────────────────────────────────────────

pub(crate) const ACCENT: Color = Color::Rgb(130, 100, 220);
pub(crate) const DIM: Color = Color::Rgb(90, 90, 110);
pub(crate) const SUCCESS: Color = Color::Rgb(80, 200, 80);
pub(crate) const FAILURE: Color = Color::Rgb(220, 80, 80);

// ── MenuItem ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MenuItem {
    EncryptDecrypt,
    Preview,
    Vault,
}

impl MenuItem {
    const ALL: &'static [MenuItem] =
        &[MenuItem::EncryptDecrypt, MenuItem::Preview, MenuItem::Vault];

    fn label(self) -> &'static str {
        match self {
            MenuItem::EncryptDecrypt => "Encrypt / Decrypt",
            MenuItem::Preview => "Preview",
            MenuItem::Vault => "Vault",
        }
    }
}

// ── Screen ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Screen {
    Menu,
    Page(MenuItem),
}

// ── App ────────────────────────────────────────────────────────────────────

pub(crate) struct App {
    pub(crate) screen: Screen,
    pub(crate) list_state: ListState,
    pub(crate) enc_dec: pages::enc_dec::EncDecState,
    pub(crate) preview: pages::preview::PreviewState,
    /// Active file browser overlay. While `Some`, all key events are routed
    /// to the browser. Set to `None` when the user selects or cancels.
    pub(crate) file_browser: Option<FileBrowser>,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App {
            screen: Screen::Menu,
            list_state,
            enc_dec: pages::enc_dec::EncDecState::new(),
            preview: pages::preview::PreviewState::new(),
            file_browser: None,
        }
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

    fn enter_page(&mut self) {
        let item = self.selected_item();
        match item {
            MenuItem::EncryptDecrypt => {
                self.enc_dec = pages::enc_dec::EncDecState::new();
                self.file_browser = Some(FileBrowser::open(None, FileBrowserTarget::EncDecPath));
            }
            MenuItem::Preview => {
                self.preview = pages::preview::PreviewState::new();
                self.file_browser = Some(FileBrowser::open(None, FileBrowserTarget::PreviewPath));
            }
            _ => {}
        }
        self.screen = Screen::Page(item);
    }

    pub(crate) fn back(&mut self) {
        self.screen = Screen::Menu;
    }

    /// Open the file browser, starting in the directory inferred from `path_hint`.
    pub(crate) fn open_file_browser(&mut self, path_hint: &str, target: FileBrowserTarget) {
        let start = infer_start_dir(path_hint);
        self.file_browser = Some(FileBrowser::open(start.as_deref(), target));
    }
}

/// Given a partially typed path, infer which directory to start the browser in.
fn infer_start_dir(hint: &str) -> Option<PathBuf> {
    let hint = hint.trim();
    if hint.is_empty() { return None; }
    let p = PathBuf::from(hint);
    if p.is_dir() { Some(p) } else { p.parent().filter(|pp| pp.exists()).map(|pp| pp.to_path_buf()) }
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;
    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        // Drain progress messages from the background workers before each draw.
        app.enc_dec.poll_progress();
        app.preview.poll_progress();

        // Render image on the main thread if decryption just completed.
        if let pages::preview::PreviewPhase::PendingRender { .. } = app.preview.phase {
            pages::preview::render_preview(&mut app.preview, terminal);
        }

        // ── Draw ────────────────────────────────────────────────────────────
        terminal.draw(|frame| {
            match app.screen {
                Screen::Menu => draw_menu(frame, &mut app.list_state),
                Screen::Page(MenuItem::EncryptDecrypt) => {
                    pages::enc_dec::draw_enc_dec(frame, &app.enc_dec)
                }
                Screen::Page(MenuItem::Preview) => pages::preview::draw_preview(frame, &app.preview),
                Screen::Page(MenuItem::Vault) => pages::vault::draw_vault(frame),
            }
            // File browser draws on top when open (full-screen overlay)
            if let Some(ref mut fb) = app.file_browser {
                fb.draw(frame);
            }
        })?;

        // ── Events ──────────────────────────────────────────────────────────
        // While an operation is running, poll with a short timeout so the
        // progress bar keeps updating even without user input.
        let running = matches!(app.enc_dec.status, OpStatus::Running(_))
            || matches!(app.preview.phase, pages::preview::PreviewPhase::Decrypting(_));
        let has_event = if running {
            event::poll(Duration::from_millis(50))?
        } else {
            true // block until an event arrives
        };

        if !has_event {
            continue; // timeout — loop back to poll progress + redraw
        }

        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press { continue; }

        // File browser intercepts all keys while open.
        if app.file_browser.is_some() {
            let event = app.file_browser.as_mut().unwrap().handle_key(key.code);
            let target = app.file_browser.as_ref().unwrap().target;
            match event {
                FileBrowserEvent::Selected(path) => {
                    app.file_browser = None;
                    apply_browser_selection(&mut app, target, path);
                }
                FileBrowserEvent::Cancelled => { app.file_browser = None; }
                FileBrowserEvent::Pending => {}
            }
            continue;
        }

        // Normal page routing.
        match app.screen {
            Screen::Menu => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Enter | KeyCode::Char('l') => app.enter_page(),
                _ => {}
            },
            Screen::Page(MenuItem::EncryptDecrypt) => {
                pages::enc_dec::handle_enc_dec(&mut app, key.code)
            }
            Screen::Page(MenuItem::Preview) => {
                pages::preview::handle_preview(&mut app, key.code)
            }
            Screen::Page(MenuItem::Vault) => {
                pages::vault::handle_vault(&mut app, key.code)
            }
        }
    }
}

/// Route a browser selection to the correct field based on what opened the browser.
fn apply_browser_selection(app: &mut App, target: FileBrowserTarget, path: PathBuf) {
    match target {
        FileBrowserTarget::EncDecPath => {
            app.enc_dec.path = path.to_string_lossy().into_owned();
            app.enc_dec.status = OpStatus::Idle;
            app.enc_dec.focus = 1; // advance to password field
        }
        FileBrowserTarget::PreviewPath => {
            app.preview.path = path.to_string_lossy().into_owned();
            app.preview.focus = 1; // advance to password field
        }
    }
}

// ── draw_menu ──────────────────────────────────────────────────────────────

fn draw_menu(frame: &mut ratatui::Frame, list_state: &mut ListState) {
    let area = frame.area();
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(Span::styled(
                " pnd-cli ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center),
        area,
    );

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(2), // title
            Constraint::Length(1), // spacer
            Constraint::Min(3),    // list
            Constraint::Length(1), // hint
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("pnd", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(" — password & note depot", Style::default().fg(Color::White)),
        ]))
        .alignment(Alignment::Center),
        c[0],
    );

    let items: Vec<ListItem> =
        MenuItem::ALL.iter().map(|m| ListItem::new(format!("  {}  ", m.label()))).collect();

    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ "),
        c[2],
        list_state,
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑↓ / jk  navigate    ", Style::default().fg(DIM)),
            Span::styled("Enter  select    ", Style::default().fg(DIM)),
            Span::styled("q  quit", Style::default().fg(DIM)),
        ]))
        .alignment(Alignment::Center),
        c[3],
    );
}
