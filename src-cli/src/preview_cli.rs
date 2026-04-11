//! Non-interactive file preview (Phases 4 and 10-C).
//!
//! Decrypts a `.lock` file into memory (never to disk) and dispatches to the
//! existing `render_preview` pipeline (Kitty / mpv / bat / gallery).
//! Plain (non-encrypted) files are read directly without prompting for a password.
//!
//! Phase 10-C adds stdin source support: when stdin is not a TTY (or the explicit
//! `-` argument is given), `--ext` must be supplied to identify the file type.
//! Whether to decrypt is determined by `--mode decrypt` being present.

use crate::cli::{Cli, OperationMode};
use crate::pages::preview::{PreviewPhase, PreviewResult, PreviewState, render_preview};
use crate::password::read_password;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    fs,
    io::{self, IsTerminal, Read, Write},
    path::Path,
    process,
};

// ── Entry point ────────────────────────────────────────────────────────────

/// Run non-interactive preview.  Never returns — always calls `process::exit`.
pub fn run(cli: &Cli) -> ! {
    // ── Detect stdin source ───────────────────────────────────────────────
    let explicit_stdin = cli.files.first().map(|p| p.as_os_str() == "-").unwrap_or(false);
    let implicit_stdin = cli.files.is_empty() && !io::stdin().is_terminal();
    let stdin_source = explicit_stdin || implicit_stdin;

    if stdin_source {
        run_stdin(cli);
    }

    // ── File source ───────────────────────────────────────────────────────
    if cli.files.is_empty() {
        eprintln!("error: -p/--preview requires a file argument");
        process::exit(3);
    }
    if cli.files.len() > 1 {
        eprintln!("error: -p/--preview takes exactly one file");
        process::exit(3);
    }

    let input_path = &cli.files[0];

    // ── Validate input ────────────────────────────────────────────────────
    if !input_path.exists() {
        eprintln!("error: file not found: {}", input_path.display());
        process::exit(2);
    }
    if input_path.is_dir() {
        eprintln!("error: {} is a directory", input_path.display());
        process::exit(3);
    }

    // ── Determine if encrypted ────────────────────────────────────────────
    let is_encrypted = input_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("lock"))
        .unwrap_or(false);

    // ── Derive extension for preview dispatch ─────────────────────────────
    // For `.lock` files, strip the `.lock` suffix then read the next extension.
    let ext = if is_encrypted {
        let fname = input_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let without_lock = fname.strip_suffix(".lock").unwrap_or(&fname);
        Path::new(without_lock)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    } else {
        input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    };

    // ── Read bytes into memory ────────────────────────────────────────────
    let bytes: Vec<u8> = if is_encrypted {
        let password = read_password();
        let file_size = input_path.metadata().map(|m| m.len()).unwrap_or(1).max(1);
        let stderr_tty = io::stderr().is_terminal();
        let mut bytes_done: u64 = 0;

        let mut on_progress = move |n: usize| {
            bytes_done += n as u64;
            if stderr_tty {
                let pct = (bytes_done * 100 / file_size).min(100);
                eprint!("\rDecrypting\u{2026} {}%", pct);
                let _ = io::stderr().flush();
            }
        };

        let mut input_file = match fs::File::open(input_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: cannot open {}: {}", input_path.display(), e);
                process::exit(2);
            }
        };

        let mut buf = Vec::new();
        let result =
            crate::crypto::decrypt_file(&mut input_file, &mut buf, &password, &mut on_progress);

        if stderr_tty {
            eprint!("\r\x1b[K");
            let _ = io::stderr().flush();
        }

        match result {
            Err(e) => {
                eprintln!("error: {}", e);
                process::exit(2);
            }
            Ok(false) => {
                eprintln!("error: wrong password or corrupted file");
                process::exit(1);
            }
            Ok(true) => buf,
        }
    } else {
        // Plain (non-encrypted) file — read directly, no password needed.
        match fs::read(input_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", input_path.display(), e);
                process::exit(2);
            }
        }
    };

    render_and_exit(bytes, ext);
}

// ── Stdin path (Phase 10-C) ────────────────────────────────────────────────

/// Handle `-p` when the input comes from stdin.  Never returns.
fn run_stdin(cli: &Cli) -> ! {
    // --ext is required when reading from stdin.
    let ext = match &cli.ext {
        Some(e) => e.to_ascii_lowercase(),
        None => {
            eprintln!("error: --ext is required when piping into -p");
            process::exit(3);
        }
    };

    // Whether to decrypt: --mode decrypt (or -m d) must be given explicitly.
    let is_encrypted = matches!(cli.mode, Some(OperationMode::Decrypt));

    let bytes: Vec<u8> = if is_encrypted {
        let password = read_password();
        let mut input = io::stdin();
        let mut buf = Vec::new();

        let result = crate::crypto::decrypt_file(&mut input, &mut buf, &password, &mut |_| {});

        match result {
            Err(e) => {
                eprintln!("error: {}", e);
                process::exit(2);
            }
            Ok(false) => {
                eprintln!("error: wrong password or corrupted file");
                process::exit(1);
            }
            Ok(true) => buf,
        }
    } else {
        // Plain stdin — read all bytes directly.
        let mut buf = Vec::new();
        if let Err(e) = io::stdin().read_to_end(&mut buf) {
            eprintln!("error: cannot read stdin: {}", e);
            process::exit(2);
        }
        buf
    };

    render_and_exit(bytes, ext);
}

// ── Shared render pipeline ─────────────────────────────────────────────────

/// Set up the terminal, run the preview pipeline, tear down, and exit. Never returns.
fn render_and_exit(bytes: Vec<u8>, ext: String) -> ! {
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
                eprintln!(
                    "error: mpv is not installed; install it to preview media files"
                );
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
