//! Non-interactive vault initialisation (`--vault-init`).
//!
//! Creates a new empty vault in an existing directory by writing an encrypted
//! `index.lock` file. Mirrors the three-field TUI create-vault form:
//!   1. vault directory  (positional / --vault-init argument)
//!   2. blobs subdirectory name  (--blobs-dir, optional)
//!   3. master password  (prompted twice to prevent typos)

use crate::cli::Cli;
use crate::pages::vault::crypto::create_vault;
use crate::pages::vault::types::VaultError;
use crate::password::read_password_with_confirm;
use std::process;

// ── Entry point ────────────────────────────────────────────────────────────

/// Run `--vault-init`. Never returns — always calls `process::exit`.
pub fn run(cli: &Cli) -> ! {
    let vault_dir = cli.vault_init.as_deref().unwrap();

    // ── Validate blobs_dir name ───────────────────────────────────────────
    if let Some(name) = &cli.blobs_dir {
        if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
            eprintln!(
                "error: --blobs-dir must be a single directory name, not a path (got {:?})",
                name
            );
            process::exit(3);
        }
        if name.is_empty() {
            eprintln!("error: --blobs-dir must not be empty; omit the flag to store blobs alongside index.lock");
            process::exit(3);
        }
    }

    // ── Validate vault directory ──────────────────────────────────────────
    if !vault_dir.exists() {
        eprintln!("error: directory not found: {}", vault_dir.display());
        process::exit(2);
    }
    if !vault_dir.is_dir() {
        eprintln!("error: {} is not a directory", vault_dir.display());
        process::exit(3);
    }
    if vault_dir.join("index.lock").exists() {
        eprintln!(
            "error: a vault already exists at {} (index.lock found)",
            vault_dir.display()
        );
        process::exit(4);
    }

    // ── Read password (double-prompt) ─────────────────────────────────────
    let password = read_password_with_confirm();

    // ── Create vault ──────────────────────────────────────────────────────
    let blobs = cli.blobs_dir.as_deref();
    match create_vault(vault_dir, blobs, &password) {
        Ok(_) => {
            if let Some(name) = blobs {
                println!(
                    "Vault created at {} (blobs in {}/{})",
                    vault_dir.display(),
                    vault_dir.display(),
                    name
                );
            } else {
                println!("Vault created at {}", vault_dir.display());
            }
            process::exit(0);
        }
        Err(VaultError::InvalidFormat(msg)) => {
            eprintln!("error: {}", msg);
            process::exit(4);
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
    use crate::pages::vault::crypto::{create_vault, open_vault};
    use tempfile::TempDir;

    #[test]
    fn create_vault_no_blobs_dir() {
        let dir = TempDir::new().unwrap();
        let handle = create_vault(dir.path(), None, "password").unwrap();
        assert!(dir.path().join("index.lock").exists());
        assert_eq!(handle.index.blobs_dir, None);
        assert_eq!(handle.blobs_dir, dir.path());
    }

    #[test]
    fn create_vault_with_blobs_dir() {
        let dir = TempDir::new().unwrap();
        let handle = create_vault(dir.path(), Some("blobs"), "password").unwrap();
        assert!(dir.path().join("index.lock").exists());
        assert!(dir.path().join("blobs").is_dir());
        assert_eq!(handle.index.blobs_dir, Some("blobs".to_string()));
        assert_eq!(handle.blobs_dir, dir.path().join("blobs"));
    }

    #[test]
    fn create_vault_roundtrips_via_open() {
        let dir = TempDir::new().unwrap();
        create_vault(dir.path(), None, "pw").unwrap();
        let opened = open_vault(dir.path(), "pw").unwrap();
        assert_eq!(opened.index.version, 1);
        assert!(opened.index.entries.is_empty());
    }

    #[test]
    fn create_vault_wrong_password_on_open() {
        let dir = TempDir::new().unwrap();
        create_vault(dir.path(), None, "correct").unwrap();
        let result = open_vault(dir.path(), "wrong");
        assert!(matches!(result, Err(crate::pages::vault::types::VaultError::WrongPassword)));
    }

    #[test]
    fn create_vault_refuses_existing_index() {
        let dir = TempDir::new().unwrap();
        create_vault(dir.path(), None, "pw").unwrap();
        let result = create_vault(dir.path(), None, "pw");
        assert!(matches!(result, Err(crate::pages::vault::types::VaultError::InvalidFormat(_))));
    }
}
