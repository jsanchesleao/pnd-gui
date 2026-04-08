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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── Helpers ─────────────────────────────────────────────────────────────

    fn make_entry(name: &str, path: &str) -> VaultEntry {
        VaultEntry {
            name: name.to_string(),
            path: path.to_string(),
            size: 0,
            parts: vec![],
            thumbnail_uuid: None,
            thumbnail_key_base64: None,
        }
    }

    fn make_handle_with_entries(entries: &[(&str, &str, &str)]) -> VaultHandle {
        // entries: (uuid, name, path)
        let mut map = HashMap::new();
        for (uuid, name, path) in entries {
            map.insert(uuid.to_string(), make_entry(name, path));
        }
        let index = VaultIndex { version: 1, blobs_dir: None, entries: map };
        let root = PathBuf::from("/tmp/vault");
        let blobs_dir = root.clone();
        VaultHandle { root, blobs_dir, password: "pw".to_string(), index }
    }

    // ── resolve_blobs_dir ────────────────────────────────────────────────────

    #[test]
    fn resolve_blobs_dir_no_sub() {
        let root = PathBuf::from("/my/vault");
        let index = VaultIndex { version: 1, blobs_dir: None, entries: HashMap::new() };
        let result = VaultHandle::resolve_blobs_dir(&root, &index);
        assert_eq!(result, root);
    }

    #[test]
    fn resolve_blobs_dir_with_sub() {
        let root = PathBuf::from("/my/vault");
        let index = VaultIndex {
            version: 1,
            blobs_dir: Some("blobs".to_string()),
            entries: HashMap::new(),
        };
        let result = VaultHandle::resolve_blobs_dir(&root, &index);
        assert_eq!(result, PathBuf::from("/my/vault/blobs"));
    }

    // ── blob_path ────────────────────────────────────────────────────────────

    #[test]
    fn blob_path_joins_correctly() {
        let handle = make_handle_with_entries(&[]);
        let p = handle.blob_path("abc-123");
        assert_eq!(p, PathBuf::from("/tmp/vault/abc-123"));
    }

    // ── subfolders ───────────────────────────────────────────────────────────

    #[test]
    fn subfolders_at_root() {
        let handle = make_handle_with_entries(&[
            ("u1", "a.txt",   ""),
            ("u2", "b.jpg",   "photos"),
            ("u3", "c.jpg",   "photos/summer"),
            ("u4", "d.pdf",   "documents"),
        ]);
        let mut subs = handle.subfolders("");
        subs.sort();
        assert_eq!(subs, vec!["documents", "photos"]);
    }

    #[test]
    fn subfolders_nested() {
        let handle = make_handle_with_entries(&[
            ("u1", "a.jpg", "photos/summer"),
            ("u2", "b.jpg", "photos/winter"),
            ("u3", "c.jpg", "photos/winter/snow"),
        ]);
        let mut subs = handle.subfolders("photos");
        subs.sort();
        assert_eq!(subs, vec!["summer", "winter"]);
    }

    #[test]
    fn subfolders_leaf_is_empty() {
        let handle = make_handle_with_entries(&[
            ("u1", "a.jpg", "photos/summer"),
        ]);
        assert!(handle.subfolders("photos/summer").is_empty());
    }

    #[test]
    fn subfolders_no_false_prefix_match() {
        // "photosother" must NOT appear as a subfolder of "photos"
        let handle = make_handle_with_entries(&[
            ("u1", "a.jpg", "photosother"),
        ]);
        assert!(handle.subfolders("photos").is_empty());
    }

    #[test]
    fn subfolders_empty_vault() {
        let handle = make_handle_with_entries(&[]);
        assert!(handle.subfolders("").is_empty());
    }

    // ── entries_in_path ──────────────────────────────────────────────────────

    #[test]
    fn entries_in_path_root_only() {
        let handle = make_handle_with_entries(&[
            ("u1", "b.txt", ""),
            ("u2", "a.txt", ""),
            ("u3", "c.jpg", "photos"),
        ]);
        let entries = handle.entries_in_path("");
        assert_eq!(entries.len(), 2);
        // Sorted alphabetically by name
        assert_eq!(entries[0].1.name, "a.txt");
        assert_eq!(entries[1].1.name, "b.txt");
    }

    #[test]
    fn entries_in_path_nested() {
        let handle = make_handle_with_entries(&[
            ("u1", "img.jpg", "photos/summer"),
            ("u2", "doc.pdf", ""),
        ]);
        let entries = handle.entries_in_path("photos/summer");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1.name, "img.jpg");
    }

    #[test]
    fn entries_in_path_no_partial_match() {
        // "photos/summer" should not appear under "photos"
        let handle = make_handle_with_entries(&[
            ("u1", "img.jpg", "photos/summer"),
        ]);
        assert!(handle.entries_in_path("photos").is_empty());
    }

    #[test]
    fn entries_in_path_empty() {
        let handle = make_handle_with_entries(&[]);
        assert!(handle.entries_in_path("").is_empty());
    }
}
