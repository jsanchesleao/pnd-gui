use clap::Parser;
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
use std::{io, path::PathBuf, process, time::Duration};

mod cli;
mod crypto;
mod enc_dec_cli;
mod file_browser;
mod pages;
mod password;
mod preview_cli;
mod vault_add_cli;
mod vault_list_cli;
mod vault_op_cli;
mod vault_rmd_cli;
mod yazi;

use file_browser::{FileBrowser, FileBrowserEvent, FileBrowserTarget};
use password::read_password;
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

// ── TuiPreload ─────────────────────────────────────────────────────────────

/// Pre-loaded state passed from the CLI into the TUI event loop.
enum TuiPreload {
    /// Open the Encrypt/Decrypt page with this file path already filled in.
    EncDec(String),
    /// Open the Preview page with this file path already filled in.
    Preview(String),
    /// Open the Vault page and immediately start unlocking with the given credentials.
    Vault { vault_path: String, password: String },
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
    /// Set to `true` when the TUI was launched directly into the vault via
    /// `--vault`. "Back" actions that would normally return to the main menu
    /// quit the TUI instead.
    pub(crate) direct_vault_launch: bool,
    /// Set to `true` to signal the event loop to exit cleanly.
    pub(crate) quit: bool,
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
            direct_vault_launch: false,
            quit: false,
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
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            let code = match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 3,
            };
            process::exit(code);
        }
    };

    // ── Phase 3: --tui flag — launch TUI with optional preload ───────────
    if cli.tui {
        if cli.output.is_some() {
            eprintln!("warning: -o is ignored when --tui is given");
        }
        let preload = cli.files.first().map(|f| {
            let path = f.to_string_lossy().into_owned();
            if cli.preview_mode {
                TuiPreload::Preview(path)
            } else {
                TuiPreload::EncDec(path)
            }
        });
        return run_tui(preload);
    }

    // Zero args → launch TUI (no preload).
    if cli.is_tui_mode() {
        return run_tui(None);
    }

    // ── Dispatch non-interactive modes ────────────────────────────────────
    let has_vault_cmd = cli.vault.is_some()
        || cli.vault_list.is_some()
        || cli.vault_preview.is_some()
        || cli.vault_export.is_some()
        || !cli.vault_add.is_empty();

    if !cli.files.is_empty() && !cli.preview_mode && !has_vault_cmd {
        // Phase 2: single-file encrypt / decrypt (non-interactive)
        enc_dec_cli::run(&cli);
    }

    if cli.preview_mode {
        // Phase 4: non-interactive preview
        preview_cli::run(&cli);
    }

    // ── Phase 5: --vault — open vault in TUI ─────────────────────────────
    if let Some(vault_dir) = &cli.vault {
        if !vault_dir.exists() {
            eprintln!("error: directory not found: {}", vault_dir.display());
            process::exit(2);
        }
        if !vault_dir.is_dir() {
            eprintln!("error: {} is not a directory", vault_dir.display());
            process::exit(3);
        }
        if !vault_dir.join("index.lock").exists() {
            eprintln!("error: no vault found at {}", vault_dir.display());
            process::exit(2);
        }

        let password = read_password();
        let vault_path = vault_dir.to_string_lossy().into_owned();
        return run_tui(Some(TuiPreload::Vault { vault_path, password }));
    }

    // ── Phase 6: --vault-list ─────────────────────────────────────────────
    if cli.vault_list.is_some() {
        vault_list_cli::run(&cli);
    }

    // ── Phase 7: --vault-preview and --vault-export ───────────────────────
    if cli.vault_preview.is_some() {
        vault_op_cli::run_preview(&cli);
    }

    if cli.vault_export.is_some() {
        vault_op_cli::run_export(&cli);
    }

    // ── Phase 8: --vault-add ──────────────────────────────────────────────
    if !cli.vault_add.is_empty() {
        vault_add_cli::run_add(&cli);
    }

    // ── Phase 9: --vault-rename, --vault-move, --vault-delete ────────────
    if !cli.vault_rename.is_empty() {
        vault_rmd_cli::run_rename(&cli);
    }

    if !cli.vault_move.is_empty() {
        vault_rmd_cli::run_move(&cli);
    }

    if !cli.vault_delete.is_empty() {
        vault_rmd_cli::run_delete(&cli);
    }

    // Remaining modes are implemented in later phases.
    eprintln!("error: this mode is not yet implemented");
    process::exit(3);
}

fn run_tui(preload: Option<TuiPreload>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;
    let result = run(&mut terminal, preload);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, preload: Option<TuiPreload>) -> io::Result<()> {
    let mut app = App::new();

    // Apply any pre-loaded state from CLI arguments (Phase 3).
    if let Some(pre) = preload {
        match pre {
            TuiPreload::EncDec(path) => {
                app.enc_dec.path = path;
                app.enc_dec.focus = 1; // advance to password field
                app.screen = Screen::Page(MenuItem::EncryptDecrypt);
            }
            TuiPreload::Preview(path) => {
                let is_encrypted = path.ends_with(".lock");
                app.preview.path = path;
                app.screen = Screen::Page(MenuItem::Preview);
                if !is_encrypted {
                    // Plain file — no password needed, start preview immediately.
                    app.preview.start();
                } else {
                    // Encrypted file — advance focus to the password field and wait.
                    app.preview.focus = 1;
                }
            }
            TuiPreload::Vault { vault_path, password } => {
                app.vault.phase = pages::vault::Phase::Locked {
                    vault_path,
                    password,
                    focus: 0,
                    path_edit_mode: false,
                    error: None,
                };
                app.vault.start_unlock();
                app.screen = Screen::Page(MenuItem::Vault);
                app.direct_vault_launch = true;
            }
        }
    }

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

        if app.quit {
            return Ok(());
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
