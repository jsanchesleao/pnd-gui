//! Non-interactive vault listing (Phase 6).
//!
//! Decrypts `index.lock`, optionally filters by virtual folder, and prints
//! each entry either as human-readable text or as a JSON array.

use crate::cli::Cli;
use crate::pages::vault::crypto::open_vault;
use crate::pages::vault::types::VaultError;
use crate::password::read_password;
use std::{path::Path, process};

// ── Entry point ────────────────────────────────────────────────────────────

/// Run non-interactive vault list.  Never returns — always calls `process::exit`.
pub fn run(cli: &Cli) -> ! {
    let vault_dir = cli.vault_list.as_ref().unwrap();

    // ── Validate vault directory ──────────────────────────────────────────
    validate_vault_dir(vault_dir);

    // ── Read password ─────────────────────────────────────────────────────
    let password = read_password();

    // ── Open vault ────────────────────────────────────────────────────────
    let handle = match open_vault(vault_dir, &password) {
        Ok(h) => h,
        Err(VaultError::WrongPassword) => {
            eprintln!("error: wrong password or corrupted index");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(2);
        }
    };

    // ── Filter entries by --path ──────────────────────────────────────────
    let path_filter = cli.path.as_deref().map(normalize_vault_path);

    let entries: Vec<_> = handle
        .index
        .entries
        .values()
        .filter(|e| {
            if let Some(ref filter) = path_filter {
                // Exact folder match or recursive sub-folder match.
                e.path == *filter || e.path.starts_with(&format!("{}/", filter))
            } else {
                true
            }
        })
        .collect();

    // ── Print output ──────────────────────────────────────────────────────
    if cli.json {
        print_json(&entries);
    } else {
        print_human(&entries);
    }

    process::exit(0);
}

// ── Output formatters ──────────────────────────────────────────────────────

fn print_human(entries: &[&crate::pages::vault::types::VaultEntry]) {
    if entries.is_empty() {
        return;
    }

    // Build full virtual paths (folder/name) for alignment.
    let paths: Vec<String> = entries
        .iter()
        .map(|e| full_path(&e.path, &e.name))
        .collect();

    let col_width = paths.iter().map(|p| p.len()).max().unwrap_or(0);

    for (path, entry) in paths.iter().zip(entries.iter()) {
        println!("{:<width$}   ({})", path, format_size(entry.size), width = col_width);
    }
}

fn print_json(entries: &[&crate::pages::vault::types::VaultEntry]) {
    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "path": full_path(&e.path, &e.name),
                "name": e.name,
                "size": e.size,
            })
        })
        .collect();

    match serde_json::to_string(&items) {
        Ok(s) => println!("{}", s),
        Err(e) => {
            eprintln!("error: failed to serialize JSON: {}", e);
            process::exit(2);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Validate that `dir` exists, is a directory, and contains `index.lock`.
/// Exits with an appropriate code on failure.
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

/// Normalise a virtual vault path: strip leading and trailing slashes.
fn normalize_vault_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

/// Build the display path for an entry: `folder/name` or just `name` at root.
fn full_path(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", folder, name)
    }
}

/// Format a byte count as a human-readable string (B / KB / MB / GB).
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
