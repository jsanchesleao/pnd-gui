use clap::Parser;
use std::path::PathBuf;

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

    // ── Encrypt/decrypt options ────────────────────────────────────────────

    /// Write output to PATH instead of the default location
    #[arg(short = 'o', value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Overwrite the output file if it already exists
    #[arg(short = 'f', long)]
    pub force: bool,

    // ── Vault-list options ─────────────────────────────────────────────────

    /// Output vault list as a JSON array
    #[arg(long)]
    pub json: bool,

    /// Filter vault list to this virtual subfolder
    #[arg(long, value_name = "VAULT_PATH")]
    pub path: Option<String>,

    // ── Vault-add options ──────────────────────────────────────────────────

    /// Virtual folder inside the vault where added files are placed (default: root)
    #[arg(long, value_name = "VAULT_PATH")]
    pub vault_path: Option<String>,

    // ── Vault-export options ───────────────────────────────────────────────

    /// Destination directory for --vault-export (default: current directory)
    #[arg(long, value_name = "DIR")]
    pub dest: Option<PathBuf>,

    // ── Positional ─────────────────────────────────────────────────────────

    /// File(s) to encrypt, decrypt, or preview
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,
}

impl Cli {
    /// Returns true when no action is requested — the TUI should be launched.
    pub fn is_tui_mode(&self) -> bool {
        self.files.is_empty()
            && !self.preview_mode
            && self.vault.is_none()
            && self.vault_list.is_none()
            && self.vault_preview.is_none()
            && self.vault_export.is_none()
            && self.vault_add.is_empty()
    }
}
