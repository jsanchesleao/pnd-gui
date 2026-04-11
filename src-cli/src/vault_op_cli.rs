//! Non-interactive vault preview and export (Phase 7).
//!
//! Both commands share a common "open vault → find entry → decrypt blobs" step.
//! After decryption:
//!   `--vault-preview` dispatches to the existing `render_preview` pipeline.
//!   `--vault-export`  writes the plaintext to a file on disk (atomic temp-rename).
//!
//! `--vault-export` also accepts a virtual folder path instead of a single file.
//! In that case it collects all matching entries, prompts for confirmation, then
//! exports each one.  `-r` / `--recursive` extends the collection to subfolders.

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
    io::{self, IsTerminal, Write as _},
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
            PreviewResult::InlineShown
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
    let vault_path_arg = cli.vault_export.as_deref().unwrap().trim_matches('/');
    let vault_dir = resolve_vault_dir(cli);

    // ── --stdout guards ───────────────────────────────────────────────────
    if cli.stdout && cli.recursive {
        eprintln!("error: --stdout is incompatible with --recursive");
        process::exit(3);
    }

    validate_vault_dir(&vault_dir);

    let password = read_password();
    let handle = open_vault_or_exit(&vault_dir, &password);

    // Prefer an exact file match; fall back to folder export.
    if let Some((entry, _)) = find_entry(&handle, vault_path_arg) {
        if cli.stdout {
            export_single_file_stdout(&handle, entry);
        } else {
            let dest_dir = cli.dest.as_deref().unwrap_or(Path::new("."));
            if !dest_dir.exists() || !dest_dir.is_dir() {
                eprintln!("error: destination directory does not exist: {}", dest_dir.display());
                process::exit(2);
            }
            export_single_file(&handle, entry, dest_dir, cli);
        }
    } else if is_vault_folder(&handle, vault_path_arg) {
        if cli.stdout {
            eprintln!("error: --stdout requires a single-file vault path, not a folder");
            process::exit(3);
        }
        let dest_dir = cli.dest.as_deref().unwrap_or(Path::new("."));
        if !dest_dir.exists() || !dest_dir.is_dir() {
            eprintln!("error: destination directory does not exist: {}", dest_dir.display());
            process::exit(2);
        }
        export_directory(&handle, vault_path_arg, dest_dir, cli);
    } else {
        eprintln!("error: no file or folder found at vault path: {}", vault_path_arg);
        process::exit(2);
    }
}

// ── Single-file export ─────────────────────────────────────────────────────

fn export_single_file_stdout(handle: &VaultHandle, entry: &VaultEntry) -> ! {
    let bytes = decrypt_entry_or_exit(handle, entry);
    if let Err(e) = io::stdout().write_all(&bytes) {
        eprintln!("error: write failed: {}", e);
        process::exit(2);
    }
    process::exit(0);
}

fn export_single_file(handle: &VaultHandle, entry: &VaultEntry, dest_dir: &Path, cli: &Cli) -> ! {
    let out_path = dest_dir.join(&entry.name);

    if out_path.exists() && !cli.force {
        eprintln!(
            "error: output file already exists: {} (use -f to overwrite)",
            out_path.display()
        );
        process::exit(4);
    }

    let bytes = decrypt_entry_or_exit(handle, entry);
    write_atomic(&bytes, &out_path, dest_dir);

    let vault_path = if entry.path.is_empty() {
        entry.name.clone()
    } else {
        format!("{}/{}", entry.path, entry.name)
    };
    println!("{} → {}", vault_path, out_path.display());
    process::exit(0);
}

// ── Directory export ───────────────────────────────────────────────────────

fn export_directory(handle: &VaultHandle, folder: &str, dest_dir: &Path, cli: &Cli) -> ! {
    // Collect matching entries.
    let entries = collect_folder_entries(handle, folder, cli.recursive);

    if entries.is_empty() {
        eprintln!("error: no files found under vault path: {}", folder);
        process::exit(2);
    }

    // Confirmation prompt (shown when stdin is a TTY and -y not given).
    if !cli.yes && io::stdin().is_terminal() {
        eprint!(
            "Extract {} file(s) into {}? [y/N] ",
            entries.len(),
            dest_dir.display()
        );
        let _ = io::stderr().flush();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).ok();
        if !answer.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            process::exit(0);
        }
    }

    // Pre-flight collision check (all files, before writing anything).
    if !cli.force {
        let mut any_collision = false;
        for entry in &entries {
            let rel = relative_out_path(entry, folder);
            let out = dest_dir.join(&rel);
            if out.exists() {
                eprintln!(
                    "error: output file already exists: {} (use -f to overwrite)",
                    out.display()
                );
                any_collision = true;
            }
        }
        if any_collision {
            process::exit(4);
        }
    }

    // Decrypt and write each entry.
    let mut exit_code = 0i32;
    for entry in &entries {
        let rel = relative_out_path(entry, folder);
        let out = dest_dir.join(&rel);

        // Create intermediate directories for recursive exports.
        if let Some(parent) = out.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("error: cannot create directory {}: {}", parent.display(), e);
                    exit_code = 2;
                    continue;
                }
            }
        }

        match decrypt_entry(handle, entry) {
            Ok(bytes) => {
                let out_parent = out.parent().unwrap_or(dest_dir);
                write_atomic(&bytes, &out, out_parent);
                let vault_full = if entry.path.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}/{}", entry.path, entry.name)
                };
                println!("{} → {}", vault_full, out.display());
            }
            Err(msg) => {
                eprintln!("error: {}: {}", entry.name, msg);
                exit_code = 2;
            }
        }
    }

    process::exit(exit_code);
}

// ── Entry collection ───────────────────────────────────────────────────────

/// Collect all entries directly inside `folder` (and, when `recursive`, in
/// all descendant folders too).
fn collect_folder_entries<'h>(
    handle: &'h VaultHandle,
    folder: &str,
    recursive: bool,
) -> Vec<&'h VaultEntry> {
    handle
        .index
        .entries
        .values()
        .filter(|e| {
            if recursive {
                // Any path equal to folder or a descendant.
                e.path == folder
                    || e.path.starts_with(&format!("{}/", folder))
                    || folder.is_empty()
            } else {
                e.path == folder
            }
        })
        .collect()
}

/// Build the output path for `entry` relative to `folder`.
///
/// Examples (folder = "photos/summer"):
///   entry.path = "photos/summer",        name = "beach.jpg"  → "beach.jpg"
///   entry.path = "photos/summer/2023",   name = "pic.jpg"    → "2023/pic.jpg"
fn relative_out_path(entry: &VaultEntry, folder: &str) -> PathBuf {
    let rel_dir: &str = if folder.is_empty() {
        // Exporting from vault root — preserve the full path.
        &entry.path
    } else if entry.path == folder {
        ""
    } else {
        // Strip the "folder/" prefix to get the sub-path.
        &entry.path[folder.len() + 1..]
    };

    if rel_dir.is_empty() {
        PathBuf::from(&entry.name)
    } else {
        PathBuf::from(rel_dir).join(&entry.name)
    }
}

/// Returns `true` when at least one vault entry lives under `folder`.
fn is_vault_folder(handle: &VaultHandle, folder: &str) -> bool {
    handle.index.entries.values().any(|e| {
        e.path == folder || e.path.starts_with(&format!("{}/", folder))
    })
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
/// Returns `None` when not found.
fn find_entry<'h>(handle: &'h VaultHandle, vault_path: &str) -> Option<(&'h VaultEntry, &'h str)> {
    let vault_path = vault_path.trim_matches('/');
    for (uuid, entry) in &handle.index.entries {
        let full = if entry.path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", entry.path, entry.name)
        };
        if full == vault_path {
            return Some((entry, uuid.as_str()));
        }
    }
    None
}

/// Like `find_entry` but exits with code 2 when not found.
fn find_entry_or_exit<'h>(
    handle: &'h VaultHandle,
    vault_path: &str,
) -> (&'h VaultEntry, &'h str) {
    find_entry(handle, vault_path).unwrap_or_else(|| {
        eprintln!("error: entry not found in vault: {}", vault_path.trim_matches('/'));
        process::exit(2);
    })
}

/// Decrypt all blob parts for `entry` and return the concatenated plaintext,
/// or an error message string.
fn decrypt_entry(handle: &VaultHandle, entry: &VaultEntry) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(entry.size as usize);
    for part in &entry.parts {
        let blob_path = handle.blob_path(&part.uuid);
        let blob = std::fs::read(&blob_path)
            .map_err(|e| format!("cannot read blob {}: {}", blob_path.display(), e))?;
        match decrypt_blob_with_key(&blob, &part.key_base64) {
            Ok(plain) => out.extend_from_slice(&plain),
            Err(VaultError::WrongPassword) => {
                return Err("blob decryption failed (corrupted vault or wrong password)".into());
            }
            Err(e) => return Err(format!("{}", e)),
        }
    }
    Ok(out)
}

/// Decrypt `entry` or exit on failure.
fn decrypt_entry_or_exit(handle: &VaultHandle, entry: &VaultEntry) -> Vec<u8> {
    decrypt_entry(handle, entry).unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        process::exit(2);
    })
}

/// Write `bytes` to `out_path` atomically (temp file in `tmp_dir` → rename).
fn write_atomic(bytes: &[u8], out_path: &Path, tmp_dir: &Path) {
    let mut tmp = match tempfile::Builder::new().prefix(".pnd_").tempfile_in(tmp_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot create temp file in {}: {}", tmp_dir.display(), e);
            process::exit(2);
        }
    };
    if let Err(e) = tmp.as_file_mut().write_all(bytes) {
        drop(tmp);
        eprintln!("error: write failed: {}", e);
        process::exit(2);
    }
    if let Err(e) = tmp.persist(out_path) {
        drop(e.file);
        eprintln!("error: could not write output: {}", e.error);
        process::exit(2);
    }
}

/// Extract the lowercased extension from a filename (without the leading dot).
fn entry_ext(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
