//! Smoke tests for Phase 10-B: reading from stdin for encrypt/decrypt (`--mode` / `-m`).

use std::process::{Command, Stdio};
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") { p.pop(); }
    p.push("pnd-cli");
    p
}

/// Spawn pnd-cli with PND_PASSWORD set, piped stdin, and any extra args.
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
    child.stdin.take().unwrap().write_all(stdin_bytes).unwrap();
    child.wait_with_output().unwrap()
}

fn write_file(dir: &TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

// ── Test 1: stdin encrypt → stdout round-trip ─────────────────────────────

#[test]
fn stdin_encrypt_stdout_roundtrip() {
    let original = b"hello from stdin";

    // Encrypt stdin → stdout
    let enc = pnd_piped(original, &["-m", "encrypt", "--stdout"]);
    assert!(enc.status.success(),
        "encrypt stderr: {}", String::from_utf8_lossy(&enc.stderr));
    assert!(!enc.stdout.is_empty());
    assert_ne!(enc.stdout, original);

    // Decrypt ciphertext back via stdin → stdout
    let dec = pnd_piped(&enc.stdout, &["-m", "decrypt", "--stdout"]);
    assert!(dec.status.success(),
        "decrypt stderr: {}", String::from_utf8_lossy(&dec.stderr));
    assert_eq!(dec.stdout, original);
}

// ── Test 2: mode aliases work (enc / e / dec / d) ─────────────────────────

#[test]
fn mode_aliases_work() {
    let original = b"alias test";

    let enc = pnd_piped(original, &["-m", "enc", "--stdout"]);
    assert!(enc.status.success());

    let dec = pnd_piped(&enc.stdout, &["-m", "d", "--stdout"]);
    assert!(dec.status.success());
    assert_eq!(dec.stdout, original);
}

// ── Test 3: missing --mode when stdin is piped → exit 3 ───────────────────

#[test]
fn missing_mode_exits_3() {
    let out = pnd_piped(b"data", &[]); // no -m
    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("-m"), "expected hint about -m in error message");
}

// ── Test 4: implicit stdout when stdin piped and no -o ────────────────────

#[test]
fn implicit_stdout_when_no_o() {
    let original = b"implicit stdout";

    // No --stdout flag: output should still appear on stdout
    let enc = pnd_piped(original, &["-m", "encrypt"]);
    assert!(enc.status.success(),
        "stderr: {}", String::from_utf8_lossy(&enc.stderr));
    assert!(!enc.stdout.is_empty(), "expected ciphertext on stdout");

    let dec = pnd_piped(&enc.stdout, &["-m", "decrypt"]);
    assert!(dec.status.success());
    assert_eq!(dec.stdout, original);
}

// ── Test 5: stdin + -o writes to named file, nothing on stdout ────────────

#[test]
fn stdin_with_o_writes_file() {
    let dir = TempDir::new().unwrap();
    let out_path = dir.path().join("out.lock");
    let original = b"write to file";

    let enc = pnd_piped(original, &[
        "-m", "encrypt",
        "-o", out_path.to_str().unwrap(),
    ]);
    assert!(enc.status.success(),
        "stderr: {}", String::from_utf8_lossy(&enc.stderr));
    assert!(out_path.exists(), "output file should have been created");
    // stdout carries the "stdin → <file>" success line, not ciphertext data.
    let stdout_str = String::from_utf8_lossy(&enc.stdout);
    assert!(stdout_str.contains("stdin →"), "expected success line on stdout, got: {stdout_str}");

    // Decrypt the file back to verify correctness
    let dec = Command::new(bin())
        .env("PND_PASSWORD", "testpass")
        .args([out_path.to_str().unwrap(), "--stdout"])
        .output().unwrap();
    assert!(dec.status.success());
    assert_eq!(dec.stdout, original);
}

// ── Test 6: wrong password during stdin decrypt → exit 1 ──────────────────

#[test]
fn wrong_password_stdin_exits_1() {
    // Encrypt with "testpass"
    let enc = pnd_piped(b"secret", &["-m", "encrypt", "--stdout"]);
    assert!(enc.status.success());

    // Decrypt with wrong password
    use std::io::Write as _;
    let mut child2 = Command::new(bin())
        .env("PND_PASSWORD", "wrongpass")
        .args(["-m", "decrypt", "--stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().unwrap();
    child2.stdin.take().unwrap().write_all(&enc.stdout).unwrap();
    let result = child2.wait_with_output().unwrap();

    assert_eq!(result.status.code(), Some(1),
        "stderr: {}", String::from_utf8_lossy(&result.stderr));
}

// ── Test 7: explicit "-" argument treated as stdin ────────────────────────

#[test]
fn explicit_dash_is_stdin() {
    let original = b"explicit dash";

    let enc = pnd_piped(original, &["-", "-m", "encrypt", "--stdout"]);
    assert!(enc.status.success(),
        "stderr: {}", String::from_utf8_lossy(&enc.stderr));

    let dec = pnd_piped(&enc.stdout, &["-", "-m", "decrypt", "--stdout"]);
    assert!(dec.status.success());
    assert_eq!(dec.stdout, original);
}

// ── Test 8: large-ish input round-trips correctly (multi-frame) ───────────

#[test]
fn large_stdin_roundtrip() {
    // 2 MB of deterministic data — enough to span multiple 64 MiB frames in theory,
    // but exercises the streaming path end-to-end.
    let original: Vec<u8> = (0u32..524_288).flat_map(|i| i.to_le_bytes()).collect();

    let enc = pnd_piped(&original, &["-m", "encrypt", "--stdout"]);
    assert!(enc.status.success());

    let dec = pnd_piped(&enc.stdout, &["-m", "decrypt", "--stdout"]);
    assert!(dec.status.success());
    assert_eq!(dec.stdout, original);
}

// ── Test 9: stdin + file collision check with -o and existing file ─────────

#[test]
fn stdin_o_collision_exits_4() {
    let dir = TempDir::new().unwrap();
    let existing = write_file(&dir, "exists.lock", b"already here");

    let out = pnd_piped(b"data", &[
        "-m", "encrypt",
        "-o", existing.to_str().unwrap(),
        // no -f
    ]);
    assert_eq!(out.status.code(), Some(4),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

// ── Test 10: acceptance criterion from the spec ───────────────────────────

#[test]
fn spec_acceptance_criterion() {
    // "echo hello | PND_PASSWORD=pw pnd-cli -m encrypt --stdout |
    //  PND_PASSWORD=pw pnd-cli -m decrypt --stdout" prints "hello\n"
    let enc = pnd_piped(b"hello\n", &["-m", "encrypt", "--stdout"]);
    assert!(enc.status.success());

    let dec = pnd_piped(&enc.stdout, &["-m", "decrypt", "--stdout"]);
    assert!(dec.status.success());
    assert_eq!(dec.stdout, b"hello\n");
}
