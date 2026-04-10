//! Non-interactive single-file encrypt / decrypt (Phase 2).
//!
//! Mode is detected from the file extension: `.lock` → decrypt, anything else → encrypt.
//! Output path defaults to `<file>.lock` (encrypt) or `<file>` with `.lock` stripped (decrypt).
//! The output is written atomically via a temp file in the same directory.

use crate::cli::Cli;
use crate::password::read_password;
use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process,
};

// ── Entry point ────────────────────────────────────────────────────────────

/// Run non-interactive encrypt/decrypt.  Never returns — always calls `process::exit`.
pub fn run(cli: &Cli) -> ! {
    if cli.files.len() > 1 {
        eprintln!(
            "error: encrypt/decrypt takes exactly one file; got {}",
            cli.files.len()
        );
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

    // ── Determine mode and default output path ────────────────────────────
    let is_decrypt = input_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("lock"))
        .unwrap_or(false);

    let default_out: PathBuf = if is_decrypt {
        input_path.with_extension("") // strip the .lock extension
    } else {
        let fname = input_path
            .file_name()
            .expect("regular file must have a name")
            .to_string_lossy();
        input_path.with_file_name(format!("{}.lock", fname))
    };

    let out_path: &Path = cli.output.as_deref().unwrap_or(&default_out);

    // ── Check output collision ────────────────────────────────────────────
    if out_path.exists() && !cli.force {
        eprintln!(
            "error: output file already exists: {} (use -f to overwrite)",
            out_path.display()
        );
        process::exit(4);
    }

    // ── Read password ─────────────────────────────────────────────────────
    let password = read_password();

    // ── Open input file ───────────────────────────────────────────────────
    let mut input_file = match fs::File::open(input_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot open {}: {}", input_path.display(), e);
            process::exit(2);
        }
    };

    // Total bytes for progress (plaintext size for encrypt; ciphertext size for
    // decrypt — close enough for a progress bar since overhead is tiny).
    let file_size = input_path.metadata().map(|m| m.len()).unwrap_or(0);

    // ── Create temp file in the same directory (enables atomic rename) ────
    let out_parent = out_path.parent().unwrap_or(Path::new("."));
    let mut tmp = match tempfile::Builder::new()
        .prefix(".pnd_")
        .tempfile_in(out_parent)
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot create output file in {}: {}", out_parent.display(), e);
            process::exit(2);
        }
    };

    let stderr_tty = io::stderr().is_terminal();
    let op_label = if is_decrypt { "Decrypting" } else { "Encrypting" };
    let mut bytes_done: u64 = 0;

    // ── Run crypto ────────────────────────────────────────────────────────
    // Scope the mutable borrow of `tmp` so it ends before the `persist` call below.
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

    // Clear the progress line (if any)
    if stderr_tty {
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    }

    // ── Handle result ─────────────────────────────────────────────────────
    match result {
        Err(e) => {
            // I/O failure — NamedTempFile drops and auto-deletes the partial output
            drop(tmp);
            eprintln!("error: {}", e);
            process::exit(2);
        }
        Ok(false) => {
            // Wrong password / corrupt data — delete partial output
            drop(tmp);
            eprintln!("error: wrong password or corrupted file");
            process::exit(1);
        }
        Ok(true) => {
            // Success — persist temp file to final path (atomic rename)
            match tmp.persist(out_path) {
                Ok(_) => {
                    println!("{} → {}", input_path.display(), out_path.display());
                    process::exit(0);
                }
                Err(e) => {
                    // e.file is the NamedTempFile; dropping it cleans up the temp
                    drop(e.file);
                    eprintln!("error: could not write output: {}", e.error);
                    process::exit(2);
                }
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Print (or overwrite) a progress line on stderr when it is a TTY.

fn report_progress(tty: bool, label: &str, done: u64, total: u64) {
    if !tty || total == 0 {
        return;
    }
    let pct = (done * 100 / total).min(100);
    eprint!("\r{}… {}%", label, pct);
    let _ = io::stderr().flush();
}

