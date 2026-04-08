use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
               disable_raw_mode, enable_raw_mode},
};
use image::{ImageFormat, imageops::FilterType};
use ratatui::{
    Frame,
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};
use std::{io, io::Write as _, mem, path::Path, process::Command, sync::mpsc};
use tempfile::Builder;

use crate::{App, Screen, ACCENT, DIM, FAILURE, SUCCESS};
use crate::file_browser::FileBrowserTarget;

// ── Worker messages ────────────────────────────────────────────────────────

enum PreviewWorkerMsg {
    Progress(u8),
    /// Decryption succeeded. Carries the raw plaintext bytes and the original
    /// (pre-.lock) file extension, lowercased.
    DecryptedBytes(Vec<u8>, String),
    WrongPassword,
    IoError(String),
}

// ── Phase / result types ──────────────────────────────────────────────────

pub(crate) enum PreviewPhase {
    Idle,
    /// Background thread is decrypting; value is 0–100.
    Decrypting(u8),
    /// Bytes are ready; `render_image` must be called on the main thread before
    /// the next `terminal.draw`.
    PendingRender { bytes: Vec<u8>, ext: String },
    Done(PreviewResult),
}

pub(crate) enum PreviewResult {
    NotAnImage,
    WrongPassword,
    IoError(String),
    KittyShown,
    XdgOpened,
    RenderFailed(String),
}

// ── State ──────────────────────────────────────────────────────────────────

pub(crate) struct PreviewState {
    pub(crate) path: String,
    pub(crate) password: String,
    /// 0 = path field, 1 = password field, 2 = Preview button.
    pub(crate) focus: usize,
    pub(crate) phase: PreviewPhase,
    progress_rx: Option<mpsc::Receiver<PreviewWorkerMsg>>,
}

impl PreviewState {
    pub(crate) fn new() -> Self {
        Self {
            path: String::new(),
            password: String::new(),
            focus: 0,
            phase: PreviewPhase::Idle,
            progress_rx: None,
        }
    }

    fn advance_focus(&mut self) { self.focus = (self.focus + 1) % 3; }
    fn retreat_focus(&mut self) { self.focus = (self.focus + 2) % 3; }

    /// Drain pending messages from the background worker.
    pub(crate) fn poll_progress(&mut self) {
        let rx = match &self.progress_rx {
            Some(r) => r,
            None => return,
        };
        loop {
            match rx.try_recv() {
                Ok(PreviewWorkerMsg::Progress(pct)) => {
                    self.phase = PreviewPhase::Decrypting(pct);
                }
                Ok(PreviewWorkerMsg::DecryptedBytes(bytes, ext)) => {
                    self.progress_rx = None;
                    self.phase = PreviewPhase::PendingRender { bytes, ext };
                    break;
                }
                Ok(PreviewWorkerMsg::WrongPassword) => {
                    self.progress_rx = None;
                    self.phase = PreviewPhase::Done(PreviewResult::WrongPassword);
                    break;
                }
                Ok(PreviewWorkerMsg::IoError(msg)) => {
                    self.progress_rx = None;
                    self.phase = PreviewPhase::Done(PreviewResult::IoError(msg));
                    break;
                }
                Err(_) => break,
            }
        }
    }

    /// Spawn the decryption worker. Returns immediately.
    pub(crate) fn start(&mut self) {
        let path = self.path.trim().to_string();
        let password = self.password.clone();

        if path.is_empty() {
            self.phase = PreviewPhase::Done(PreviewResult::IoError("File path cannot be empty.".into()));
            return;
        }
        if password.is_empty() {
            self.phase = PreviewPhase::Done(PreviewResult::IoError("Password cannot be empty.".into()));
            return;
        }
        if !path.ends_with(".lock") {
            self.phase = PreviewPhase::Done(PreviewResult::IoError("File must have a .lock extension.".into()));
            return;
        }

        let total_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(1).max(1);

        let (tx, rx) = mpsc::channel::<PreviewWorkerMsg>();
        self.progress_rx = Some(rx);
        self.phase = PreviewPhase::Decrypting(0);

        std::thread::spawn(move || {
            let tx_prog = tx.clone();
            let mut bytes_done = 0u64;
            let mut on_progress = move |n: usize| {
                bytes_done += n as u64;
                let pct = ((bytes_done * 100) / total_bytes).min(100) as u8;
                let _ = tx_prog.send(PreviewWorkerMsg::Progress(pct));
            };

            // Derive the original extension from the path before ".lock".
            let original = path.strip_suffix(".lock").unwrap_or(&path);
            let ext = Path::new(original)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            let result: io::Result<Vec<u8>> = (|| {
                let mut input = std::fs::File::open(&path)?;
                let mut buf = Vec::new();
                let ok = crate::crypto::decrypt_file(
                    &mut input, &mut buf, &password, &mut on_progress,
                )?;
                if !ok {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "wrong_password"));
                }
                Ok(buf)
            })();

            match result {
                Ok(bytes) => { let _ = tx.send(PreviewWorkerMsg::DecryptedBytes(bytes, ext)); }
                Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                    let _ = tx.send(PreviewWorkerMsg::WrongPassword);
                }
                Err(e) => { let _ = tx.send(PreviewWorkerMsg::IoError(e.to_string())); }
            }
        });
    }
}

// ── Image rendering ────────────────────────────────────────────────────────

fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif")
}

fn supports_kitty() -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    let prog = std::env::var("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
    term == "xterm-kitty" || prog == "kitty" || prog == "wezterm"
}

fn decode_rgba(bytes: &[u8], ext: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let fmt = match ext {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png"          => ImageFormat::Png,
        "gif"          => ImageFormat::Gif,
        "webp"         => ImageFormat::WebP,
        "bmp"          => ImageFormat::Bmp,
        "tiff" | "tif" => ImageFormat::Tiff,
        other          => return Err(format!("unsupported image format: {other}")),
    };
    let img = image::load_from_memory_with_format(bytes, fmt).map_err(|e| e.to_string())?;

    // Scale down large images to avoid overwhelming the terminal.
    let img = {
        let (w, h) = (img.width(), img.height());
        if w > 1920 || h > 1080 {
            let scale = (1920.0 / w as f64).min(1080.0 / h as f64);
            let nw = (w as f64 * scale) as u32;
            let nh = (h as f64 * scale) as u32;
            img.resize(nw, nh, FilterType::Lanczos3)
        } else {
            img
        }
    };

    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Transmit image bytes via the Kitty terminal graphics protocol.
/// RGBA data is sent in 3072-byte chunks, each base64-encoded inside an APC sequence.
fn transmit_kitty(out: &mut impl io::Write, rgba: &[u8], width: u32, height: u32) -> io::Result<()> {
    const CHUNK: usize = 3072;
    let chunks: Vec<&[u8]> = rgba.chunks(CHUNK).collect();
    let total = chunks.len();

    // Handle degenerate case of empty image.
    if total == 0 {
        write!(out, "\x1b_Ga=T,f=32,s={width},v={height},m=0;\x1b\\")?;
        return Ok(());
    }

    for (i, chunk) in chunks.iter().enumerate() {
        let encoded = STANDARD.encode(chunk);
        let more = u8::from(i + 1 < total);
        let params = if i == 0 {
            format!("a=T,f=32,s={width},v={height},m={more}")
        } else {
            format!("m={more}")
        };
        write!(out, "\x1b_G{params};{encoded}\x1b\\")?;
    }
    Ok(())
}

/// Suspend ratatui, render the image with the Kitty protocol, wait for a keypress,
/// then resume ratatui.
fn render_kitty(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> io::Result<()> {
    // Suspend ratatui.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

    transmit_kitty(&mut stdout, rgba, width, height)?;
    stdout.flush()?;

    // Prompt below the image.
    println!("\r\n\r\n[Press any key to return]");
    stdout.flush()?;

    // Wait for a keypress in raw mode.
    enable_raw_mode()?;
    loop {
        if let Event::Key(k) = event::read()? {
            if k.kind == KeyEventKind::Press { break; }
        }
    }
    disable_raw_mode()?;

    // Resume ratatui.
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}

/// Open the image bytes with the system viewer via xdg-open.
fn open_with_xdg(bytes: &[u8], ext: &str) -> Result<(), String> {
    let mut tmp = Builder::new()
        .prefix("pnd-preview-")
        .suffix(&format!(".{ext}"))
        .tempfile()
        .map_err(|e| e.to_string())?;

    tmp.write_all(bytes).map_err(|e| e.to_string())?;
    tmp.flush().map_err(|e| e.to_string())?;

    // Keep the file so xdg-open can read it (leaks in /tmp — acceptable).
    let (_, path) = tmp.keep().map_err(|e| e.to_string())?;

    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;

    Ok(())
}

/// Called from `main.rs` before `terminal.draw` whenever the phase is
/// `PendingRender`. Moves bytes out, decodes, and dispatches to Kitty or xdg-open.
pub(crate) fn render_image(
    state: &mut PreviewState,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) {
    let (bytes, ext) = match mem::replace(&mut state.phase, PreviewPhase::Idle) {
        PreviewPhase::PendingRender { bytes, ext } => (bytes, ext),
        other => { state.phase = other; return; }
    };

    if !is_image_ext(&ext) {
        state.phase = PreviewPhase::Done(PreviewResult::NotAnImage);
        return;
    }

    let result = match decode_rgba(&bytes, &ext) {
        Err(e) => PreviewResult::RenderFailed(e),
        Ok((rgba, w, h)) => {
            if supports_kitty() {
                match render_kitty(terminal, &rgba, w, h) {
                    Ok(()) => PreviewResult::KittyShown,
                    Err(e) => PreviewResult::RenderFailed(e.to_string()),
                }
            } else {
                match open_with_xdg(&bytes, &ext) {
                    Ok(()) => PreviewResult::XdgOpened,
                    Err(e) => PreviewResult::RenderFailed(e),
                }
            }
        }
    };

    state.phase = PreviewPhase::Done(result);
    state.path.clear();
    state.password.clear();
    state.focus = 0;
}

// ── Drawing helpers ────────────────────────────────────────────────────────

fn outer_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " Preview ",
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

fn tail_fit(s: &str, cols: usize) -> &str {
    if s.len() <= cols { s } else { &s[s.len() - cols..] }
}

// ── Draw ───────────────────────────────────────────────────────────────────

pub fn draw_preview(frame: &mut Frame, state: &PreviewState) {
    let area = frame.area();
    frame.render_widget(outer_block(), area);

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
            Constraint::Length(1), // [7]  Preview button
            Constraint::Length(1), // [8]  progress label / blank
            Constraint::Length(1), // [9]  progress gauge / status text
            Constraint::Min(0),    // [10] filler
            Constraint::Length(1), // [11] hint bar
        ])
        .split(area);

    let is_decrypting = matches!(state.phase, PreviewPhase::Decrypting(_));

    // [0] info line
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Decrypt a .lock file and preview its contents",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))),
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

    // [3] path sub-hint
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

    // [7] Preview button
    let btn_style = if !is_decrypting && state.focus == 2 {
        Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("[ Preview ]", btn_style)))
            .alignment(Alignment::Center),
        c[7],
    );

    // [8] + [9]: progress bar when decrypting, status text otherwise
    match &state.phase {
        PreviewPhase::Decrypting(pct) => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Decrypting…",
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )),
                c[8],
            );
            frame.render_widget(
                Gauge::default()
                    .gauge_style(Style::default().fg(ACCENT).bg(DIM))
                    .ratio(*pct as f64 / 100.0)
                    .label(format!("{pct}%")),
                c[9],
            );
        }
        PreviewPhase::PendingRender { .. } => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Rendering…",
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )),
                c[8],
            );
        }
        PreviewPhase::Done(result) => {
            let line = match result {
                PreviewResult::NotAnImage => Line::from(Span::styled(
                    "✗  Preview not available for this file type",
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::WrongPassword => Line::from(Span::styled(
                    "✗  Wrong password or corrupted file",
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::IoError(msg) => Line::from(Span::styled(
                    format!("✗  Error: {msg}"),
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::KittyShown => Line::from(Span::styled(
                    "✓  Image displayed in terminal",
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::XdgOpened => Line::from(Span::styled(
                    "✓  Opened in system image viewer",
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::RenderFailed(msg) => Line::from(Span::styled(
                    format!("✗  Render failed: {msg}"),
                    Style::default().fg(FAILURE),
                )),
            };
            frame.render_widget(Paragraph::new(line), c[9]);
        }
        PreviewPhase::Idle => {}
    }

    // [11] hint bar
    let hint = if is_decrypting {
        "please wait…"
    } else {
        match state.focus {
            0 => "Esc back    Tab next field    Enter browse filesystem",
            1 => "Esc back    Tab next field    Enter advance",
            _ => "Esc back    Tab next field    Enter preview",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[11],
    );
}

// ── Key handler ────────────────────────────────────────────────────────────

pub fn handle_preview(app: &mut App, code: KeyCode) {
    if matches!(app.preview.phase, PreviewPhase::Decrypting(_)) {
        return;
    }

    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; return; }
        KeyCode::Char('q') if app.preview.focus == 2 => { app.screen = Screen::Menu; return; }
        KeyCode::Enter if app.preview.focus == 0 => {
            let hint = app.preview.path.clone();
            app.open_file_browser(&hint, FileBrowserTarget::PreviewPath);
            return;
        }
        _ => {}
    }

    let s = &mut app.preview;
    match code {
        KeyCode::Tab => s.advance_focus(),
        KeyCode::BackTab => s.retreat_focus(),
        KeyCode::Enter => {
            if s.focus == 2 { s.start(); } else { s.advance_focus(); }
        }
        KeyCode::Char(c) if s.focus < 2 => {
            if s.focus == 0 {
                s.path.push(c);
                s.phase = PreviewPhase::Idle;
            } else {
                s.password.push(c);
            }
        }
        KeyCode::Backspace if s.focus < 2 => {
            if s.focus == 0 { s.path.pop(); s.phase = PreviewPhase::Idle; }
            else { s.password.pop(); }
        }
        _ => {}
    }
}
