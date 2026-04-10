//! Non-interactive vault preview and export (Phase 7).
//!
//! Both commands share a common "open vault → find entry → decrypt blobs" step.
//! After decryption:
//!   `--vault-preview` dispatches to the existing `render_preview` pipeline.
//!   `--vault-export`  writes the plaintext to a file on disk (atomic temp-rename).

use crate::cli::Cli;
use crate::pages::vault::crypto::{decrypt_blob_with_key, open_vault};
use crate::pages::vault::types::{VaultEntry, VaultError, VaultHandle};
use crate::password::read_password;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io,
    path::{Path, PathBuf},
    process,
};

// ── Public entry points ────────────────────────────────────────────────────

/// Run `--vault-preview`.  Never returns.
pub fn run_preview(cli: &Cli) -> ! {
    let vault_path_arg = cli.vault_preview.as_deref().unwrap();
    let vault_dir = resolve_vault_dir(cli);

    validate_vault_dir(&vault_dir);
    let password = read_password();
    let handle = open_vault_or_exit(&vault_dir, &password);

    let (entry, _uuid) = find_entry_or_exit(&handle, vault_path_arg);
    let ext = entry_ext(&entry.name);
    let bytes = decrypt_entry_or_exit(&handle, entry);

    // ── Initialise a minimal terminal for the render pipeline ─────────────
    if let Err(e) = enable_raw_mode() {
        eprintln!("error: could not enable raw mode: {}", e);
        process::exit(2);
    }
    let mut stdout = io::stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen) {
        eprintln!("error: could not enter alternate screen: {}", e);
        let _ = disable_raw_mode();
        process::exit(2);
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: could not create terminal: {}", e);
            let _ = disable_raw_mode();
            process::exit(2);
        }
    };

    // ── Dispatch to the render pipeline ──────────────────────────────────
    use crate::pages::preview::{PreviewPhase, PreviewResult, PreviewState, render_preview};
    let mut state = PreviewState::new();
    state.phase = PreviewPhase::PendingRender { bytes, ext: ext.clone() };
    render_preview(&mut state, &mut terminal);

    // ── Tear down terminal ────────────────────────────────────────────────
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    // ── Map result to exit code ───────────────────────────────────────────
    let exit_code = match &state.phase {
        PreviewPhase::Done(result) => match result {
            PreviewResult::NotSupported => {
                if ext.is_empty() {
                    eprintln!("No previewer for files with no extension");
                } else {
                    eprintln!("No previewer for .{ext} files");
                }
                0
            }
            PreviewResult::WrongPassword => {
                eprintln!("error: wrong password or corrupted file");
                1
            }
            PreviewResult::IoError(msg) => {
                eprintln!("error: {msg}");
                2
            }
            PreviewResult::MpvNotInstalled => {
                eprintln!("error: mpv is not installed; install it to preview media files");
                2
            }
            PreviewResult::RenderFailed(msg) => {
                eprintln!("error: preview failed: {msg}");
                2
            }
            PreviewResult::KittyShown
            | PreviewResult::XdgOpened
            | PreviewResult::MpvOpened
            | PreviewResult::GalleryShown(_)
            | PreviewResult::GalleryXdgOpened
            | PreviewResult::TextShown(_) => 0,
        },
        _ => {
            eprintln!("error: unexpected preview state after render");
            2
        }
    };

    process::exit(exit_code);
}

/// Run `--vault-export`.  Never returns.
pub fn run_export(cli: &Cli) -> ! {
    let vault_path_arg = cli.vault_export.as_deref().unwrap();
    let vault_dir = resolve_vault_dir(cli);
    let dest_dir = cli.dest.as_deref().unwrap_or(Path::new("."));

    validate_vault_dir(&vault_dir);

    // ── Validate destination directory ────────────────────────────────────
    if !dest_dir.exists() || !dest_dir.is_dir() {
        eprintln!("error: destination directory does not exist: {}", dest_dir.display());
        process::exit(2);
    }

    let password = read_password();
    let handle = open_vault_or_exit(&vault_dir, &password);

    let (entry, _uuid) = find_entry_or_exit(&handle, vault_path_arg);
    let out_path = dest_dir.join(&entry.name);

    // ── Collision check ───────────────────────────────────────────────────
    if out_path.exists() && !cli.force {
        eprintln!(
            "error: output file already exists: {} (use -f to overwrite)",
            out_path.display()
        );
        process::exit(4);
    }

    let bytes = decrypt_entry_or_exit(&handle, entry);

    // ── Atomic write (temp file in dest dir → rename) ─────────────────────
    let mut tmp = match tempfile::Builder::new().prefix(".pnd_").tempfile_in(dest_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot create temp file in {}: {}", dest_dir.display(), e);
            process::exit(2);
        }
    };

    if let Err(e) = io::Write::write_all(tmp.as_file_mut(), &bytes) {
        drop(tmp);
        eprintln!("error: write failed: {}", e);
        process::exit(2);
    }

    match tmp.persist(&out_path) {
        Ok(_) => {
            println!("{} → {}", vault_path_arg, out_path.display());
            process::exit(0);
        }
        Err(e) => {
            drop(e.file);
            eprintln!("error: could not write output: {}", e.error);
            process::exit(2);
        }
    }
}

// ── Shared helpers ─────────────────────────────────────────────────────────

/// Determine the vault directory: `--vault-dir` flag, falling back to `.`.
fn resolve_vault_dir(cli: &Cli) -> PathBuf {
    cli.vault_dir
        .as_deref()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Validate that `dir` exists, is a directory, and contains `index.lock`.
fn validate_vault_dir(dir: &Path) {
    if !dir.exists() {
        eprintln!("error: directory not found: {}", dir.display());
        process::exit(2);
    }
    if !dir.is_dir() {
        eprintln!("error: {} is not a directory", dir.display());
        process::exit(3);
    }
    if !dir.join("index.lock").exists() {
        eprintln!("error: no vault found at {}", dir.display());
        process::exit(2);
    }
}

/// Open the vault, mapping errors to exit codes.
fn open_vault_or_exit(vault_dir: &Path, password: &str) -> VaultHandle {
    match open_vault(vault_dir, password) {
        Ok(h) => h,
        Err(VaultError::WrongPassword) => {
            eprintln!("error: wrong password or corrupted index");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(2);
        }
    }
}

/// Find the entry whose full virtual path (`folder/name`) matches `vault_path`.
///
/// `vault_path` is matched case-sensitively. Returns `(entry, uuid)`.
fn find_entry_or_exit<'h>(
    handle: &'h VaultHandle,
    vault_path: &str,
) -> (&'h VaultEntry, &'h str) {
    let vault_path = vault_path.trim_matches('/');

    for (uuid, entry) in &handle.index.entries {
        let full = if entry.path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", entry.path, entry.name)
        };
        if full == vault_path {
            return (entry, uuid.as_str());
        }
    }

    eprintln!("error: entry not found in vault: {}", vault_path);
    process::exit(2);
}

/// Decrypt all blob parts for `entry` and return the concatenated plaintext.
fn decrypt_entry_or_exit(handle: &VaultHandle, entry: &VaultEntry) -> Vec<u8> {
    let mut out = Vec::with_capacity(entry.size as usize);

    for part in &entry.parts {
        let blob_path = handle.blob_path(&part.uuid);
        let blob = match std::fs::read(&blob_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: cannot read blob {}: {}", blob_path.display(), e);
                process::exit(2);
            }
        };
        match decrypt_blob_with_key(&blob, &part.key_base64) {
            Ok(plain) => out.extend_from_slice(&plain),
            Err(VaultError::WrongPassword) => {
                eprintln!("error: blob decryption failed (corrupted vault or wrong password)");
                process::exit(1);
            }
            Err(e) => {
                eprintln!("error: {}", e);
                process::exit(2);
            }
        }
    }

    out
}

/// Extract the lowercased extension from a filename (without the leading dot).
fn entry_ext(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
