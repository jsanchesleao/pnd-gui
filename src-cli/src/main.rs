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
mod yazi;

use file_browser::{FileBrowser, FileBrowserEvent, FileBrowserTarget};
use yazi::{YaziPick, run_yazi};
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
    pub(crate) vault: pages::vault::VaultState,
    /// Active file browser overlay. While `Some`, all key events are routed
    /// to the browser. Set to `None` when the user selects or cancels.
    pub(crate) file_browser: Option<FileBrowser>,
    /// Pending yazi invocation. When `Some`, the main loop runs yazi before
    /// the next draw and then clears this field.
    pub(crate) yazi_pending: Option<YaziPick>,
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
            vault: pages::vault::VaultState::new(),
            file_browser: None,
            yazi_pending: None,
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
            }
            MenuItem::Preview => {
                self.preview = pages::preview::PreviewState::new();
            }
            MenuItem::Vault => {
                self.vault = pages::vault::VaultState::new();
                // No file browser here — the vault submenu opens first.
            }
        }
        self.screen = Screen::Page(item);
    }

    /// Open a file picker, starting in the directory inferred from `path_hint`.
    /// Uses yazi if available; falls back to the built-in TUI file browser.
    pub(crate) fn open_file_browser(&mut self, path_hint: &str, target: FileBrowserTarget) {
        let start = infer_start_dir(path_hint);
        if yazi::yazi_available() {
            self.yazi_pending = Some(YaziPick { target, start_dir: start, multi: false });
        } else {
            self.file_browser = Some(FileBrowser::open(start.as_deref(), target));
        }
    }

    /// Open a directory picker.
    /// Uses yazi if available; falls back to the built-in TUI file browser.
    pub(crate) fn open_file_browser_dir(&mut self, path_hint: &str, target: FileBrowserTarget) {
        let start = infer_start_dir(path_hint);
        if yazi::yazi_available() {
            self.yazi_pending = Some(YaziPick { target, start_dir: start, multi: false });
        } else {
            self.file_browser = Some(FileBrowser::open_for_dir(start.as_deref(), target));
        }
    }

    /// Open a multi-file picker.
    /// Uses yazi if available; falls back to the built-in TUI file browser.
    pub(crate) fn open_file_browser_multi(&mut self, start: Option<&std::path::Path>, target: FileBrowserTarget) {
        if yazi::yazi_available() {
            self.yazi_pending = Some(YaziPick {
                target,
                start_dir: start.map(|p| p.to_path_buf()),
                multi: true,
            });
        } else {
            self.file_browser = Some(FileBrowser::open_multi(start, target, " Add Files to Vault "));
        }
    }

    /// Always open the built-in TUI file browser, regardless of yazi availability.
    pub(crate) fn open_builtin_browser(&mut self, path_hint: &str, target: FileBrowserTarget) {
        let start = infer_start_dir(path_hint);
        self.file_browser = Some(FileBrowser::open(start.as_deref(), target));
    }

    /// Always open the built-in TUI directory browser, regardless of yazi availability.
    pub(crate) fn open_builtin_browser_dir(&mut self, path_hint: &str, target: FileBrowserTarget) {
        let start = infer_start_dir(path_hint);
        self.file_browser = Some(FileBrowser::open_for_dir(start.as_deref(), target));
    }

    /// Queue a yazi pick (caller must verify `yazi::yazi_available()` first).
    pub(crate) fn open_yazi_picker(&mut self, path_hint: &str, target: FileBrowserTarget) {
        let start = infer_start_dir(path_hint);
        self.yazi_pending = Some(YaziPick { target, start_dir: start, multi: false });
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
        app.vault.poll_progress();
        app.vault.poll_add_progress();
        app.vault.poll_preview_progress();
        app.vault.poll_export_progress();
        app.vault.poll_gallery_progress();
        // Clear vault status messages that have been visible for ≥ 3 seconds.
        app.vault.tick(3);

        // Render image on the main thread if decryption just completed.
        if let pages::preview::PreviewPhase::PendingRender { .. } = app.preview.phase {
            pages::preview::render_preview(&mut app.preview, terminal);
        }
        // Render vault preview if a vault entry was just decrypted.
        if let pages::vault::Phase::PreviewReady { .. } = &app.vault.phase {
            pages::vault::render_vault_preview(&mut app.vault, terminal);
        }
        // Render vault folder gallery if all images are loaded.
        if let pages::vault::Phase::GalleryReady { .. } = &app.vault.phase {
            pages::vault::render_vault_gallery(&mut app.vault, terminal);
        }

        // Run yazi if a pick was requested this iteration.
        if let Some(pick) = app.yazi_pending.take() {
            if let Some(event) = run_yazi(terminal, pick.start_dir.as_deref(), pick.multi) {
                match event {
                    FileBrowserEvent::Selected(path) => {
                        apply_browser_selection(&mut app, pick.target, path);
                    }
                    FileBrowserEvent::MultiSelected(paths) => {
                        apply_browser_multi_selection(&mut app, pick.target, paths);
                    }
                    FileBrowserEvent::Cancelled | FileBrowserEvent::Pending => {}
                }
            }
        }

        // ── Draw ────────────────────────────────────────────────────────────
        terminal.draw(|frame| {
            match app.screen {
                Screen::Menu => draw_menu(frame, &mut app.list_state),
                Screen::Page(MenuItem::EncryptDecrypt) => {
                    pages::enc_dec::draw_enc_dec(frame, &app.enc_dec)
                }
                Screen::Page(MenuItem::Preview) => pages::preview::draw_preview(frame, &app.preview),
                Screen::Page(MenuItem::Vault) => pages::vault::draw_vault(frame, &app.vault),
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
            || matches!(app.preview.phase, pages::preview::PreviewPhase::Decrypting(_))
            || app.vault.is_opening()
            || app.vault.is_adding()
            || app.vault.is_previewing()
            || app.vault.is_exporting()
            || app.vault.is_loading_gallery()
            || app.vault.has_pending_status();
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
                FileBrowserEvent::MultiSelected(paths) => {
                    app.file_browser = None;
                    apply_browser_multi_selection(&mut app, target, paths);
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
        FileBrowserTarget::VaultDir => {
            app.vault.set_path(path.to_string_lossy().as_ref());
        }
        FileBrowserTarget::VaultCreateDir => {
            app.vault.set_create_path(path.to_string_lossy().as_ref());
        }
        FileBrowserTarget::VaultAddFiles => {
            // Single file selected without using Space — treat as a one-file add.
            app.vault.start_add(vec![path]);
        }
        FileBrowserTarget::VaultExportDir => {
            app.vault.start_export(path);
        }
    }
}

/// Route a multi-file browser selection to the correct handler.
fn apply_browser_multi_selection(app: &mut App, target: FileBrowserTarget, paths: Vec<PathBuf>) {
    match target {
        FileBrowserTarget::VaultAddFiles => {
            app.vault.start_add(paths);
        }
        // Other targets don't support multi-select; ignore.
        _ => {}
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
