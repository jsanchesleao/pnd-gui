//! Vault cryptography — compatible with pnd-gui's vault format.
//!
//! Two distinct decrypt paths:
//!
//! 1. **index.lock** — password-derived key (PBKDF2-HMAC-SHA256, 100 k iters).
//!    Layout: `[salt 16 B][IV 12 B][AES-256-GCM ciphertext + 16 B tag]`
//!
//! 2. **Blob files** — raw AES-256 key stored (base64) in the index entry.
//!    Layout: `[salt 16 B (ignored)][IV 12 B][AES-256-GCM ciphertext + 16 B tag]`

use aes_gcm::{Aes256Gcm, aead::{Aead, KeyInit}};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use pbkdf2::pbkdf2_hmac;
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;
use std::{io, path::Path};

use super::types::{VaultError, VaultHandle, VaultIndex};

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const IV_LEN: usize = 12;
const MIN_BLOB_LEN: usize = SALT_LEN + IV_LEN + 16; // 16 = GCM auth tag

// ── Key derivation ────────────────────────────────────────────────────────

/// Derive a 32-byte AES-256 key from `password` and a 16-byte `salt` using
/// PBKDF2-HMAC-SHA256 with 100 000 iterations. Used only for `index.lock`.
fn pbkdf2_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

// ── Blob helpers (raw key) ────────────────────────────────────────────────

/// Decrypt a vault blob using a raw base64-encoded AES-256 key.
///
/// Layout: `[salt 16 B (skipped)][IV 12 B][ciphertext + tag]`
pub(crate) fn decrypt_blob_with_key(data: &[u8], key_base64: &str) -> Result<Vec<u8>, VaultError> {
    if data.len() < MIN_BLOB_LEN {
        return Err(VaultError::InvalidFormat("blob too short".into()));
    }

    let key_bytes = B64
        .decode(key_base64)
        .map_err(|e| VaultError::InvalidFormat(format!("bad key base64: {e}")))?;

    // First 16 bytes are salt — present for format uniformity, not used here.
    let iv = &data[SALT_LEN..SALT_LEN + IV_LEN];
    let ciphertext = &data[SALT_LEN + IV_LEN..];

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| VaultError::InvalidFormat(format!("bad key length: {e}")))?;
    let nonce = aes_gcm::Nonce::from_slice(iv);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| VaultError::WrongPassword)
}

/// Encrypt `plaintext` with a raw base64-encoded AES-256 key.
///
/// Writes `[salt 16 B (zeros)][IV 12 B (random)][ciphertext + tag]`.
/// The salt field is zeroed because the key is not password-derived for blobs.
pub(crate) fn encrypt_blob_with_key(plaintext: &[u8], key_base64: &str) -> Result<Vec<u8>, VaultError> {
    let key_bytes = B64
        .decode(key_base64)
        .map_err(|e| VaultError::InvalidFormat(format!("bad key base64: {e}")))?;

    let mut iv_bytes = [0u8; IV_LEN];
    OsRng.fill_bytes(&mut iv_bytes);

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| VaultError::InvalidFormat(format!("bad key length: {e}")))?;
    let nonce = aes_gcm::Nonce::from_slice(&iv_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| VaultError::InvalidFormat("encryption failed".into()))?;

    let mut out = Vec::with_capacity(SALT_LEN + IV_LEN + ciphertext.len());
    out.extend_from_slice(&[0u8; SALT_LEN]); // zeroed salt — key is already stored in index
    out.extend_from_slice(&iv_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Generate a fresh random base64-encoded AES-256 key (32 random bytes).
pub(crate) fn generate_key_base64() -> String {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    B64.encode(key)
}

/// Generate a random UUID v4 string (no external crate required).
pub(crate) fn generate_uuid() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    // Set version (4) and variant (RFC4122) bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

// ── index.lock helpers (password-derived key) ─────────────────────────────

/// Decrypt bytes that were encrypted with a PBKDF2-derived key.
///
/// Layout: `[salt 16 B][IV 12 B][ciphertext + tag]`
fn decrypt_with_password(data: &[u8], password: &str) -> Result<Vec<u8>, VaultError> {
    if data.len() < MIN_BLOB_LEN {
        return Err(VaultError::InvalidFormat("index.lock too short".into()));
    }

    let salt = &data[0..SALT_LEN];
    let iv   = &data[SALT_LEN..SALT_LEN + IV_LEN];
    let ciphertext = &data[SALT_LEN + IV_LEN..];

    let key_bytes = pbkdf2_key(password, salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32-byte key");
    let nonce  = aes_gcm::Nonce::from_slice(iv);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| VaultError::WrongPassword)
}

/// Encrypt `plaintext` with a PBKDF2-derived key (fresh random salt each call).
///
/// Writes `[salt 16 B][IV 12 B][ciphertext + tag]`.
fn encrypt_with_password(plaintext: &[u8], password: &str) -> Vec<u8> {
    let mut salt     = [0u8; SALT_LEN];
    let mut iv_bytes = [0u8; IV_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut iv_bytes);

    let key_bytes = pbkdf2_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32-byte key");
    let nonce  = aes_gcm::Nonce::from_slice(&iv_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption never fails for AES-GCM");

    let mut out = Vec::with_capacity(SALT_LEN + IV_LEN + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&iv_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

// ── Public vault operations ───────────────────────────────────────────────

/// Create a new empty vault at `root` and write the initial `index.lock`.
///
/// - `root` must be an existing directory.
/// - `root/index.lock` must not already exist.
/// - If `blobs_dir_name` is `Some("name")`, the subdirectory `root/name/` is created.
pub(crate) fn create_vault(
    root: &Path,
    blobs_dir_name: Option<&str>,
    password: &str,
) -> Result<VaultHandle, VaultError> {
    if !root.is_dir() {
        return Err(VaultError::InvalidFormat(
            "vault path must be an existing directory".into(),
        ));
    }
    let index_path = root.join("index.lock");
    if index_path.exists() {
        return Err(VaultError::InvalidFormat(
            "index.lock already exists — this directory is already a vault".into(),
        ));
    }
    if let Some(name) = blobs_dir_name {
        std::fs::create_dir_all(root.join(name))?;
    }
    let root_buf = root.to_path_buf();
    let index = VaultIndex {
        version: 1,
        blobs_dir: blobs_dir_name.map(str::to_string),
        entries: indexmap::IndexMap::new(),
    };
    let blobs_dir = VaultHandle::resolve_blobs_dir(&root_buf, &index);
    let handle = VaultHandle {
        root: root_buf,
        blobs_dir,
        password: password.to_string(),
        index,
    };
    save_vault(&handle)?;
    Ok(handle)
}

/// Read and decrypt `<root>/index.lock`, returning a populated [`VaultHandle`].
pub(crate) fn open_vault(root: &Path, password: &str) -> Result<VaultHandle, VaultError> {
    let index_path = root.join("index.lock");
    let encrypted = std::fs::read(&index_path)
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound =>
                VaultError::InvalidFormat("index.lock not found — not a vault directory".into()),
            _ => VaultError::Io(e),
        })?;

    let json_bytes = decrypt_with_password(&encrypted, password)?;

    let index: VaultIndex = serde_json::from_slice(&json_bytes)
        .map_err(|e| VaultError::InvalidFormat(format!("index JSON invalid: {e}")))?;

    if index.version != 1 {
        return Err(VaultError::InvalidFormat(
            format!("unsupported vault version {}", index.version),
        ));
    }

    let root_buf = root.to_path_buf();
    let blobs_dir = VaultHandle::resolve_blobs_dir(&root_buf, &index);

    Ok(VaultHandle { root: root_buf, blobs_dir, password: password.to_string(), index })
}

/// Serialize and re-encrypt the in-memory index, writing it to `index.lock`.
///
/// Uses an atomic write: encrypt → write to `index.lock.tmp` → rename over `index.lock`.
pub(crate) fn save_vault(handle: &VaultHandle) -> Result<(), VaultError> {
    let json = serde_json::to_vec(&handle.index)
        .map_err(|e| VaultError::InvalidFormat(format!("index serialization failed: {e}")))?;

    let encrypted = encrypt_with_password(&json, &handle.password);

    let tmp_path  = handle.root.join("index.lock.tmp");
    let lock_path = handle.root.join("index.lock");

    std::fs::write(&tmp_path, &encrypted)?;
    std::fs::rename(&tmp_path, &lock_path)?;

    Ok(())
}

/// Decrypt all parts of a vault entry and return the concatenated plaintext.
#[cfg(test)]
pub(crate) fn decrypt_entry(handle: &VaultHandle, uuid: &str) -> Result<Vec<u8>, VaultError> {
    let entry = handle
        .index
        .entries
        .get(uuid)
        .ok_or_else(|| VaultError::NotFound(uuid.to_string()))?;

    let mut out = Vec::with_capacity(entry.size as usize);

    for part in &entry.parts {
        let blob = std::fs::read(handle.blob_path(&part.uuid))?;
        let plain = decrypt_blob_with_key(&blob, &part.key_base64)?;
        out.extend_from_slice(&plain);
    }

    Ok(out)
}

/// Encrypt one file from disk and write its blob(s) to `blobs_dir`.
///
/// Large files (> 256 MiB) are split into multiple parts, each encrypted with
/// an independent random AES-256 key. Returns a `(file_uuid, VaultEntry)` pair
/// ready to be inserted into the vault index.
///
/// `virtual_path` is the vault folder path where the file will appear (use `""`
/// for the vault root).
pub(crate) fn encrypt_file_to_vault(
    file_path: &Path,
    blobs_dir: &Path,
    virtual_path: &str,
) -> Result<(String, super::types::VaultEntry), VaultError> {
    use super::types::{VaultEntry, VaultPart};

    const BLOCK_SIZE: usize = 256 * 1024 * 1024; // 256 MiB

    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());

    let data = std::fs::read(file_path)?;
    let size = data.len() as u64;

    let file_uuid = generate_uuid();
    let mut parts: Vec<VaultPart> = Vec::new();

    if data.is_empty() {
        // Empty file: one empty blob part.
        let key = generate_key_base64();
        let blob_uuid = generate_uuid();
        let blob = encrypt_blob_with_key(&[], &key)?;
        std::fs::write(blobs_dir.join(&blob_uuid), blob)?;
        parts.push(VaultPart { uuid: blob_uuid, key_base64: key });
    } else {
        for chunk in data.chunks(BLOCK_SIZE) {
            let key = generate_key_base64();
            let blob_uuid = generate_uuid();
            let blob = encrypt_blob_with_key(chunk, &key)?;
            std::fs::write(blobs_dir.join(&blob_uuid), blob)?;
            parts.push(VaultPart { uuid: blob_uuid, key_base64: key });
        }
    }

    let entry = VaultEntry {
        name,
        path: virtual_path.to_string(),
        size,
        parts,
        thumbnail_uuid: None,
        thumbnail_key_base64: None,
    };

    Ok((file_uuid, entry))
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use tempfile::TempDir;

    // ── Helpers ─────────────────────────────────────────────────────────────

    /// Build a minimal VaultHandle backed by a real temp directory.
    fn make_handle(dir: &TempDir, password: &str) -> VaultHandle {
        let root = dir.path().to_path_buf();
        let index = VaultIndex { version: 1, blobs_dir: None, entries: IndexMap::new() };
        VaultHandle {
            blobs_dir: root.clone(),
            root,
            password: password.to_string(),
            index,
        }
    }

    // ── generate_key_base64 ──────────────────────────────────────────────────

    #[test]
    fn key_base64_has_correct_length() {
        // 32 raw bytes → 44 base64 characters (STANDARD encoding, no line breaks)
        let k = generate_key_base64();
        assert_eq!(k.len(), 44, "expected 44-char base64 for 32 bytes, got: {k}");
    }

    #[test]
    fn key_base64_is_unique_each_call() {
        let k1 = generate_key_base64();
        let k2 = generate_key_base64();
        assert_ne!(k1, k2, "two generated keys should differ");
    }

    #[test]
    fn key_base64_is_valid_base64() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let k = generate_key_base64();
        let decoded = STANDARD.decode(&k).expect("should decode as valid base64");
        assert_eq!(decoded.len(), 32);
    }

    // ── encrypt_blob_with_key / decrypt_blob_with_key ────────────────────────

    #[test]
    fn blob_key_roundtrip() {
        let key = generate_key_base64();
        let plain = b"hello from vault blob";
        let enc = encrypt_blob_with_key(plain, &key).unwrap();
        let dec = decrypt_blob_with_key(&enc, &key).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn blob_key_roundtrip_empty_plaintext() {
        let key = generate_key_base64();
        let enc = encrypt_blob_with_key(&[], &key).unwrap();
        let dec = decrypt_blob_with_key(&enc, &key).unwrap();
        assert!(dec.is_empty());
    }

    #[test]
    fn blob_key_wrong_key_returns_error() {
        let key1 = generate_key_base64();
        let key2 = generate_key_base64();
        let enc = encrypt_blob_with_key(b"secret", &key1).unwrap();
        assert!(matches!(decrypt_blob_with_key(&enc, &key2), Err(VaultError::WrongPassword)));
    }

    #[test]
    fn blob_key_too_short_returns_error() {
        let key = generate_key_base64();
        let short = vec![0u8; 10];
        let result = decrypt_blob_with_key(&short, &key);
        assert!(matches!(result, Err(VaultError::InvalidFormat(_))));
    }

    #[test]
    fn blob_key_bad_base64_returns_error() {
        let data = vec![0u8; 50];
        let result = decrypt_blob_with_key(&data, "not-valid-base64!!!");
        assert!(matches!(result, Err(VaultError::InvalidFormat(_))));
    }

    #[test]
    fn blob_salt_field_is_zeroed() {
        let key = generate_key_base64();
        let enc = encrypt_blob_with_key(b"data", &key).unwrap();
        assert_eq!(&enc[..16], &[0u8; 16], "salt bytes should be zeroed for blob files");
    }

    #[test]
    fn two_encryptions_produce_distinct_ciphertext() {
        let key = generate_key_base64();
        let e1 = encrypt_blob_with_key(b"same", &key).unwrap();
        let e2 = encrypt_blob_with_key(b"same", &key).unwrap();
        assert_ne!(e1, e2, "random IV must produce unique ciphertext each call");
    }

    // ── open_vault / save_vault ──────────────────────────────────────────────

    #[test]
    fn save_then_open_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut handle = make_handle(&dir, "correct-password");
        handle.index.entries.insert(
            "uuid-1".to_string(),
            super::super::types::VaultEntry {
                name: "file.txt".to_string(),
                path: "docs".to_string(),
                size: 42,
                parts: vec![],
                thumbnail_uuid: None,
                thumbnail_key_base64: None,
            },
        );

        save_vault(&handle).unwrap();
        let reopened = open_vault(dir.path(), "correct-password").unwrap();

        assert_eq!(reopened.index.version, 1);
        assert!(reopened.index.entries.contains_key("uuid-1"));
        let entry = &reopened.index.entries["uuid-1"];
        assert_eq!(entry.name, "file.txt");
        assert_eq!(entry.path, "docs");
        assert_eq!(entry.size, 42);
    }

    #[test]
    fn open_vault_wrong_password_returns_error() {
        let dir = TempDir::new().unwrap();
        let handle = make_handle(&dir, "correct");
        save_vault(&handle).unwrap();
        assert!(matches!(open_vault(dir.path(), "wrong"), Err(VaultError::WrongPassword)));
    }

    #[test]
    fn open_vault_missing_index_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = open_vault(dir.path(), "pw");
        assert!(matches!(result, Err(VaultError::InvalidFormat(_))));
    }

    #[test]
    fn save_vault_no_tmp_leftover() {
        let dir = TempDir::new().unwrap();
        let handle = make_handle(&dir, "pw");
        save_vault(&handle).unwrap();
        assert!(!dir.path().join("index.lock.tmp").exists());
        assert!(dir.path().join("index.lock").exists());
    }

    // ── decrypt_entry ────────────────────────────────────────────────────────

    #[test]
    fn decrypt_single_part_entry() {
        let dir = TempDir::new().unwrap();
        let plaintext = b"vault file contents";
        let key = generate_key_base64();
        let blob = encrypt_blob_with_key(plaintext, &key).unwrap();
        let blob_uuid = "test-blob-uuid";
        std::fs::write(dir.path().join(blob_uuid), &blob).unwrap();

        let mut handle = make_handle(&dir, "pw");
        handle.index.entries.insert(
            "file-uuid".to_string(),
            super::super::types::VaultEntry {
                name: "file.txt".to_string(),
                path: "".to_string(),
                size: plaintext.len() as u64,
                parts: vec![super::super::types::VaultPart {
                    uuid: blob_uuid.to_string(),
                    key_base64: key,
                }],
                thumbnail_uuid: None,
                thumbnail_key_base64: None,
            },
        );

        let recovered = decrypt_entry(&handle, "file-uuid").unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn decrypt_entry_not_found() {
        let dir = TempDir::new().unwrap();
        let handle = make_handle(&dir, "pw");
        assert!(matches!(
            decrypt_entry(&handle, "nonexistent"),
            Err(VaultError::NotFound(_))
        ));
    }

    #[test]
    fn decrypt_multi_part_entry() {
        let dir = TempDir::new().unwrap();
        let part1 = b"first chunk";
        let part2 = b"second chunk";
        let key1 = generate_key_base64();
        let key2 = generate_key_base64();

        std::fs::write(dir.path().join("blob-1"), encrypt_blob_with_key(part1, &key1).unwrap()).unwrap();
        std::fs::write(dir.path().join("blob-2"), encrypt_blob_with_key(part2, &key2).unwrap()).unwrap();

        let mut handle = make_handle(&dir, "pw");
        handle.index.entries.insert(
            "file-uuid".to_string(),
            super::super::types::VaultEntry {
                name: "big.bin".to_string(),
                path: "".to_string(),
                size: (part1.len() + part2.len()) as u64,
                parts: vec![
                    super::super::types::VaultPart { uuid: "blob-1".to_string(), key_base64: key1 },
                    super::super::types::VaultPart { uuid: "blob-2".to_string(), key_base64: key2 },
                ],
                thumbnail_uuid: None,
                thumbnail_key_base64: None,
            },
        );

        let recovered = decrypt_entry(&handle, "file-uuid").unwrap();
        let mut expected = part1.to_vec();
        expected.extend_from_slice(part2);
        assert_eq!(recovered, expected);
    }
}
