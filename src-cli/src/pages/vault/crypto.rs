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
