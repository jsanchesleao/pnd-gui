use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum OperationMode {
    #[value(name = "encrypt", alias = "enc", alias = "e")]
    Encrypt,
    #[value(name = "decrypt", alias = "dec", alias = "d")]
    Decrypt,
}

/// Password and note depot — encrypt, decrypt, preview, and manage encrypted vaults.
#[derive(Parser, Debug)]
#[command(name = "pnd-cli", version, about, long_about = None)]
pub struct Cli {
    // ── Global options ─────────────────────────────────────────────────────

    /// Vault directory (alternative to positional <vault-dir> for all vault commands)
    #[arg(long, value_name = "DIR", global = true)]
    pub vault_dir: Option<PathBuf>,

    // ── Mode flags ─────────────────────────────────────────────────────────

    /// Open the TUI with <file> pre-loaded instead of running non-interactively
    #[arg(short = 't', long)]
    pub tui: bool,

    /// Decrypt <file> into memory and open a preview (non-interactive)
    #[arg(short = 'p', long = "preview")]
    pub preview_mode: bool,

    /// Open the vault at [VAULT_DIR] in the TUI vault browser (default: current directory)
    #[arg(long, value_name = "VAULT_DIR", num_args = 0..=1, default_missing_value = ".")]
    pub vault: Option<PathBuf>,

    /// List vault contents (non-interactive); optional [VAULT_DIR] defaults to current directory
    #[arg(long, value_name = "VAULT_DIR", num_args = 0..=1, default_missing_value = ".")]
    pub vault_list: Option<PathBuf>,

    /// Preview a vault entry at VAULT_PATH (non-interactive)
    #[arg(long, value_name = "VAULT_PATH")]
    pub vault_preview: Option<String>,

    /// Add one or more files to the vault
    #[arg(long, value_name = "FILE", num_args = 1..)]
    pub vault_add: Vec<PathBuf>,

    /// Export a vault entry at VAULT_PATH to disk
    #[arg(long, value_name = "VAULT_PATH")]
    pub vault_export: Option<String>,

    /// Rename a vault entry: --vault-rename <VAULT_PATH> <NEW_NAME>
    #[arg(long, value_name = "VAULT_PATH NEW_NAME", num_args = 2)]
    pub vault_rename: Vec<String>,

    /// Move a vault entry to a different virtual folder: --vault-move <VAULT_PATH> <DEST_FOLDER>
    #[arg(long, value_name = "VAULT_PATH DEST_FOLDER", num_args = 2)]
    pub vault_move: Vec<String>,

    /// Delete one or more vault entries: --vault-delete <VAULT_PATH>...
    #[arg(long, value_name = "VAULT_PATH", num_args = 1..)]
    pub vault_delete: Vec<String>,

    /// Initialise a new empty vault at [VAULT_DIR] (default: current directory)
    #[arg(long, value_name = "VAULT_DIR", num_args = 0..=1, default_missing_value = ".")]
    pub vault_init: Option<PathBuf>,

    // ── Encrypt/decrypt options ────────────────────────────────────────────

    /// Explicit operation mode — required when reading from stdin (encrypt/decrypt only)
    #[arg(short = 'm', long, value_enum)]
    pub mode: Option<OperationMode>,

    /// Write output to PATH instead of the default location
    #[arg(short = 'o', value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Overwrite the output file if it already exists
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Write output to stdout instead of a file
    #[arg(short = 'c', long)]
    pub stdout: bool,

    // ── Vault-list options ─────────────────────────────────────────────────

    /// Output vault list as a JSON array
    #[arg(long)]
    pub json: bool,

    /// Filter vault list to this virtual subfolder
    #[arg(long, value_name = "VAULT_PATH")]
    pub path: Option<String>,

    // ── Vault-init options ─────────────────────────────────────────────────

    /// Blobs subdirectory name inside the vault (used with --vault-init).
    /// Leave unset to store blobs alongside index.lock.
    #[arg(long, value_name = "NAME")]
    pub blobs_dir: Option<String>,

    // ── Vault-add options ──────────────────────────────────────────────────

    /// Virtual folder inside the vault where added files are placed (default: root)
    #[arg(long, value_name = "VAULT_PATH")]
    pub vault_path: Option<String>,

    // ── Vault-export options ───────────────────────────────────────────────

    /// Destination directory for --vault-export (default: current directory)
    #[arg(long, value_name = "DIR")]
    pub dest: Option<PathBuf>,

    /// Include files in subfolders when exporting a vault directory
    #[arg(short = 'r', long)]
    pub recursive: bool,

    /// Skip the confirmation prompt (for --vault-export dir and --vault-delete)
    #[arg(short = 'y', long)]
    pub yes: bool,

    // ── Vault-move options ─────────────────────────────────────────────────

    /// Rename the entry while moving it (used with --vault-move)
    #[arg(long, value_name = "NEW_NAME")]
    pub name: Option<String>,

    // ── Positional ─────────────────────────────────────────────────────────

    /// File(s) to encrypt, decrypt, or preview
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,
}

impl Cli {
    /// Returns true when no action is requested — the TUI should be launched.
    pub fn is_tui_mode(&self) -> bool {
        !self.stdout
            && self.mode.is_none()
            && self.files.is_empty()
            && !self.preview_mode
            && self.vault.is_none()
            && self.vault_list.is_none()
            && self.vault_preview.is_none()
            && self.vault_export.is_none()
            && self.vault_add.is_empty()
            && self.vault_rename.is_empty()
            && self.vault_move.is_empty()
            && self.vault_delete.is_empty()
            && self.vault_init.is_none()
    }
}
