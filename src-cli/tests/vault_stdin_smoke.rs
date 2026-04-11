//! Smoke tests for Phase 10-D: reading from stdin for `--vault-add`.
//!
//! All tests bootstrap a vault with `--vault-init` and use `PND_PASSWORD` to
//! avoid interactive prompts, following the pattern established in
//! `vault_init_smoke.rs`.

use std::process::{Command, Stdio};
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

/// Spawn with piped stdin; write `stdin_bytes`, collect output.
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

/// Initialise a vault and return its directory.
fn init_vault(dir: &TempDir) {
    let out = pnd(&["--vault-init", dir.path().to_str().unwrap()])
        .output().unwrap();
    assert!(out.status.success(),
        "vault-init failed: {}", String::from_utf8_lossy(&out.stderr));
}

// ── Test 1: spec acceptance criterion ────────────────────────────────────
//
//   echo "secret" | pnd-cli --vault-add - --name secret.txt
//   pnd-cli --vault-export secret.txt --stdout  →  prints "secret\n"

#[test]
fn spec_acceptance_criterion() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let content = b"secret\n";

    // Add from stdin.
    let add = pnd_piped(content, &[
        "--vault-add", "-",
        "--name", "secret.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert!(add.status.success(),
        "vault-add failed: {}", String::from_utf8_lossy(&add.stderr));

    // Export back to stdout and verify content.
    let export = pnd(&[
        "--vault-export", "secret.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();
    assert!(export.status.success(),
        "vault-export failed: {}", String::from_utf8_lossy(&export.stderr));
    assert_eq!(export.stdout, content);
}

// ── Test 2: binary content round-trips correctly ──────────────────────────

#[test]
fn binary_content_roundtrip() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let original: Vec<u8> = (0u16..1024).flat_map(|i| i.to_le_bytes()).collect();

    let add = pnd_piped(&original, &[
        "--vault-add", "-",
        "--name", "data.bin",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert!(add.status.success(),
        "vault-add failed: {}", String::from_utf8_lossy(&add.stderr));

    let export = pnd(&[
        "--vault-export", "data.bin",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();
    assert!(export.status.success());
    assert_eq!(export.stdout, original);
}

// ── Test 3: entry appears in --vault-list ────────────────────────────────

#[test]
fn entry_visible_in_vault_list() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let add = pnd_piped(b"hello", &[
        "--vault-add", "-",
        "--name", "note.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert!(add.status.success());

    let list = pnd(&[
        "--vault-list", vault_dir.path().to_str().unwrap(),
    ]).output().unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("note.txt"),
        "expected 'note.txt' in vault list, got:\n{stdout}");
}

// ── Test 4: missing --name → exit 3 ──────────────────────────────────────

#[test]
fn missing_name_exits_3() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let out = pnd_piped(b"data", &[
        "--vault-add", "-",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--name"),
        "expected mention of --name in error, got: {stderr}");
}

// ── Test 5: stdin mixed with real files → exit 3 ─────────────────────────

#[test]
fn stdin_mixed_with_files_exits_3() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    // Pass "-" alongside a real path; clap accepts multiple values for vault_add.
    let out = pnd_piped(b"data", &[
        "--vault-add", "-", "somefile.txt",
        "--name", "stdin.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(3),
        "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot combine"),
        "expected 'cannot combine' error, got: {stderr}");
}

// ── Test 6: collision without --force → exit 2 ───────────────────────────

#[test]
fn collision_without_force_exits_2() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let args = &[
        "--vault-add", "-",
        "--name", "dup.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ];

    let first = pnd_piped(b"version 1", args);
    assert!(first.status.success());

    let second = pnd_piped(b"version 2", args);
    assert_eq!(second.status.code(), Some(2),
        "stderr: {}", String::from_utf8_lossy(&second.stderr));
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("already exists"),
        "expected collision error, got: {stderr}");
}

// ── Test 7: --force replaces existing entry ───────────────────────────────

#[test]
fn force_replaces_existing_entry() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let base_args = &[
        "--vault-add", "-",
        "--name", "overwrite.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ];

    let first = pnd_piped(b"original content", base_args);
    assert!(first.status.success());

    // Second add with --force.
    let force_args = &[
        "--vault-add", "-",
        "--name", "overwrite.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "-f",
    ];
    let second = pnd_piped(b"replaced content", force_args);
    assert!(second.status.success(),
        "stderr: {}", String::from_utf8_lossy(&second.stderr));

    // Export should yield the new content.
    let export = pnd(&[
        "--vault-export", "overwrite.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();
    assert!(export.status.success());
    assert_eq!(export.stdout, b"replaced content");
}

// ── Test 8: empty stdin adds an empty entry ───────────────────────────────

#[test]
fn empty_stdin_adds_empty_entry() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let add = pnd_piped(b"", &[
        "--vault-add", "-",
        "--name", "empty.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert!(add.status.success(),
        "stderr: {}", String::from_utf8_lossy(&add.stderr));

    let export = pnd(&[
        "--vault-export", "empty.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();
    assert!(export.status.success());
    assert!(export.stdout.is_empty(), "expected empty output");
}

// ── Test 9: wrong password → exit 1 ──────────────────────────────────────

#[test]
fn wrong_password_exits_1() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let mut child = Command::new(bin())
        .env("PND_PASSWORD", "wrongpass")
        .args([
            "--vault-add", "-",
            "--name", "secret.txt",
            "--vault-dir", vault_dir.path().to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().unwrap();

    use std::io::Write as _;
    child.stdin.take().unwrap().write_all(b"data").unwrap();
    let result = child.wait_with_output().unwrap();

    assert_eq!(result.status.code(), Some(1),
        "stderr: {}", String::from_utf8_lossy(&result.stderr));
}

// ── Test 10: stdin add into a virtual subfolder ──────────────────────────

#[test]
fn stdin_add_to_virtual_subfolder() {
    let vault_dir = TempDir::new().unwrap();
    init_vault(&vault_dir);

    let add = pnd_piped(b"doc content", &[
        "--vault-add", "-",
        "--name", "report.txt",
        "--vault-path", "documents",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
    ]);
    assert!(add.status.success(),
        "stderr: {}", String::from_utf8_lossy(&add.stderr));

    // List should show the nested path.
    let list = pnd(&[
        "--vault-list", vault_dir.path().to_str().unwrap(),
    ]).output().unwrap();
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("report.txt"),
        "expected 'report.txt' in vault list, got:\n{stdout}");

    // Export via the full path.
    let export = pnd(&[
        "--vault-export", "documents/report.txt",
        "--vault-dir", vault_dir.path().to_str().unwrap(),
        "--stdout",
    ]).output().unwrap();
    assert!(export.status.success(),
        "stderr: {}", String::from_utf8_lossy(&export.stderr));
    assert_eq!(export.stdout, b"doc content");
}
