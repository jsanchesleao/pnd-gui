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

mod crypto;

// ── Palette ────────────────────────────────────────────────────────────────

const ACCENT: Color = Color::Rgb(130, 100, 220);
const DIM: Color = Color::Rgb(90, 90, 110);
const SUCCESS: Color = Color::Rgb(80, 200, 80);
const FAILURE: Color = Color::Rgb(220, 80, 80);

// ── MenuItem ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum MenuItem {
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
enum Screen {
    Menu,
    Page(MenuItem),
}

// ── Encrypt/Decrypt page ───────────────────────────────────────────────────

enum OpStatus {
    Idle,
    Success(String),
    Failure(String),
}

/// Focus positions: 0 = path field, 1 = password field, 2 = Execute button.
struct EncDecState {
    path: String,
    password: String,
    focus: usize,
    status: OpStatus,
}

impl EncDecState {
    fn new() -> Self {
        Self {
            path: String::new(),
            password: String::new(),
            focus: 0,
            status: OpStatus::Idle,
        }
    }

    fn is_decrypt(&self) -> bool {
        self.path.trim_end().ends_with(".lock")
    }

    fn advance_focus(&mut self) {
        self.focus = (self.focus + 1) % 3;
    }

    fn retreat_focus(&mut self) {
        self.focus = (self.focus + 2) % 3;
    }

    fn run(&mut self) {
        let path = self.path.trim().to_string();
        let password = self.password.clone();

        if path.is_empty() {
            self.status = OpStatus::Failure("File path cannot be empty.".into());
            return;
        }
        if password.is_empty() {
            self.status = OpStatus::Failure("Password cannot be empty.".into());
            return;
        }

        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                self.status = OpStatus::Failure(format!("Cannot read file: {e}"));
                return;
            }
        };

        if self.is_decrypt() {
            match crypto::decrypt_file(&data, &password) {
                Some(plain) => {
                    let out = path.strip_suffix(".lock").unwrap().to_string();
                    match std::fs::write(&out, &plain) {
                        Ok(()) => self.status = OpStatus::Success(format!("Saved → {out}")),
                        Err(e) => self.status = OpStatus::Failure(format!("Write error: {e}")),
                    }
                }
                None => {
                    self.status = OpStatus::Failure(
                        "Decryption failed — wrong password or corrupted file.".into(),
                    );
                }
            }
        } else {
            let out = format!("{path}.lock");
            let encrypted = crypto::encrypt_file(&data, &password);
            match std::fs::write(&out, &encrypted) {
                Ok(()) => self.status = OpStatus::Success(format!("Saved → {out}")),
                Err(e) => self.status = OpStatus::Failure(format!("Write error: {e}")),
            }
        }
    }
}

// ── App ────────────────────────────────────────────────────────────────────

struct App {
    screen: Screen,
    list_state: ListState,
    enc_dec: EncDecState,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App { screen: Screen::Menu, list_state, enc_dec: EncDecState::new() }
    }

    fn selected_item(&self) -> MenuItem {
        MenuItem::ALL[self.list_state.selected().unwrap_or(0)]
    }

    fn move_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some(if i == 0 { MenuItem::ALL.len() - 1 } else { i - 1 }));
    }

    fn move_down(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % MenuItem::ALL.len()));
    }

    fn enter_page(&mut self) {
        let item = self.selected_item();
        if item == MenuItem::EncryptDecrypt {
            self.enc_dec = EncDecState::new();
        }
        self.screen = Screen::Page(item);
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
        terminal.draw(|frame| match app.screen {
            Screen::Menu => draw_menu(frame, &mut app.list_state),
            Screen::Page(MenuItem::EncryptDecrypt) => draw_enc_dec(frame, &app.enc_dec),
            Screen::Page(item) => draw_coming_soon(frame, item),
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.screen {
                Screen::Menu => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Enter => app.enter_page(),
                    _ => {}
                },
                Screen::Page(MenuItem::EncryptDecrypt) => handle_enc_dec(&mut app, key.code),
                Screen::Page(_) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => app.back(),
                    _ => {}
                },
            }
        }
    }
}

/// Handle a keypress on the Encrypt/Decrypt page.
///
/// Esc and 'q' (when button is focused) are handled first to avoid conflicts
/// with typing those characters into the text fields.
fn handle_enc_dec(app: &mut App, code: KeyCode) {
    // Keys that navigate away — must be checked before borrowing enc_dec.
    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; return; }
        // 'q' is only a back-key when the button is focused (not in a text field).
        KeyCode::Char('q') if app.enc_dec.focus == 2 => { app.screen = Screen::Menu; return; }
        _ => {}
    }

    let s = &mut app.enc_dec;
    match code {
        KeyCode::Tab => s.advance_focus(),
        KeyCode::BackTab => s.retreat_focus(),
        KeyCode::Enter => {
            if s.focus == 2 {
                s.run();
            } else {
                s.advance_focus();
            }
        }
        KeyCode::Char(c) if s.focus < 2 => {
            if s.focus == 0 {
                s.path.push(c);
                s.status = OpStatus::Idle; // clear stale result when path changes
            } else {
                s.password.push(c);
            }
        }
        KeyCode::Backspace if s.focus < 2 => {
            if s.focus == 0 {
                s.path.pop();
                s.status = OpStatus::Idle;
            } else {
                s.password.pop();
            }
        }
        _ => {}
    }
}

// ── Drawing helpers ────────────────────────────────────────────────────────

fn outer_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
}

fn input_block(label: &'static str, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(
            format!(" {label} "),
            Style::default().fg(if focused { ACCENT } else { DIM }),
        ))
}

/// Trim the displayed string so the tail (cursor end) always fits within
/// `available_cols`, keeping the last-typed characters visible.
fn tail_fit(s: &str, available_cols: usize) -> &str {
    if s.len() <= available_cols {
        return s;
    }
    // byte-level trim from the left (ASCII paths are safe; Unicode paths clip gracefully)
    &s[s.len() - available_cols..]
}

// ── draw_menu ──────────────────────────────────────────────────────────────

fn draw_menu(frame: &mut ratatui::Frame, list_state: &mut ListState) {
    let area = frame.area();
    frame.render_widget(outer_block("pnd-cli"), area);

    let chunks = Layout::default()
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
        chunks[0],
    );

    let items: Vec<ListItem> =
        MenuItem::ALL.iter().map(|m| ListItem::new(format!("  {}  ", m.label()))).collect();

    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ "),
        chunks[2],
        list_state,
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑↓ / jk  navigate    ", Style::default().fg(DIM)),
            Span::styled("Enter  select    ", Style::default().fg(DIM)),
            Span::styled("q  quit", Style::default().fg(DIM)),
        ]))
        .alignment(Alignment::Center),
        chunks[3],
    );
}

// ── draw_enc_dec ───────────────────────────────────────────────────────────

fn draw_enc_dec(frame: &mut ratatui::Frame, state: &EncDecState) {
    let area = frame.area();
    frame.render_widget(outer_block("Encrypt / Decrypt"), area);

    // 13-slot vertical layout inside the outer border
    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0]  mode label
            Constraint::Length(1), // [1]  blank
            Constraint::Length(3), // [2]  path input  (border+content+border)
            Constraint::Length(1), // [3]  blank
            Constraint::Length(3), // [4]  password input
            Constraint::Length(1), // [5]  blank
            Constraint::Length(1), // [6]  Execute button
            Constraint::Length(1), // [7]  blank
            Constraint::Length(1), // [8]  status
            Constraint::Min(0),    // [9]  filler
            Constraint::Length(1), // [10] hint
        ])
        .split(area);

    // [0] mode
    let (mode_label, mode_color) = if state.is_decrypt() {
        ("Decrypt", Color::Cyan)
    } else {
        ("Encrypt", Color::LightYellow)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Mode: ", Style::default().fg(DIM)),
            Span::styled(mode_label, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
            Span::styled(
                if state.is_decrypt() {
                    "   (auto-detected: .lock suffix)"
                } else {
                    "   (auto-detected: no .lock suffix)"
                },
                Style::default().fg(DIM),
            ),
        ])),
        c[0],
    );

    // [2] path input
    let inner_width = c[2].width.saturating_sub(4) as usize; // -2 borders -2 padding
    let path_display = {
        let s = if state.focus == 0 {
            format!("{}|", state.path)
        } else {
            state.path.clone()
        };
        tail_fit(&s, inner_width).to_string()
    };
    frame.render_widget(
        Paragraph::new(path_display.as_str()).block(input_block("File path", state.focus == 0)),
        c[2],
    );

    // [4] password input (masked)
    let masked = "•".repeat(state.password.len());
    let pass_display = if state.focus == 1 { format!("{masked}|") } else { masked };
    frame.render_widget(
        Paragraph::new(pass_display.as_str()).block(input_block("Password", state.focus == 1)),
        c[4],
    );

    // [6] Execute button
    let btn_style = if state.focus == 2 {
        Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("[ Execute ]", btn_style)))
            .alignment(Alignment::Center),
        c[6],
    );

    // [8] status
    let status_line = match &state.status {
        OpStatus::Idle => Line::from(""),
        OpStatus::Success(msg) => Line::from(Span::styled(
            format!("✓  {msg}"),
            Style::default().fg(SUCCESS),
        )),
        OpStatus::Failure(msg) => Line::from(Span::styled(
            format!("✗  {msg}"),
            Style::default().fg(FAILURE),
        )),
    };
    frame.render_widget(Paragraph::new(status_line), c[8]);

    // [10] hint
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Esc  back    ", Style::default().fg(DIM)),
            Span::styled("Tab / Shift+Tab  cycle fields    ", Style::default().fg(DIM)),
            Span::styled("Enter  advance / run", Style::default().fg(DIM)),
        ]))
        .alignment(Alignment::Center),
        c[10],
    );
}

// ── draw_coming_soon ───────────────────────────────────────────────────────

fn draw_coming_soon(frame: &mut ratatui::Frame, item: MenuItem) {
    let area = frame.area();
    frame.render_widget(outer_block(item.label()), area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "coming soon",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center),
        inner[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Esc / Backspace / q  back",
            Style::default().fg(DIM),
        )))
        .alignment(Alignment::Center),
        inner[1],
    );
}
