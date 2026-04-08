//! State machine and background worker for the Encrypt/Decrypt page.

use std::{io, sync::mpsc};

// ── Worker messages ────────────────────────────────────────────────────────

/// Messages sent from the background worker thread to the main thread.
pub(super) enum WorkerMsg {
    Progress(u8),
    Done(OpStatus),
}

// ── Operation status ───────────────────────────────────────────────────────

pub(crate) enum OpStatus {
    Idle,
    /// Background thread is running; value is 0–100 percent complete.
    Running(u8),
    Success(String),
    Failure(String),
}

// ── Page state ─────────────────────────────────────────────────────────────

/// Focus positions: 0 = path field, 1 = password field.
pub(crate) struct EncDecState {
    pub(crate) path: String,
    pub(crate) password: String,
    pub(crate) focus: usize,
    pub(crate) status: OpStatus,
    pub(super) progress_rx: Option<mpsc::Receiver<WorkerMsg>>,
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

    /// Returns true when the selected path has a `.lock` suffix, indicating decrypt mode.
    pub(crate) fn is_decrypt(&self) -> bool {
        self.path.trim_end().ends_with(".lock")
    }

    /// Cycle focus forward between path (0) and password (1).
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

    /// Validate inputs and spawn the encrypt/decrypt worker. Returns immediately.
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
