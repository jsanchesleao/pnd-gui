//! Non-interactive single-file encrypt / decrypt (Phases 2 and 10-B).
//!
//! Input source:
//!   - File path (positional argument): mode auto-detected from `.lock` extension.
//!   - Explicit stdin (`-`): `--mode` / `-m` required.
//!   - Implicit stdin (no file, stdin not a TTY): `--mode` / `-m` required.
//!
//! Output destination:
//!   - `--stdout` / `-c`: write directly to stdout (progress suppressed).
//!   - `-o PATH`: write to a named file via atomic temp-rename.
//!   - Default with file source: `<file>.lock` (encrypt) or `<file>` with `.lock` stripped (decrypt).
//!   - Default with stdin source and no `-o`: implicit stdout (same as `--stdout`).

use crate::cli::{Cli, OperationMode};
use crate::password::read_password;
use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process,
};

// ── Entry point ────────────────────────────────────────────────────────────

/// Run non-interactive encrypt/decrypt. Never returns — always calls `process::exit`.
pub fn run(cli: &Cli) -> ! {
    // ── Detect input source ───────────────────────────────────────────────
    let explicit_stdin = cli.files.first().map(|p| p.as_os_str() == "-").unwrap_or(false);
    let implicit_stdin = cli.files.is_empty() && !io::stdin().is_terminal();
    let stdin_source = explicit_stdin || implicit_stdin;

    if !stdin_source && cli.files.len() > 1 {
        eprintln!(
            "error: encrypt/decrypt takes exactly one file; got {}",
            cli.files.len()
        );
        process::exit(3);
    }

    // ── Require --mode when reading from stdin ────────────────────────────
    if stdin_source && cli.mode.is_none() {
        eprintln!("error: stdin is not a TTY; use -m to specify encrypt or decrypt");
        process::exit(3);
    }

    // ── Determine operation mode ──────────────────────────────────────────
    let is_decrypt = if let Some(mode) = cli.mode {
        matches!(mode, OperationMode::Decrypt)
    } else {
        // File source: auto-detect from extension.
        cli.files[0]
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("lock"))
            .unwrap_or(false)
    };

    // ── Determine output destination ──────────────────────────────────────
    // Implicit stdout when stdin is the source and -o is not given.
    // Explicit --stdout wins over -o (with a warning).
    let write_to_stdout = if cli.stdout {
        if cli.output.is_some() {
            eprintln!("warning: -o is ignored when --stdout is given");
        }
        true
    } else {
        stdin_source && cli.output.is_none()
    };

    // ── Read password ─────────────────────────────────────────────────────
    let password = read_password();

    let op_label = if is_decrypt { "Decrypting" } else { "Encrypting" };

    // ── Stdin source path ─────────────────────────────────────────────────
    if stdin_source {
        let mut input = io::stdin();

        if write_to_stdout {
            // stdin → stdout: no temp file, no progress.
            let mut out = io::stdout();
            let result: io::Result<bool> = if is_decrypt {
                crate::crypto::decrypt_file(&mut input, &mut out, &password, &mut |_| {})
            } else {
                crate::crypto::encrypt_file(&mut input, &mut out, &password, &mut |_| {})
                    .map(|()| true)
            };
            handle_stdout_result(result);
        }

        // stdin → named file (-o PATH, atomic rename).
        let out_path = cli.output.as_deref()
            .expect("output must be Some when write_to_stdout is false");

        if out_path.exists() && !cli.force {
            eprintln!(
                "error: output file already exists: {} (use -f to overwrite)",
                out_path.display()
            );
            process::exit(4);
        }

        let out_parent = out_path.parent().unwrap_or(Path::new("."));
        let mut tmp = make_tempfile(out_parent);
        let stderr_tty = io::stderr().is_terminal();
        let mut bytes_done: u64 = 0;

        let result: io::Result<bool> = {
            let out = tmp.as_file_mut();
            if is_decrypt {
                crate::crypto::decrypt_file(&mut input, out, &password, &mut |n| {
                    bytes_done += n as u64;
                    report_progress_bytes(stderr_tty, op_label, bytes_done);
                })
            } else {
                crate::crypto::encrypt_file(&mut input, out, &password, &mut |n| {
                    bytes_done += n as u64;
                    report_progress_bytes(stderr_tty, op_label, bytes_done);
                })
                .map(|()| true)
            }
        };

        if stderr_tty {
            eprint!("\r\x1b[K");
            let _ = io::stderr().flush();
        }

        handle_file_result(result, tmp, out_path, "stdin");
    }

    // ── File source path ──────────────────────────────────────────────────
    let input_path = &cli.files[0];

    if !input_path.exists() {
        eprintln!("error: file not found: {}", input_path.display());
        process::exit(2);
    }
    if input_path.is_dir() {
        eprintln!("error: {} is a directory", input_path.display());
        process::exit(3);
    }

    let mut input_file = match fs::File::open(input_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot open {}: {}", input_path.display(), e);
            process::exit(2);
        }
    };

    if write_to_stdout {
        // file → stdout: no temp file, no progress.
        let mut out = io::stdout();
        let result: io::Result<bool> = if is_decrypt {
            crate::crypto::decrypt_file(&mut input_file, &mut out, &password, &mut |_| {})
        } else {
            crate::crypto::encrypt_file(&mut input_file, &mut out, &password, &mut |_| {})
                .map(|()| true)
        };
        handle_stdout_result(result);
    }

    // ── File → named output file (atomic temp-rename) ─────────────────────
    let default_out: PathBuf = if is_decrypt {
        input_path.with_extension("") // strip .lock
    } else {
        let fname = input_path
            .file_name()
            .expect("regular file must have a name")
            .to_string_lossy();
        input_path.with_file_name(format!("{}.lock", fname))
    };

    let out_path: &Path = cli.output.as_deref().unwrap_or(&default_out);

    if out_path.exists() && !cli.force {
        eprintln!(
            "error: output file already exists: {} (use -f to overwrite)",
            out_path.display()
        );
        process::exit(4);
    }

    // Total bytes for progress percentage (close enough — overhead is small).
    let file_size = input_path.metadata().map(|m| m.len()).unwrap_or(0);

    let out_parent = out_path.parent().unwrap_or(Path::new("."));
    let mut tmp = make_tempfile(out_parent);
    let stderr_tty = io::stderr().is_terminal();
    let mut bytes_done: u64 = 0;

    let result: io::Result<bool> = {
        let out = tmp.as_file_mut();
        if is_decrypt {
            crate::crypto::decrypt_file(
                &mut input_file,
                out,
                &password,
                &mut |n| {
                    bytes_done += n as u64;
                    report_progress(stderr_tty, op_label, bytes_done, file_size);
                },
            )
        } else {
            crate::crypto::encrypt_file(
                &mut input_file,
                out,
                &password,
                &mut |n| {
                    bytes_done += n as u64;
                    report_progress(stderr_tty, op_label, bytes_done, file_size);
                },
            )
            .map(|()| true)
        }
    };

    if stderr_tty {
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    }

    handle_file_result(
        result,
        tmp,
        out_path,
        &input_path.display().to_string(),
    );
}

// ── Shared result handlers ─────────────────────────────────────────────────

/// Handle a crypto result when writing to stdout. Never returns.
fn handle_stdout_result(result: io::Result<bool>) -> ! {
    match result {
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(2);
        }
        Ok(false) => {
            eprintln!("error: wrong password or corrupted file");
            process::exit(1);
        }
        Ok(true) => process::exit(0),
    }
}

/// Handle a crypto result when writing to a temp file, then atomically rename. Never returns.
fn handle_file_result(
    result: io::Result<bool>,
    tmp: tempfile::NamedTempFile,
    out_path: &Path,
    input_label: &str,
) -> ! {
    match result {
        Err(e) => {
            // NamedTempFile auto-deletes on drop.
            drop(tmp);
            eprintln!("error: {}", e);
            process::exit(2);
        }
        Ok(false) => {
            drop(tmp);
            eprintln!("error: wrong password or corrupted file");
            process::exit(1);
        }
        Ok(true) => match tmp.persist(out_path) {
            Ok(_) => {
                println!("{} → {}", input_label, out_path.display());
                process::exit(0);
            }
            Err(e) => {
                drop(e.file);
                eprintln!("error: could not write output: {}", e.error);
                process::exit(2);
            }
        },
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn make_tempfile(dir: &Path) -> tempfile::NamedTempFile {
    tempfile::Builder::new()
        .prefix(".pnd_")
        .tempfile_in(dir)
        .unwrap_or_else(|e| {
            eprintln!("error: cannot create output file in {}: {}", dir.display(), e);
            process::exit(2);
        })
}

/// Progress with percentage — used when total file size is known.
fn report_progress(tty: bool, label: &str, done: u64, total: u64) {
    if !tty || total == 0 {
        return;
    }
    let pct = (done * 100 / total).min(100);
    eprint!("\r{}… {}%", label, pct);
    let _ = io::stderr().flush();
}

/// Progress with byte count only — used when reading from stdin (size unknown).
fn report_progress_bytes(tty: bool, label: &str, done: u64) {
    if !tty {
        return;
    }
    let (val, unit) = if done >= 1_048_576 {
        (done as f64 / 1_048_576.0, "MB")
    } else {
        (done as f64 / 1_024.0, "KB")
    };
    eprint!("\r{}… {:.1} {}", label, val, unit);
    let _ = io::stderr().flush();
}
