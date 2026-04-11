//! Smoke tests for --vault-init and the vault-export --stdout happy path
//! (which now uses --vault-init to bootstrap the vault non-interactively).

use std::process::Command;
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") { p.pop(); }
    p.push("pnd-cli");
    p
}

fn pnd(args: &[&str]) -> Command {
    let mut cmd = Command::new(bin());
    cmd.env("PND_PASSWORD", "testpass");
    for a in args { cmd.arg(a); }
    cmd
}

fn write_file(dir: &TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

// ── --vault-init ──────────────────────────────────────────────────────────

#[test]
fn vault_init_creates_index_lock() {
    let dir = TempDir::new().unwrap();
    let out = pnd(&["--vault-init", dir.path().to_str().unwrap()])
        .output().unwrap();

    assert!(out.status.success(),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.path().join("index.lock").exists(), "index.lock should exist");
}

#[test]
fn vault_init_with_blobs_dir() {
    let dir = TempDir::new().unwrap();
    let out = pnd(&["--vault-init", dir.path().to_str().unwrap(), "--blobs-dir", "blobs"])
        .output().unwrap();

    assert!(out.status.success(),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.path().join("index.lock").exists());
    assert!(dir.path().join("blobs").is_dir(), "blobs/ subdirectory should be created");
}

#[test]
fn vault_init_default_dir_requires_index_lock_free() {
    // Running --vault-init on a directory that already has index.lock → exit 4
    let dir = TempDir::new().unwrap();
    // First init succeeds
    pnd(&["--vault-init", dir.path().to_str().unwrap()])
        .output().unwrap();
    // Second init on the same dir must fail with exit 4
    let out = pnd(&["--vault-init", dir.path().to_str().unwrap()])
        .output().unwrap();
    assert_eq!(out.status.code(), Some(4),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn vault_init_nonexistent_dir_exits_2() {
    let out = pnd(&["--vault-init", "/tmp/pnd_no_such_dir_smoke_test"])
        .output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn vault_init_blobs_dir_path_separator_exits_3() {
    let dir = TempDir::new().unwrap();
    let out = pnd(&["--vault-init", dir.path().to_str().unwrap(), "--blobs-dir", "sub/dir"])
        .output().unwrap();
    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn vault_init_then_vault_list_works() {
    let dir = TempDir::new().unwrap();
    pnd(&["--vault-init", dir.path().to_str().unwrap()])
        .output().unwrap();

    let list = pnd(&["--vault-list", dir.path().to_str().unwrap()])
        .output().unwrap();
    assert!(list.status.success(),
        "stderr: {}", String::from_utf8_lossy(&list.stderr));
    // Empty vault — output should be empty or just a header
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(!stdout.contains("error"), "unexpected error in vault-list output");
}

// ── vault-export --stdout happy path (now possible via --vault-init) ──────

#[test]
fn vault_export_stdout_single_entry() {
    let vault_dir = TempDir::new().unwrap();
    let file_dir  = TempDir::new().unwrap();
    let content   = b"exported via stdout\n";
    let src = write_file(&file_dir, "note.txt", content);

    // Initialise vault
    pnd(&["--vault-init", vault_dir.path().to_str().unwrap()])
        .output().unwrap();

    // Add file to vault
    let add = pnd(&[
        "--vault-add", src.to_str().unwrap(),
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]).output().unwrap();
    assert!(add.status.success(),
        "vault-add stderr: {}", String::from_utf8_lossy(&add.stderr));

    // Export to stdout
    let export = pnd(&[
        "--vault-export", "note.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();

    assert!(export.status.success(),
        "stderr: {}", String::from_utf8_lossy(&export.stderr));
    assert_eq!(export.stdout, content);
}

#[test]
fn vault_export_stdout_folder_exits_3() {
    let vault_dir = TempDir::new().unwrap();
    let file_dir  = TempDir::new().unwrap();
    let src = write_file(&file_dir, "img.txt", b"data");

    pnd(&["--vault-init", vault_dir.path().to_str().unwrap()])
        .output().unwrap();

    pnd(&[
        "--vault-add", src.to_str().unwrap(),
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--vault-path", "photos",
    ]).output().unwrap();

    let out = pnd(&[
        "--vault-export", "photos",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();

    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
}
