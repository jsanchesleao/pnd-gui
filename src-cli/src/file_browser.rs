//! Reusable full-screen file-browser overlay.
//!
//! # Usage
//!
//! ```ignore
//! // Open the browser (call once, store in App)
//! app.file_browser = Some(FileBrowser::open(start_dir, FileBrowserTarget::EncDecPath));
//!
//! // In the draw loop — render after the background page
//! if let Some(ref mut fb) = app.file_browser { fb.draw(frame); }
//!
//! // In the event loop — consume key events while open
//! if let Some(ref mut fb) = app.file_browser {
//!     match fb.handle_key(code) {
//!         FileBrowserEvent::Selected(path) => { /* use path */ app.file_browser = None; }
//!         FileBrowserEvent::Cancelled      => { app.file_browser = None; }
//!         FileBrowserEvent::Pending        => {}
//!     }
//! }
//! ```
//!
//! The browser covers the full terminal area, hiding whatever is underneath.
//! It does not depend on any page-specific state.

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── Palette (local copy so this module is self-contained) ──────────────────

const ACCENT: Color = Color::Rgb(130, 100, 220);
const DIM: Color = Color::Rgb(90, 90, 110);
const DIR_COLOR: Color = Color::Cyan;

// ── Public types ───────────────────────────────────────────────────────────

/// What the caller receives when the user makes a choice.
pub enum FileBrowserEvent {
    /// User confirmed a file selection. The path is guaranteed to exist.
    Selected(PathBuf),
    /// User pressed Esc — no selection was made.
    Cancelled,
    /// Still navigating; caller should redraw and keep listening.
    Pending,
}

/// Identifies which part of the app opened the browser so the result can be
/// routed to the right field when the user selects a file.
#[derive(Clone, Copy)]
pub enum FileBrowserTarget {
    /// Fill the File path field in the Encrypt/Decrypt page.
    EncDecPath,
    /// Fill the File path field in the Preview page.
    PreviewPath,
    /// Pick a vault root directory (fires Selected on Enter for directories too).
    VaultDir,
    /// Pick the root directory for a new vault being created.
    VaultCreateDir,
}

// ── Internal entry ─────────────────────────────────────────────────────────

struct Entry {
    /// Display name (bare filename or "..")
    name: String,
    /// Absolute path
    path: PathBuf,
    is_dir: bool,
}

// ── FileBrowser ────────────────────────────────────────────────────────────

pub struct FileBrowser {
    pub target: FileBrowserTarget,
    cwd: PathBuf,
    entries: Vec<Entry>,
    list_state: ListState,
    /// Non-fatal error shown in place of the list (e.g. permission denied)
    load_error: Option<String>,
    /// When true, pressing Enter on a directory fires Selected instead of navigating.
    select_dirs: bool,
}

impl FileBrowser {
    /// Create and immediately load `start_dir` (falls back to `std::env::current_dir`).
    /// Pressing Enter on a file fires `Selected`; directories are navigated into.
    pub fn open(start_dir: Option<&Path>, target: FileBrowserTarget) -> Self {
        Self::new_inner(start_dir, target, false)
    }

    /// Like `open`, but pressing Enter on a **directory** fires `Selected` instead
    /// of navigating into it. Used for vault folder selection.
    pub fn open_for_dir(start_dir: Option<&Path>, target: FileBrowserTarget) -> Self {
        Self::new_inner(start_dir, target, true)
    }

    fn new_inner(start_dir: Option<&Path>, target: FileBrowserTarget, select_dirs: bool) -> Self {
        let cwd = start_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let mut fb = Self {
            target,
            cwd,
            entries: Vec::new(),
            list_state: ListState::default(),
            load_error: None,
            select_dirs,
        };
        fb.load();
        fb
    }

    // ── Navigation ──────────────────────────────────────────────────────────

    /// Process a key event and return what the caller should do next.
    pub fn handle_key(&mut self, code: KeyCode) -> FileBrowserEvent {
        match code {
            // ── Cancel ──────────────────────────────────────────────────────
            KeyCode::Esc => return FileBrowserEvent::Cancelled,

            // ── List movement ───────────────────────────────────────────────
            KeyCode::Up | KeyCode::Char('k') => self.move_by(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(1),
            KeyCode::Char('g') => self.jump_top(),
            KeyCode::Char('G') => self.jump_bottom(),
            KeyCode::PageUp => self.move_by(-10),
            KeyCode::PageDown => self.move_by(10),

            // ── Go up to parent ─────────────────────────────────────────────
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => self.go_up(),

            // ── Enter / select ──────────────────────────────────────────────
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(entry) = self.entries.get(idx) {
                        if entry.is_dir {
                            // In dir-selection mode, Enter on a real dir fires Selected
                            // (but ".." always navigates up, never selects)
                            if self.select_dirs && entry.name != ".." {
                                return FileBrowserEvent::Selected(entry.path.clone());
                            }
                            let path = entry.path.clone();
                            self.navigate_into(path);
                        } else {
                            return FileBrowserEvent::Selected(self.entries[idx].path.clone());
                        }
                    }
                }
            }

            _ => {}
        }
        FileBrowserEvent::Pending
    }

    fn move_by(&mut self, delta: i32) {
        let n = self.entries.len();
        if n == 0 { return; }
        let i = self.list_state.selected().unwrap_or(0) as i32;
        let new = (i + delta).clamp(0, n as i32 - 1) as usize;
        self.list_state.select(Some(new));
    }

    fn jump_top(&mut self) {
        if !self.entries.is_empty() { self.list_state.select(Some(0)); }
    }

    fn jump_bottom(&mut self) {
        let n = self.entries.len();
        if n > 0 { self.list_state.select(Some(n - 1)); }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.cwd.parent().map(|p| p.to_path_buf()) {
            self.cwd = parent;
            self.load();
        }
    }

    fn navigate_into(&mut self, path: PathBuf) {
        self.cwd = path;
        self.load();
    }

    // ── Directory loading ───────────────────────────────────────────────────

    fn load(&mut self) {
        self.entries.clear();
        self.load_error = None;

        // Parent shortcut — absent only at filesystem root
        if let Some(parent) = self.cwd.parent() {
            self.entries.push(Entry {
                name: "..".into(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }

        match fs::read_dir(&self.cwd) {
            Err(e) => {
                self.load_error = Some(format!("Cannot read directory: {e}"));
                // Keep the ".." entry so the user can escape.
            }
            Ok(rd) => {
                let mut dirs: Vec<Entry> = Vec::new();
                let mut files: Vec<Entry> = Vec::new();

                for item in rd.flatten() {
                    let path = item.path();
                    let name = item.file_name().to_string_lossy().into_owned();
                    // Skip hidden entries (dot-files) unless explicitly desired
                    // (keeping them visible for now since pnd files have no dots)
                    let is_dir = path.is_dir();
                    let entry = Entry { name, path, is_dir };
                    if is_dir { dirs.push(entry); } else { files.push(entry); }
                }

                // Sort: directories first, then files; each group alphabetically (case-insensitive)
                dirs.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
                files.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));

                self.entries.extend(dirs);
                self.entries.extend(files);
            }
        }

        // Always reset selection to the first item after loading
        self.list_state.select(if self.entries.is_empty() { None } else { Some(0) });
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Outer block covers the whole terminal
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(Span::styled(
                " File Browser ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center);

        // Clear every cell in the overlay area first so no background page text bleeds through.
        frame.render_widget(Clear, area);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.draw_inner(frame, inner);
    }

    fn draw_inner(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // [0] current path
                Constraint::Length(1), // [1] column headers
                Constraint::Min(1),    // [2] entry list
                Constraint::Length(1), // [3] blank
                Constraint::Length(1), // [4] hint
            ])
            .split(area);

        // [0] Current working directory
        let cwd_str = self.cwd.to_string_lossy();
        frame.render_widget(
            Paragraph::new(Span::styled(
                cwd_str.as_ref(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            chunks[0],
        );

        // [1] Column header
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  type   ", Style::default().fg(DIM)),
                Span::styled("name", Style::default().fg(DIM).add_modifier(Modifier::UNDERLINED)),
            ])),
            chunks[1],
        );

        // [2] Entry list or error message
        if let Some(ref err) = self.load_error.clone() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    err.as_str(),
                    Style::default().fg(Color::Rgb(220, 80, 80)),
                )),
                chunks[2],
            );
        } else if self.entries.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "(empty directory)",
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )),
                chunks[2],
            );
        } else {
            let items: Vec<ListItem> = self.entries.iter().map(|e| {
                if e.name == ".." {
                    ListItem::new(Line::from(vec![
                        Span::styled("  [..]   ", Style::default().fg(DIR_COLOR)),
                        Span::styled("parent directory", Style::default().fg(DIM)),
                    ]))
                } else if e.is_dir {
                    ListItem::new(Line::from(vec![
                        Span::styled("  [/]    ", Style::default().fg(DIM)),
                        Span::styled(e.name.clone(), Style::default().fg(DIR_COLOR)),
                        Span::styled("/", Style::default().fg(DIM)),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled("         ", Style::default().fg(DIM)),
                        Span::styled(e.name.clone(), Style::default().fg(Color::White)),
                    ]))
                }
            }).collect();

            frame.render_stateful_widget(
                List::new(items)
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ "),
                chunks[2],
                &mut self.list_state,
            );
        }

        // [4] Key hint bar
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("↑↓ jk  move    ", Style::default().fg(DIM)),
                Span::styled("PgUp/Dn  scroll    ", Style::default().fg(DIM)),
                Span::styled("Enter l  open    ", Style::default().fg(DIM)),
                Span::styled("Bksp h ←  parent    ", Style::default().fg(DIM)),
                Span::styled("g G  top/bottom    ", Style::default().fg(DIM)),
                Span::styled("Esc  cancel", Style::default().fg(DIM)),
            ]))
            .alignment(Alignment::Center),
            chunks[4],
        );
    }
}
