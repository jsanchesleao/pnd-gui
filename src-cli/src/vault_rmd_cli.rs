//! Non-interactive vault rename, move, and delete (Phase 9).
//!
//! All three operations are pure index mutations (no blob I/O except delete,
//! which also removes the blob files from disk).  Each command decrypts
//! `index.lock`, mutates the in-memory index, then saves it atomically.

use crate::cli::Cli;
use crate::pages::vault::crypto::{open_vault, save_vault};
use crate::pages::vault::types::{VaultError, VaultHandle};
use crate::password::read_password;
use std::{
    io::{self, IsTerminal, Write as _},
    path::{Path, PathBuf},
    process,
};

// ── Public entry points ────────────────────────────────────────────────────

/// Run `--vault-rename <vault-path> <new-name>`. Never returns.
pub fn run_rename(cli: &Cli) -> ! {
    debug_assert_eq!(cli.vault_rename.len(), 2);
    let vault_path_arg = cli.vault_rename[0].trim_matches('/');
    let new_name = cli.vault_rename[1].as_str();
    let vault_dir = resolve_vault_dir(cli);

    // Reject names that contain a slash.
    if new_name.contains('/') {
        eprintln!(
            "error: name must not contain `/`; use --vault-move to change folder"
        );
        process::exit(3);
    }

    validate_vault_dir(&vault_dir);
    let password = read_password();
    let mut handle = open_vault_or_exit(&vault_dir, &password);

    // Find the entry to rename.
    let uuid = find_entry_uuid(&handle, vault_path_arg).unwrap_or_else(|| {
        eprintln!("error: entry not found in vault: {vault_path_arg}");
        process::exit(2);
    });

    let entry = handle.index.entries.get(&uuid).unwrap();

    // No-op check.
    if entry.name == new_name {
        println!("'{vault_path_arg}' is already named '{new_name}' — nothing to do.");
        process::exit(0);
    }

    // Collision check: another file with new_name in the same folder?
    let folder = entry.path.clone();
    let collision = handle.index.entries.iter().any(|(id, e)| {
        *id != uuid && e.path == folder && e.name == new_name
    });
    if collision {
        eprintln!("error: a file named '{new_name}' already exists here");
        process::exit(4);
    }

    // Mutate in memory.
    handle.index.entries.get_mut(&uuid).unwrap().name = new_name.to_string();

    // Save atomically.
    save_or_exit(&handle);

    let old_full = full_path_of_entry(handle.index.entries.get(&uuid).unwrap(), &folder);
    let new_full = if folder.is_empty() {
        new_name.to_string()
    } else {
        format!("{folder}/{new_name}")
    };
    println!("renamed: {old_full} → {new_full}");
    process::exit(0);
}

/// Run `--vault-move <vault-path> <dest-folder>`. Never returns.
pub fn run_move(cli: &Cli) -> ! {
    debug_assert_eq!(cli.vault_move.len(), 2);
    let vault_path_arg = cli.vault_move[0].trim_matches('/');
    let dest_folder = normalize_path(cli.vault_move[1].as_str());
    let new_name_opt = cli.name.as_deref();
    let vault_dir = resolve_vault_dir(cli);

    // Validate --name if given.
    if let Some(n) = new_name_opt {
        if n.contains('/') {
            eprintln!("error: name must not contain `/`");
            process::exit(3);
        }
    }

    validate_vault_dir(&vault_dir);
    let password = read_password();
    let mut handle = open_vault_or_exit(&vault_dir, &password);

    // Find the entry to move.
    let uuid = find_entry_uuid(&handle, vault_path_arg).unwrap_or_else(|| {
        eprintln!("error: entry not found in vault: {vault_path_arg}");
        process::exit(2);
    });

    let entry = handle.index.entries.get(&uuid).unwrap();
    let current_folder = entry.path.clone();
    let current_name = entry.name.clone();
    let final_name = new_name_opt.unwrap_or(&current_name).to_string();

    // No-op check.
    if dest_folder == current_folder && final_name == current_name {
        println!("'{vault_path_arg}' is already in '{dest_folder}' with that name — nothing to do.");
        process::exit(0);
    }

    // Collision check at destination.
    let collision = handle.index.entries.iter().any(|(id, e)| {
        *id != uuid && e.path == dest_folder && e.name == final_name
    });
    if collision {
        eprintln!(
            "error: a file named '{final_name}' already exists at vault path '{dest_folder}'"
        );
        process::exit(4);
    }

    // Mutate in memory.
    {
        let entry = handle.index.entries.get_mut(&uuid).unwrap();
        entry.path = dest_folder.clone();
        entry.name = final_name.clone();
    }

    // Save atomically.
    save_or_exit(&handle);

    let old_full = if current_folder.is_empty() {
        current_name.clone()
    } else {
        format!("{current_folder}/{current_name}")
    };
    let new_full = if dest_folder.is_empty() {
        final_name.clone()
    } else {
        format!("{dest_folder}/{final_name}")
    };
    println!("moved: {old_full} → {new_full}");
    process::exit(0);
}

/// Run `--vault-delete <vault-path>...`. Never returns.
pub fn run_delete(cli: &Cli) -> ! {
    let vault_paths: Vec<&str> = cli.vault_delete.iter().map(|s| s.trim_matches('/')).collect();
    let vault_dir = resolve_vault_dir(cli);

    validate_vault_dir(&vault_dir);
    let password = read_password();
    let mut handle = open_vault_or_exit(&vault_dir, &password);

    // Resolve all requested paths to UUIDs before mutating anything.
    let mut targets: Vec<(String, String)> = Vec::new(); // (uuid, full_vault_path)
    let mut any_not_found = false;

    for &vp in &vault_paths {
        match find_entry_uuid(&handle, vp) {
            Some(uuid) => {
                let entry = &handle.index.entries[&uuid];
                let full = full_path_of_entry(entry, &entry.path.clone());
                targets.push((uuid, full));
            }
            None => {
                eprintln!("warning: entry not found in vault: {vp}");
                any_not_found = true;
            }
        }
    }

    if targets.is_empty() {
        // Nothing to delete — all paths were missing.
        process::exit(if any_not_found { 2 } else { 0 });
    }

    // Confirmation prompt.
    if !cli.yes {
        if io::stdin().is_terminal() {
            eprint!("Delete {} item(s)? [y/N] ", targets.len());
            let _ = io::stderr().flush();
            let mut answer = String::new();
            io::stdin().read_line(&mut answer).ok();
            if !answer.trim().eq_ignore_ascii_case("y") {
                eprintln!("Aborted.");
                process::exit(0);
            }
        } else {
            eprintln!(
                "error: stdin is not a terminal; use `-y` to confirm deletion non-interactively"
            );
            process::exit(3);
        }
    }

    // Delete blobs and remove index entries, saving once at the end.
    for (uuid, full_path) in &targets {
        // Remove blob files (warn if missing but continue).
        if let Some(entry) = handle.index.entries.get(uuid) {
            for part in &entry.parts.clone() {
                let blob = handle.blob_path(&part.uuid);
                if let Err(e) = std::fs::remove_file(&blob) {
                    if e.kind() == io::ErrorKind::NotFound {
                        eprintln!(
                            "warning: blob file missing for '{full_path}' (vault may be corrupted)"
                        );
                    } else {
                        eprintln!("warning: could not remove blob for '{full_path}': {e}");
                    }
                }
            }
            // Also remove thumbnail blob if present.
            if let Some(ref thumb_uuid) = entry.thumbnail_uuid.clone() {
                let _ = std::fs::remove_file(handle.blob_path(thumb_uuid));
            }
        }
        // Remove from index.
        handle.index.entries.shift_remove(uuid);
        println!("deleted: {full_path}");
    }

    // Save index once after all deletions.
    save_or_exit(&handle);

    process::exit(if any_not_found { 2 } else { 0 });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Determine the vault directory from `--vault-dir` flag, falling back to `.`.
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
            eprintln!("error: {e}");
            process::exit(2);
        }
    }
}

/// Save the vault index, exiting on failure.
fn save_or_exit(handle: &VaultHandle) {
    if let Err(e) = save_vault(handle) {
        eprintln!("error: could not save vault index: {e}");
        process::exit(2);
    }
}

/// Return the UUID of the entry whose full virtual path matches `vault_path`.
/// Full path is `<entry.path>/<entry.name>`, or just `<entry.name>` at root.
fn find_entry_uuid(handle: &VaultHandle, vault_path: &str) -> Option<String> {
    let vault_path = vault_path.trim_matches('/');
    for (uuid, entry) in &handle.index.entries {
        let full = full_path_of_entry(entry, &entry.path);
        if full == vault_path {
            return Some(uuid.clone());
        }
    }
    None
}

/// Build the full virtual path string for an entry.
fn full_path_of_entry(entry: &crate::pages::vault::types::VaultEntry, folder: &str) -> String {
    if folder.is_empty() {
        entry.name.clone()
    } else {
        format!("{folder}/{}", entry.name)
    }
}

/// Strip leading and trailing slashes from a vault path component.
fn normalize_path(p: &str) -> String {
    p.trim_matches('/').to_string()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::vault::crypto::{create_vault, open_vault};
    use crate::pages::vault::types::VaultEntry;
    use tempfile::TempDir;

    fn make_vault_with_entries(
        dir: &TempDir,
        password: &str,
        entries: &[(&str, &str, &str)], // (uuid, name, path)
    ) -> VaultHandle {
        let mut handle = create_vault(dir.path(), None, password).expect("create_vault");
        for (uuid, name, path) in entries {
            handle.index.entries.insert(
                uuid.to_string(),
                VaultEntry {
                    name: name.to_string(),
                    path: path.to_string(),
                    size: 0,
                    parts: vec![],
                    thumbnail_uuid: None,
                    thumbnail_key_base64: None,
                },
            );
        }
        save_vault(&handle).expect("save_vault");
        handle
    }

    // ── find_entry_uuid ──────────────────────────────────────────────────────

    #[test]
    fn find_entry_at_root() {
        let dir = TempDir::new().unwrap();
        let handle = make_vault_with_entries(&dir, "pw", &[("u1", "notes.txt", "")]);
        assert_eq!(find_entry_uuid(&handle, "notes.txt"), Some("u1".to_string()));
    }

    #[test]
    fn find_entry_nested() {
        let dir = TempDir::new().unwrap();
        let handle =
            make_vault_with_entries(&dir, "pw", &[("u1", "beach.jpg", "photos/summer")]);
        assert_eq!(
            find_entry_uuid(&handle, "photos/summer/beach.jpg"),
            Some("u1".to_string())
        );
    }

    #[test]
    fn find_entry_not_found() {
        let dir = TempDir::new().unwrap();
        let handle = make_vault_with_entries(&dir, "pw", &[]);
        assert!(find_entry_uuid(&handle, "missing.txt").is_none());
    }

    // ── rename logic ─────────────────────────────────────────────────────────

    #[test]
    fn rename_updates_name_and_saves() {
        let dir = TempDir::new().unwrap();
        let mut handle =
            make_vault_with_entries(&dir, "pw", &[("u1", "old.txt", "docs")]);

        // Simulate the rename logic inline.
        handle.index.entries.get_mut("u1").unwrap().name = "new.txt".to_string();
        save_vault(&handle).unwrap();

        let reopened = open_vault(dir.path(), "pw").unwrap();
        let entry = reopened.index.entries.get("u1").unwrap();
        assert_eq!(entry.name, "new.txt");
        assert_eq!(entry.path, "docs");
    }

    #[test]
    fn rename_collision_detected() {
        let dir = TempDir::new().unwrap();
        let handle = make_vault_with_entries(
            &dir,
            "pw",
            &[("u1", "a.txt", ""), ("u2", "b.txt", "")],
        );
        // Trying to rename u1 to "b.txt" at root should be a collision.
        let uuid = find_entry_uuid(&handle, "a.txt").unwrap();
        let folder = handle.index.entries[&uuid].path.clone();
        let new_name = "b.txt";
        let collision = handle.index.entries.iter().any(|(id, e)| {
            *id != uuid && e.path == folder && e.name == new_name
        });
        assert!(collision);
    }

    // ── move logic ───────────────────────────────────────────────────────────

    #[test]
    fn move_updates_path_and_saves() {
        let dir = TempDir::new().unwrap();
        let mut handle =
            make_vault_with_entries(&dir, "pw", &[("u1", "photo.jpg", "photos")]);

        handle.index.entries.get_mut("u1").unwrap().path = "archive".to_string();
        save_vault(&handle).unwrap();

        let reopened = open_vault(dir.path(), "pw").unwrap();
        let entry = reopened.index.entries.get("u1").unwrap();
        assert_eq!(entry.path, "archive");
        assert_eq!(entry.name, "photo.jpg");
    }

    #[test]
    fn move_with_rename_updates_both() {
        let dir = TempDir::new().unwrap();
        let mut handle =
            make_vault_with_entries(&dir, "pw", &[("u1", "photo.jpg", "photos")]);

        {
            let e = handle.index.entries.get_mut("u1").unwrap();
            e.path = "archive".to_string();
            e.name = "renamed.jpg".to_string();
        }
        save_vault(&handle).unwrap();

        let reopened = open_vault(dir.path(), "pw").unwrap();
        let entry = reopened.index.entries.get("u1").unwrap();
        assert_eq!(entry.path, "archive");
        assert_eq!(entry.name, "renamed.jpg");
    }

    // ── delete logic ─────────────────────────────────────────────────────────

    #[test]
    fn delete_removes_index_entry() {
        let dir = TempDir::new().unwrap();
        let mut handle =
            make_vault_with_entries(&dir, "pw", &[("u1", "file.txt", ""), ("u2", "other.txt", "")]);

        handle.index.entries.shift_remove("u1");
        save_vault(&handle).unwrap();

        let reopened = open_vault(dir.path(), "pw").unwrap();
        assert!(!reopened.index.entries.contains_key("u1"));
        assert!(reopened.index.entries.contains_key("u2"));
    }

    // ── normalize_path ───────────────────────────────────────────────────────

    #[test]
    fn normalize_strips_slashes() {
        assert_eq!(normalize_path("/photos/summer/"), "photos/summer");
        assert_eq!(normalize_path(""), "");
        assert_eq!(normalize_path("/"), "");
    }
}
