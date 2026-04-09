//! State machine and background decryption worker for the Preview page.

use std::{io, path::Path, sync::mpsc};

// ── Worker messages ────────────────────────────────────────────────────────

/// Messages sent from the background decryption worker to the main thread.
pub(super) enum WorkerMsg {
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
    /// Background thread is decrypting; value is 0–100 percent complete.
    Decrypting(u8),
    /// Bytes are ready; `render_preview` must be called on the main thread
    /// before the next `terminal.draw`.
    PendingRender { bytes: Vec<u8>, ext: String },
    Done(PreviewResult),
}

pub(crate) enum PreviewResult {
    /// File type has no supported previewer.
    NotSupported,
    WrongPassword,
    IoError(String),
    /// Image rendered inline via the Kitty terminal graphics protocol.
    KittyShown,
    /// Image opened in the system viewer via xdg-open.
    XdgOpened,
    RenderFailed(String),
    /// Media file played in mpv (playback has finished).
    MpvOpened,
    /// mpv was not found on PATH; user needs to install it.
    MpvNotInstalled,
    /// ZIP image gallery browsed inline via the Kitty protocol. Carries the image count.
    GalleryShown(usize),
    /// ZIP gallery opened with xdg-open (non-Kitty terminal).
    GalleryXdgOpened,
    /// Text file previewed (bat or ratatui viewer). Carries the line count (0 when bat was used).
    TextShown(usize),
}

// ── Page state ─────────────────────────────────────────────────────────────

/// Focus positions: 0 = path field, 1 = password field.
pub(crate) struct PreviewState {
    pub(crate) path: String,
    pub(crate) password: String,
    pub(crate) focus: usize,
    /// When `true` the path field accepts keyboard text input.
    /// When `false` it shows the selected path with picker action shortcuts.
    pub(crate) path_edit_mode: bool,
    pub(crate) phase: PreviewPhase,
    pub(super) progress_rx: Option<mpsc::Receiver<WorkerMsg>>,
}

impl PreviewState {
    pub(crate) fn new() -> Self {
        Self {
            path: String::new(),
            password: String::new(),
            focus: 0,
            path_edit_mode: false,
            phase: PreviewPhase::Idle,
            progress_rx: None,
        }
    }

    /// Cycle focus forward between path (0) and password (1).
    pub(crate) fn advance_focus(&mut self) {
        self.focus = (self.focus + 1) % 2;
    }

    /// Drain pending messages from the background worker.
    pub(crate) fn poll_progress(&mut self) {
        if self.progress_rx.is_none() {
            return;
        }
        loop {
            match self.progress_rx.as_ref().unwrap().try_recv() {
                Ok(WorkerMsg::Progress(pct)) => {
                    self.phase = PreviewPhase::Decrypting(pct);
                }
                Ok(WorkerMsg::DecryptedBytes(bytes, ext)) => {
                    self.progress_rx = None;
                    self.phase = PreviewPhase::PendingRender { bytes, ext };
                    break;
                }
                Ok(WorkerMsg::WrongPassword) => {
                    self.progress_rx = None;
                    self.phase = PreviewPhase::Done(PreviewResult::WrongPassword);
                    break;
                }
                Ok(WorkerMsg::IoError(msg)) => {
                    self.progress_rx = None;
                    self.phase = PreviewPhase::Done(PreviewResult::IoError(msg));
                    break;
                }
                Err(_) => break,
            }
        }
    }

    /// Validate inputs and spawn the decryption worker. Returns immediately.
    pub(crate) fn start(&mut self) {
        let path = self.path.trim().to_string();
        let password = self.password.clone();

        if path.is_empty() {
            self.phase = PreviewPhase::Done(PreviewResult::IoError(
                "File path cannot be empty.".into(),
            ));
            return;
        }
        if password.is_empty() {
            self.phase = PreviewPhase::Done(PreviewResult::IoError(
                "Password cannot be empty.".into(),
            ));
            return;
        }
        if !path.ends_with(".lock") {
            self.phase = PreviewPhase::Done(PreviewResult::IoError(
                "File must have a .lock extension.".into(),
            ));
            return;
        }

        let total_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(1).max(1);

        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        self.progress_rx = Some(rx);
        self.phase = PreviewPhase::Decrypting(0);

        std::thread::spawn(move || {
            let tx_prog = tx.clone();
            let mut bytes_done = 0u64;
            let mut on_progress = move |n: usize| {
                bytes_done += n as u64;
                let pct = ((bytes_done * 100) / total_bytes).min(100) as u8;
                let _ = tx_prog.send(WorkerMsg::Progress(pct));
            };

            // Derive the original extension by stripping ".lock" then reading the extension.
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
                Ok(bytes) => {
                    let _ = tx.send(WorkerMsg::DecryptedBytes(bytes, ext));
                }
                Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                    let _ = tx.send(WorkerMsg::WrongPassword);
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::IoError(e.to_string()));
                }
            }
        });
    }
}
