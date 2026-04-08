//! Vault data structures — compatible with the pnd-gui TypeScript types.

use std::{collections::HashMap, io, path::PathBuf};
use serde::{Deserialize, Serialize};

// ── Index ─────────────────────────────────────────────────────────────────

/// Top-level vault index. Encrypted and stored as `index.lock`.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct VaultIndex {
    pub(crate) version: u32,
    /// Optional subdirectory (relative to vault root) where blobs are stored.
    /// Absent means blobs live directly in the root.
    #[serde(rename = "blobsDir", skip_serializing_if = "Option::is_none")]
    pub(crate) blobs_dir: Option<String>,
    /// All file entries, keyed by their UUID string.
    pub(crate) entries: HashMap<String, VaultEntry>,
}

/// Metadata for one logical file stored in the vault.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct VaultEntry {
    /// Filename only (no path component), e.g. `"photo.jpg"`.
    pub(crate) name: String,
    /// Virtual folder path with forward-slash separators and no leading/trailing
    /// slash. The empty string means the vault root.
    /// Examples: `""`, `"photos"`, `"photos/summer"`.
    pub(crate) path: String,
    /// Original plaintext byte count.
    pub(crate) size: u64,
    /// Encrypted chunks in order. Most files have exactly one part; files
    /// larger than 256 MiB are split across multiple parts.
    pub(crate) parts: Vec<VaultPart>,
    #[serde(rename = "thumbnailUuid", skip_serializing_if = "Option::is_none")]
    pub(crate) thumbnail_uuid: Option<String>,
    #[serde(rename = "thumbnailKeyBase64", skip_serializing_if = "Option::is_none")]
    pub(crate) thumbnail_key_base64: Option<String>,
}

/// One encrypted blob that forms part of a file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct VaultPart {
    /// UUID string — also the filename of the blob on disk.
    pub(crate) uuid: String,
    /// Base64-encoded raw AES-256 key used to decrypt this blob.
    #[serde(rename = "keyBase64")]
    pub(crate) key_base64: String,
}

// ── In-memory handle ──────────────────────────────────────────────────────

/// An open, unlocked vault held in memory.
pub(crate) struct VaultHandle {
    /// Path to the vault root directory on disk.
    pub(crate) root: PathBuf,
    /// Path to the blobs directory (equals `root` when `blobsDir` is absent).
    pub(crate) blobs_dir: PathBuf,
    /// Master password kept in memory for re-saving the index.
    pub(crate) password: String,
    /// Decoded, decrypted index.
    pub(crate) index: VaultIndex,
}

impl VaultHandle {
    /// Derive the blobs directory path from the vault root and index.
    pub(crate) fn resolve_blobs_dir(root: &PathBuf, index: &VaultIndex) -> PathBuf {
        match &index.blobs_dir {
            Some(sub) => root.join(sub),
            None => root.clone(),
        }
    }

    /// Return the on-disk path for a blob UUID.
    pub(crate) fn blob_path(&self, uuid: &str) -> PathBuf {
        self.blobs_dir.join(uuid)
    }

    /// Immediate child folder names under `path` (not recursive).
    /// Returns bare names (last segment only), sorted.
    pub(crate) fn subfolders(&self, path: &str) -> Vec<String> {
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for entry in self.index.entries.values() {
            let ep = &entry.path;
            if ep.is_empty() { continue; }
            if !prefix.is_empty() && !ep.starts_with(&prefix) { continue; }

            // The segment immediately after the prefix.
            let remainder = if prefix.is_empty() { ep.as_str() } else { &ep[prefix.len()..] };
            let segment = remainder.split('/').next().unwrap_or("");
            if !segment.is_empty() {
                seen.insert(segment.to_string());
            }
        }

        let mut result: Vec<String> = seen.into_iter().collect();
        result.sort();
        result
    }

    /// Files whose `path` field exactly matches `path`, sorted by name.
    pub(crate) fn entries_in_path(&self, path: &str) -> Vec<(&str, &VaultEntry)> {
        let mut out: Vec<(&str, &VaultEntry)> = self
            .index
            .entries
            .iter()
            .filter(|(_, e)| e.path == path)
            .map(|(uuid, e)| (uuid.as_str(), e))
            .collect();
        out.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        out
    }
}

// ── Errors ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) enum VaultError {
    /// Master password is incorrect or `index.lock` is corrupted.
    WrongPassword,
    /// File is present but its content cannot be interpreted as a valid vault.
    InvalidFormat(String),
    /// No index entry with the given UUID or name was found.
    NotFound(String),
    /// A file with the same name already exists in the destination path.
    DuplicateName,
    /// Underlying I/O failure.
    Io(io::Error),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::WrongPassword     => write!(f, "wrong password or corrupted index"),
            VaultError::InvalidFormat(m)  => write!(f, "invalid vault format: {m}"),
            VaultError::NotFound(n)       => write!(f, "entry not found: {n}"),
            VaultError::DuplicateName     => write!(f, "a file with that name already exists"),
            VaultError::Io(e)             => write!(f, "I/O error: {e}"),
        }
    }
}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self { VaultError::Io(e) }
}
