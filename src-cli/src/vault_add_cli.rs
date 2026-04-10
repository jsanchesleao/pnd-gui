//! Non-interactive vault-add (Phase 8).
//!
//! Encrypts one or more local files and adds them to the vault index.
//! On multi-file add, files successfully added before a failure are kept;
//! the failed file and any remaining files are skipped and the exit code is 2.

use crate::cli::Cli;
use crate::pages::vault::crypto::{encrypt_file_to_vault, open_vault, save_vault};
use crate::pages::vault::types::{VaultError, VaultHandle};
use crate::password::read_password;
use std::{
    path::{Path, PathBuf},
    process,
};

// ── Public entry point ─────────────────────────────────────────────────────

/// Run `--vault-add`. Never returns.
pub fn run_add(cli: &Cli) -> ! {
    let files = &cli.vault_add;
    let vault_dir = resolve_vault_dir(cli);
    let vault_path = normalize_vault_path(cli.vault_path.as_deref().unwrap_or(""));

    validate_vault_dir(&vault_dir);

    let password = read_password();
    let mut handle = open_vault_or_exit(&vault_dir, &password);

    let mut any_error = false;

    for file in files {
        match add_one_file(&mut handle, file, &vault_path, cli.force) {
            Ok(added_path) => {
                println!("{} → vault:{}", file.display(), added_path);
            }
            Err(AddError::FileNotFound) => {
                eprintln!("error: file not found: {}", file.display());
                any_error = true;
            }
            Err(AddError::IsDirectory) => {
                eprintln!("error: {} is a directory, skipping", file.display());
                any_error = true;
            }
            Err(AddError::NameCollision(name)) => {
                eprintln!(
                    "error: a file named '{}' already exists at vault:{} (use -f to replace)",
                    name, vault_path
                );
                any_error = true;
            }
            Err(AddError::Vault(msg)) => {
                eprintln!("error: {}: {}", file.display(), msg);
                any_error = true;
            }
        }
    }

    process::exit(if any_error { 2 } else { 0 });
}

// ── Per-file logic ─────────────────────────────────────────────────────────

#[derive(Debug)]
enum AddError {
    FileNotFound,
    IsDirectory,
    /// The name of the colliding file.
    NameCollision(String),
    /// Any other vault or I/O error.
    Vault(String),
}

/// Encrypt `file_path` and insert it into the vault at `vault_path`.
///
/// If `force` is true and a file with the same name already exists at
/// `vault_path`, the old entry (and its blob files) are removed before adding.
///
/// On success, saves the updated index atomically and returns the full vault path.
fn add_one_file(
    handle: &mut VaultHandle,
    file_path: &Path,
    vault_path: &str,
    force: bool,
) -> Result<String, AddError> {
    if !file_path.exists() {
        return Err(AddError::FileNotFound);
    }
    if file_path.is_dir() {
        return Err(AddError::IsDirectory);
    }

    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());

    // Collision detection.
    let existing_uuid = find_entry_by_name(handle, vault_path, &name);
    if let Some(ref uuid) = existing_uuid {
        if !force {
            return Err(AddError::NameCollision(name));
        }
        // --force: remove old blobs from disk then drop the index entry.
        remove_entry_blobs(handle, uuid);
        handle.index.entries.shift_remove(uuid);
    }

    // Encrypt and write blob files to the vault.
    let (file_uuid, entry) =
        encrypt_file_to_vault(file_path, &handle.blobs_dir, vault_path)
            .map_err(|e| AddError::Vault(e.to_string()))?;

    let full_path = if vault_path.is_empty() {
        entry.name.clone()
    } else {
        format!("{}/{}", vault_path, entry.name)
    };

    handle.index.entries.insert(file_uuid, entry);

    // Save the updated index atomically after each successful add so that
    // earlier files are persisted even if a later file fails.
    save_vault(handle).map_err(|e| AddError::Vault(e.to_string()))?;

    Ok(full_path)
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Find the UUID of the entry with the given `name` at the given `path`.
/// Returns `None` when no match exists.
fn find_entry_by_name(handle: &VaultHandle, path: &str, name: &str) -> Option<String> {
    handle
        .index
        .entries
        .iter()
        .find(|(_, e)| e.path == path && e.name == name)
        .map(|(uuid, _)| uuid.clone())
}

/// Delete the blob files for `uuid` from disk.
/// Silently ignores missing files — the index entry will be removed regardless.
fn remove_entry_blobs(handle: &VaultHandle, uuid: &str) {
    if let Some(entry) = handle.index.entries.get(uuid) {
        for part in &entry.parts {
            let _ = std::fs::remove_file(handle.blob_path(&part.uuid));
        }
        if let Some(thumb_uuid) = &entry.thumbnail_uuid {
            let _ = std::fs::remove_file(handle.blob_path(thumb_uuid));
        }
    }
}

/// Normalise a vault path: strip leading and trailing slashes.
fn normalize_vault_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

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

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::vault::crypto::{create_vault, open_vault};
    use crate::pages::vault::types::VaultEntry;
    use tempfile::TempDir;

    fn make_vault(dir: &TempDir, password: &str) -> VaultHandle {
        create_vault(dir.path(), None, password).expect("create_vault")
    }

    fn write_file(dir: &TempDir, name: &str, contents: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, contents).expect("write_file");
        path
    }

    // ── normalize_vault_path ─────────────────────────────────────────────────

    #[test]
    fn normalize_strips_slashes() {
        assert_eq!(normalize_vault_path("/photos/"), "photos");
        assert_eq!(normalize_vault_path("photos/summer/"), "photos/summer");
        assert_eq!(normalize_vault_path(""), "");
        assert_eq!(normalize_vault_path("/"), "");
    }

    // ── find_entry_by_name ───────────────────────────────────────────────────

    #[test]
    fn find_entry_by_name_found() {
        let dir = TempDir::new().unwrap();
        let mut handle = make_vault(&dir, "pw");
        handle.index.entries.insert(
            "uuid-1".to_string(),
            VaultEntry {
                name: "photo.jpg".to_string(),
                path: "photos".to_string(),
                size: 0,
                parts: vec![],
                thumbnail_uuid: None,
                thumbnail_key_base64: None,
            },
        );
        assert_eq!(
            find_entry_by_name(&handle, "photos", "photo.jpg"),
            Some("uuid-1".to_string())
        );
    }

    #[test]
    fn find_entry_by_name_not_found() {
        let dir = TempDir::new().unwrap();
        let handle = make_vault(&dir, "pw");
        assert_eq!(find_entry_by_name(&handle, "", "missing.txt"), None);
    }

    #[test]
    fn find_entry_by_name_wrong_path() {
        let dir = TempDir::new().unwrap();
        let mut handle = make_vault(&dir, "pw");
        handle.index.entries.insert(
            "uuid-1".to_string(),
            VaultEntry {
                name: "file.txt".to_string(),
                path: "docs".to_string(),
                size: 0,
                parts: vec![],
                thumbnail_uuid: None,
                thumbnail_key_base64: None,
            },
        );
        // Same name but different vault path → no match.
        assert_eq!(find_entry_by_name(&handle, "other", "file.txt"), None);
    }

    // ── add_one_file ─────────────────────────────────────────────────────────

    #[test]
    fn add_file_to_vault_root() {
        let vault_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");

        let file = write_file(&files_dir, "hello.txt", b"hello world");
        let result = add_one_file(&mut handle, &file, "", false);
        assert!(result.is_ok(), "{:?}", result.err());
        assert_eq!(result.unwrap(), "hello.txt");

        // Entry should be in the index.
        assert!(handle.index.entries.values().any(|e| e.name == "hello.txt" && e.path.is_empty()));

        // Index should have been saved.
        let reopened = open_vault(vault_dir.path(), "pw").unwrap();
        assert!(reopened.index.entries.values().any(|e| e.name == "hello.txt"));
    }

    #[test]
    fn add_file_to_nested_path() {
        let vault_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");

        let file = write_file(&files_dir, "pic.jpg", b"jpeg bytes");
        let result = add_one_file(&mut handle, &file, "photos/summer", false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "photos/summer/pic.jpg");

        let entry = handle.index.entries.values().find(|e| e.name == "pic.jpg").unwrap();
        assert_eq!(entry.path, "photos/summer");
    }

    #[test]
    fn add_missing_file_returns_error() {
        let vault_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");
        let result = add_one_file(&mut handle, Path::new("/nonexistent/file.txt"), "", false);
        assert!(matches!(result, Err(AddError::FileNotFound)));
    }

    #[test]
    fn add_directory_returns_error() {
        let vault_dir = TempDir::new().unwrap();
        let dir_to_add = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");
        let result = add_one_file(&mut handle, dir_to_add.path(), "", false);
        assert!(matches!(result, Err(AddError::IsDirectory)));
    }

    #[test]
    fn add_collision_without_force_returns_error() {
        let vault_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");

        let file = write_file(&files_dir, "doc.pdf", b"version 1");
        add_one_file(&mut handle, &file, "", false).unwrap();

        // Second add of the same name without --force.
        let result = add_one_file(&mut handle, &file, "", false);
        assert!(matches!(result, Err(AddError::NameCollision(_))));
    }

    #[test]
    fn add_collision_with_force_replaces() {
        let vault_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");

        let file = write_file(&files_dir, "doc.pdf", b"version 1");
        add_one_file(&mut handle, &file, "", false).unwrap();

        // Overwrite the file contents on disk so we can tell them apart.
        std::fs::write(&file, b"version 2").unwrap();

        let result = add_one_file(&mut handle, &file, "", true);
        assert!(result.is_ok());

        // There should be exactly one entry named "doc.pdf".
        let matches: Vec<_> = handle
            .index
            .entries
            .values()
            .filter(|e| e.name == "doc.pdf")
            .collect();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn add_force_removes_old_blobs() {
        let vault_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");

        let file = write_file(&files_dir, "data.bin", b"original");
        add_one_file(&mut handle, &file, "", false).unwrap();

        // Collect old blob UUIDs.
        let old_parts: Vec<String> = handle
            .index
            .entries
            .values()
            .find(|e| e.name == "data.bin")
            .unwrap()
            .parts
            .iter()
            .map(|p| p.uuid.clone())
            .collect();

        std::fs::write(&file, b"updated").unwrap();
        add_one_file(&mut handle, &file, "", true).unwrap();

        // Old blob files should be gone.
        for uuid in &old_parts {
            assert!(
                !vault_dir.path().join(uuid).exists(),
                "old blob {uuid} should have been deleted"
            );
        }
    }

    #[test]
    fn add_multiple_files_independent() {
        let vault_dir = TempDir::new().unwrap();
        let files_dir = TempDir::new().unwrap();
        let mut handle = make_vault(&vault_dir, "pw");

        let f1 = write_file(&files_dir, "a.txt", b"aaa");
        let f2 = write_file(&files_dir, "b.txt", b"bbb");

        add_one_file(&mut handle, &f1, "", false).unwrap();
        add_one_file(&mut handle, &f2, "", false).unwrap();

        let names: Vec<_> = handle.index.entries.values().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
    }
}
