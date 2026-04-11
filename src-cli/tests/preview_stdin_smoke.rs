//! Smoke tests for Phase 10-C: reading from stdin for `-p` (`--ext`).
//!
//! These tests invoke the compiled `pnd-cli` binary with `stdin(Stdio::piped())`
//! to exercise the stdin preview path without requiring a real terminal.
//!
//! Because preview rendering requires a TTY (Kitty protocol, mpv, bat), we
//! cannot fully verify the visual output in CI.  Instead we:
//!   - Verify that the process exits with code 0 or the expected non-zero code.
//!   - Verify that the correct error messages appear on stderr for invalid inputs.

use std::process::{Command, Stdio};
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") { p.pop(); }
    p.push("pnd-cli");
    p
}

/// Spawn pnd-cli with PND_PASSWORD set, piped stdin, and extra args.
fn pnd_piped(stdin_bytes: &[u8], args: &[&str]) -> std::process::Output {
    let mut child = Command::new(bin())
        .env("PND_PASSWORD", "testpass")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write as _;
    // Ignore broken-pipe: child may exit early on a usage error before reading stdin.
    let _ = child.stdin.take().unwrap().write_all(stdin_bytes);
    child.wait_with_output().unwrap()
}

fn write_file(dir: &TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

// ── Test 1: missing --ext when piping to -p → exit 3 ─────────────────────

#[test]
fn missing_ext_exits_3() {
    let out = pnd_piped(b"some bytes", &["-p"]);
    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--ext"), "expected mention of --ext in error, got: {stderr}");
}

// ── Test 2: pipe plain text with -p --ext txt → exit 0 ───────────────────

#[test]
fn stdin_plain_text_preview_exits_0() {
    // Plain text preview may fall back to "No previewer" (exit 0) or open bat/viewer.
    // In a non-TTY CI environment the terminal setup itself fails with exit 2 and
    // "could not enable raw mode" — that is also acceptable here.
    let out = pnd_piped(b"Hello, world!\n", &["-p", "--ext", "txt"]);
    let code = out.status.code().unwrap_or(99);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Accept exit 2 only when it is the terminal-not-available error, not a logic error.
    let terminal_unavailable = stderr.contains("could not enable raw mode");
    assert!(
        code == 0 || code == 1 || (code == 2 && terminal_unavailable),
        "expected exit 0/1 (or exit 2 from terminal setup in CI), got {code}\nstderr: {stderr}"
    );
}

// ── Test 3: pipe encrypted text with -p --ext txt -m decrypt → decrypts ───

#[test]
fn stdin_encrypted_text_preview() {
    let dir = TempDir::new().unwrap();
    let src = write_file(&dir, "note.txt", b"secret preview content\n");
    let lock = dir.path().join("note.txt.lock");

    // Encrypt to a file first.
    let enc = Command::new(bin())
        .env("PND_PASSWORD", "testpass")
        .args([src.to_str().unwrap(), "-o", lock.to_str().unwrap()])
        .output().unwrap();
    assert!(enc.status.success(), "encrypt failed: {}", String::from_utf8_lossy(&enc.stderr));

    let ciphertext = std::fs::read(&lock).unwrap();

    // Pipe ciphertext with -m decrypt --ext txt; should decrypt and attempt preview.
    let out = pnd_piped(&ciphertext, &["-p", "--ext", "txt", "-m", "decrypt"]);
    let code = out.status.code().unwrap_or(99);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Must NOT be a wrong-password failure (we used the correct password).
    assert_ne!(code, 1,
        "unexpected wrong-password error\nstderr: {stderr}");

    // Accept exit 2 when it is the terminal-not-available error in CI.
    let terminal_unavailable = stderr.contains("could not enable raw mode");
    assert!(
        code == 0 || (code == 2 && terminal_unavailable),
        "expected exit 0 (or exit 2 from terminal setup in CI), got {code}\nstderr: {stderr}"
    );
}

// ── Test 4: wrong password during stdin decrypt → exit 1 ─────────────────

#[test]
fn stdin_preview_wrong_password_exits_1() {
    let dir = TempDir::new().unwrap();
    let src = write_file(&dir, "secret.txt", b"data");
    let lock = dir.path().join("secret.txt.lock");

    let enc = Command::new(bin())
        .env("PND_PASSWORD", "correctpass")
        .args([src.to_str().unwrap(), "-o", lock.to_str().unwrap()])
        .output().unwrap();
    assert!(enc.status.success());

    let ciphertext = std::fs::read(&lock).unwrap();

    let mut child = Command::new(bin())
        .env("PND_PASSWORD", "wrongpass")
        .args(["-p", "--ext", "txt", "-m", "decrypt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().unwrap();

    use std::io::Write as _;
    child.stdin.take().unwrap().write_all(&ciphertext).unwrap();
    let result = child.wait_with_output().unwrap();

    assert_eq!(result.status.code(), Some(1),
        "stderr: {}", String::from_utf8_lossy(&result.stderr));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("wrong password"), "expected 'wrong password' in stderr, got: {stderr}");
}

// ── Test 5: explicit "-" argument triggers stdin path ────────────────────

#[test]
fn explicit_dash_triggers_stdin_path() {
    // Without --ext the error message must mention --ext, not "requires a file argument".
    let out = pnd_piped(b"data", &["-p", "-"]);
    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--ext"),
        "expected --ext error when using '-', got: {stderr}");
}

// ── Test 6: --ext value is lowercased (TXT same as txt) ──────────────────

#[test]
fn ext_is_case_insensitive() {
    // Both "TXT" and "txt" should work without a usage error (exit 3).
    // Exit 2 is acceptable when the failure is terminal-not-available in CI.
    let out = pnd_piped(b"hello\n", &["-p", "--ext", "TXT"]);
    let code = out.status.code().unwrap_or(99);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let terminal_unavailable = stderr.contains("could not enable raw mode");
    assert!(
        code == 0 || code == 1 || (code == 2 && terminal_unavailable),
        "expected exit 0/1 (or exit 2 from terminal setup in CI) for --ext TXT, got {code}\nstderr: {stderr}"
    );
}
