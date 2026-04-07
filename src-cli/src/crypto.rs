//! Encryption protocol — compatible with pnd-gui.
//!
//! Single-file format: fixed 1 MiB frames, each independently encrypted as a
//! blob and prefixed with a 4-byte big-endian frame size.
//!
//! Blob layout: [0–15] salt | [16–27] IV | [28+] AES-256-GCM ciphertext + tag

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit},
};
use pbkdf2::pbkdf2_hmac;
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_SIZE: usize = 16;
const IV_SIZE: usize = 12;
const FRAME_SIZE: usize = 1024 * 1024; // 1 MiB

// ── Primitives ─────────────────────────────────────────────────────────────

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

fn encrypt_blob(plaintext: &[u8], password: &str) -> Vec<u8> {
    let mut salt = [0u8; SALT_SIZE];
    let mut iv_bytes = [0u8; IV_SIZE];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut iv_bytes);

    let key_bytes = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32-byte key");
    let nonce = aes_gcm::Nonce::from_slice(&iv_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption");

    let mut out = Vec::with_capacity(SALT_SIZE + IV_SIZE + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&iv_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

fn decrypt_blob(data: &[u8], password: &str) -> Option<Vec<u8>> {
    if data.len() < SALT_SIZE + IV_SIZE + 16 {
        return None; // too short to hold even an empty GCM ciphertext + tag
    }
    let salt = &data[0..SALT_SIZE];
    let iv = &data[SALT_SIZE..SALT_SIZE + IV_SIZE];
    let ciphertext = &data[SALT_SIZE + IV_SIZE..];

    let key_bytes = derive_key(password, salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32-byte key");
    let nonce = aes_gcm::Nonce::from_slice(iv);
    cipher.decrypt(nonce, ciphertext).ok()
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Encrypt `data` with `password` using the pnd-gui single-file format.
///
/// Output: concatenation of size-prefixed encrypted frames.
pub fn encrypt_file(data: &[u8], password: &str) -> Vec<u8> {
    let mut out = Vec::new();
    // Empty file → one zero-length frame (matches JS behaviour)
    let frames: Box<dyn Iterator<Item = &[u8]>> = if data.is_empty() {
        Box::new(std::iter::once(data))
    } else {
        Box::new(data.chunks(FRAME_SIZE))
    };

    for frame in frames {
        let blob = encrypt_blob(frame, password);
        let size = (blob.len() as u32).to_be_bytes();
        out.extend_from_slice(&size);
        out.extend_from_slice(&blob);
    }
    out
}

/// Decrypt a pnd-gui single-file `.lock` blob.
///
/// Returns `None` if the password is wrong or the data is corrupted.
pub fn decrypt_file(data: &[u8], password: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if pos + 4 > data.len() {
            return None; // truncated size prefix
        }
        let size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if pos + size > data.len() {
            return None; // truncated frame body
        }
        let blob = &data[pos..pos + size];
        pos += size;

        let plain = decrypt_blob(blob, password)?;
        out.extend_from_slice(&plain);
    }

    Some(out)
}
