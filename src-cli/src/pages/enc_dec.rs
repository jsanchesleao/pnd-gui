use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};
use std::{io, sync::mpsc};

use crate::{App, Screen, ACCENT, DIM, FAILURE, SUCCESS};
use crate::file_browser::FileBrowserTarget;

// ── Worker messages ────────────────────────────────────────────────────────

pub(crate) enum WorkerMsg {
    Progress(u8),
    Done(OpStatus),
}

pub(crate) enum OpStatus {
    Idle,
    Running(u8), // 0–100 percent
    Success(String),
    Failure(String),
}

// ── State ──────────────────────────────────────────────────────────────────

/// Focus positions: 0 = path field, 1 = password field, 2 = Execute button.
pub(crate) struct EncDecState {
    pub(crate) path: String,
    pub(crate) password: String,
    pub(crate) focus: usize,
    pub(crate) status: OpStatus,
    progress_rx: Option<mpsc::Receiver<WorkerMsg>>,
}

impl EncDecState {
    pub(crate) fn new() -> Self {
        Self {
            path: String::new(),
            password: String::new(),
            focus: 0,
            status: OpStatus::Idle,
            progress_rx: None,
        }
    }

    pub(crate) fn is_decrypt(&self) -> bool {
        self.path.trim_end().ends_with(".lock")
    }

    pub(crate) fn advance_focus(&mut self) {
        self.focus = (self.focus + 1) % 2;
    }

    /// Drain any pending messages from the background worker thread.
    pub(crate) fn poll_progress(&mut self) {
        if self.progress_rx.is_none() {
            return;
        }
        loop {
            let msg = match self.progress_rx.as_ref().unwrap().try_recv() {
                Ok(m) => m,
                Err(_) => break,
            };
            match msg {
                WorkerMsg::Progress(pct) => {
                    self.status = OpStatus::Running(pct);
                }
                WorkerMsg::Done(status) => {
                    self.status = status;
                    self.progress_rx = None;
                    self.path.clear();
                    self.password.clear();
                    self.focus = 0;
                    break;
                }
            }
        }
    }

    /// Spawn a background thread to run the operation. Returns immediately.
    pub(crate) fn start(&mut self) {
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

        let total_bytes = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(1)
            .max(1);
        let is_decrypt = self.is_decrypt();

        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        self.progress_rx = Some(rx);
        self.status = OpStatus::Running(0);

        std::thread::spawn(move || {
            let tx_prog = tx.clone();
            let mut bytes_done = 0u64;
            let mut on_progress = move |n: usize| {
                bytes_done += n as u64;
                let pct = ((bytes_done * 100) / total_bytes).min(100) as u8;
                let _ = tx_prog.send(WorkerMsg::Progress(pct));
            };

            if is_decrypt {
                let out = path.strip_suffix(".lock").unwrap().to_string();
                let result = (|| -> io::Result<bool> {
                    let mut input = std::fs::File::open(&path)?;
                    let mut output = std::fs::File::create(&out)?;
                    crate::crypto::decrypt_file(&mut input, &mut output, &password, &mut on_progress)
                })();
                let final_status = match result {
                    Ok(true) => OpStatus::Success(format!("Saved → {out}")),
                    Ok(false) => {
                        let _ = std::fs::remove_file(&out);
                        OpStatus::Failure(
                            "Decryption failed — wrong password or corrupted file.".into(),
                        )
                    }
                    Err(e) => OpStatus::Failure(format!("Error: {e}")),
                };
                let _ = tx.send(WorkerMsg::Done(final_status));
            } else {
                let out = format!("{path}.lock");
                let result = (|| -> io::Result<()> {
                    let mut input = std::fs::File::open(&path)?;
                    let mut output = std::fs::File::create(&out)?;
                    crate::crypto::encrypt_file(&mut input, &mut output, &password, &mut on_progress)
                })();
                let final_status = match result {
                    Ok(()) => OpStatus::Success(format!("Saved → {out}")),
                    Err(e) => OpStatus::Failure(format!("Error: {e}")),
                };
                let _ = tx.send(WorkerMsg::Done(final_status));
            }
        });
    }
}

// ── Drawing helpers (private to this module) ───────────────────────────────

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

fn input_block(label: &str, focused: bool) -> Block<'_> {
    let color = if focused { ACCENT } else { DIM };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(format!(" {label} "), Style::default().fg(color)))
}

/// Trim so the tail (cursor end) fits within `cols`, keeping recently typed
/// characters visible. Byte-level — safe for ASCII paths; Unicode clips gracefully.
fn tail_fit(s: &str, cols: usize) -> &str {
    if s.len() <= cols { s } else { &s[s.len() - cols..] }
}

// ── Draw ───────────────────────────────────────────────────────────────────

pub fn draw_enc_dec(frame: &mut Frame, state: &EncDecState) {
    let area = frame.area();
    frame.render_widget(outer_block("Encrypt / Decrypt"), area);

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0]  mode label
            Constraint::Length(1), // [1]  blank
            Constraint::Length(3), // [2]  path input
            Constraint::Length(1), // [3]  path sub-hint
            Constraint::Length(1), // [4]  blank
            Constraint::Length(3), // [5]  password input
            Constraint::Length(1), // [6]  blank
            Constraint::Length(1), // [7]  progress label  /  blank
            Constraint::Length(1), // [8]  progress gauge  /  status text
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
                if state.is_decrypt() { "  (.lock suffix detected)" } else { "  (no .lock suffix)" },
                Style::default().fg(DIM),
            ),
        ])),
        c[0],
    );

    // [2] path input
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = {
        let s = if state.focus == 0 { format!("{}|", state.path) } else { state.path.clone() };
        tail_fit(&s, inner_w).to_string()
    };
    let path_label = if state.focus == 0 { "File path  Enter→browse" } else { "File path" };
    frame.render_widget(
        Paragraph::new(path_display.as_str()).block(input_block(path_label, state.focus == 0)),
        c[2],
    );

    // [3] path sub-hint (only visible when path field is focused)
    if state.focus == 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "  type a path, or press Enter to browse the filesystem",
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )),
            c[3],
        );
    }

    // [5] password input (masked)
    let masked = "•".repeat(state.password.len());
    let pass_display = if state.focus == 1 { format!("{masked}|") } else { masked };
    frame.render_widget(
        Paragraph::new(pass_display.as_str()).block(input_block("Password", state.focus == 1)),
        c[5],
    );

    // [7] + [8]: progress bar when running, status text otherwise
    let is_running = matches!(state.status, OpStatus::Running(_));
    match &state.status {
        OpStatus::Running(pct) => {
            let action = if state.is_decrypt() { "Decrypting" } else { "Encrypting" };
            frame.render_widget(
                Paragraph::new(Span::styled(
                    format!("  {action}…"),
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )),
                c[7],
            );
            frame.render_widget(
                Gauge::default()
                    .gauge_style(Style::default().fg(ACCENT).bg(DIM))
                    .ratio(*pct as f64 / 100.0)
                    .label(format!("{pct}%")),
                c[8],
            );
        }
        other => {
            // [7] stays blank; [8] shows status
            let status_line = match other {
                OpStatus::Idle => Line::from(""),
                OpStatus::Success(msg) => Line::from(Span::styled(
                    format!("✓  {msg}"),
                    Style::default().fg(SUCCESS),
                )),
                OpStatus::Failure(msg) => Line::from(Span::styled(
                    format!("✗  {msg}"),
                    Style::default().fg(FAILURE),
                )),
                OpStatus::Running(_) => unreachable!(),
            };
            frame.render_widget(Paragraph::new(status_line), c[8]);
        }
    }

    // [10] hint (context-sensitive)
    let hint = if is_running {
        "please wait…"
    } else {
        match state.focus {
            0 => "Esc back    Tab next field    Enter browse filesystem",
            _ => "Esc back    Tab previous field    Enter run",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}

// ── Key handler ────────────────────────────────────────────────────────────

/// Handle a keypress on the Encrypt/Decrypt page.
///
/// Keys that navigate away (Esc, q-on-button, Enter-on-path) are processed
/// before we borrow `enc_dec`, to avoid overlapping mutable borrows.
pub fn handle_enc_dec(app: &mut App, code: KeyCode) {
    // Block all input while an operation is running.
    if matches!(app.enc_dec.status, OpStatus::Running(_)) {
        return;
    }

    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; return; }
        // Enter on the path field opens the file browser instead of advancing focus.
        KeyCode::Enter if app.enc_dec.focus == 0 => {
            let hint = app.enc_dec.path.clone();
            app.open_file_browser(&hint, FileBrowserTarget::EncDecPath);
            return;
        }
        _ => {}
    }

    let s = &mut app.enc_dec;
    match code {
        KeyCode::Tab | KeyCode::BackTab => s.advance_focus(),
        // Enter on the password field runs the operation immediately.
        KeyCode::Enter if s.focus == 1 => s.start(),
        KeyCode::Char(c) => {
            if s.focus == 0 {
                s.path.push(c);
                s.status = OpStatus::Idle;
            } else {
                s.password.push(c);
            }
        }
        KeyCode::Backspace => {
            if s.focus == 0 { s.path.pop(); s.status = OpStatus::Idle; }
            else { s.password.pop(); }
        }
        _ => {}
    }
}
