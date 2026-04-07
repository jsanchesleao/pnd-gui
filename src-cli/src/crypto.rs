//! Encryption protocol — compatible with pnd-gui.
//!
//! Single-file format: fixed 64 MiB frames, each independently encrypted as a
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
use std::io::{self, Read, Write};

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_SIZE: usize = 16;
const IV_SIZE: usize = 12;
const FRAME_SIZE: usize = 1024 * 1024 * 64; // 64 MiB

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

/// Fill `buf` from `reader`, stopping at EOF. Returns the number of bytes read.
fn read_frame(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..])? {
            0 => break,
            n => total += n,
        }
    }
    Ok(total)
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Encrypt `input` with `password` using the pnd-gui single-file format.
///
/// Writes size-prefixed encrypted frames to `output` one at a time — no full
/// file is held in memory.  `on_progress` is called after each frame with the
/// number of **plaintext** bytes processed in that frame.
pub fn encrypt_file(
    input: &mut impl Read,
    output: &mut impl Write,
    password: &str,
    on_progress: &mut impl FnMut(usize),
) -> io::Result<()> {
    let mut buf = vec![0u8; FRAME_SIZE];
    let mut wrote_any = false;

    loop {
        let n = read_frame(input, &mut buf)?;
        if n == 0 {
            if !wrote_any {
                // Empty file → one zero-length frame (matches JS behaviour)
                let blob = encrypt_blob(&[], password);
                output.write_all(&(blob.len() as u32).to_be_bytes())?;
                output.write_all(&blob)?;
                on_progress(0);
            }
            break;
        }
        let blob = encrypt_blob(&buf[..n], password);
        output.write_all(&(blob.len() as u32).to_be_bytes())?;
        output.write_all(&blob)?;
        on_progress(n);
        wrote_any = true;
    }

    Ok(())
}

/// Decrypt a pnd-gui single-file `.lock` stream.
///
/// Returns `Ok(true)` on success, `Ok(false)` if the password is wrong or the
/// data is corrupted, and `Err` for I/O failures.  Frames are decrypted one at
/// a time — no full file is held in memory.  `on_progress` is called after
/// each frame with the number of **plaintext** bytes produced.
pub fn decrypt_file(
    input: &mut impl Read,
    output: &mut impl Write,
    password: &str,
    on_progress: &mut impl FnMut(usize),
) -> io::Result<bool> {
    let mut size_buf = [0u8; 4];

    loop {
        match input.read_exact(&mut size_buf) {
            Ok(()) => {}
            // Clean EOF at a frame boundary — all frames processed.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }

        let size = u32::from_be_bytes(size_buf) as usize;
        let mut blob = vec![0u8; size];
        input.read_exact(&mut blob)?;

        match decrypt_blob(&blob, password) {
            Some(plain) => {
                on_progress(plain.len());
                output.write_all(&plain)?;
            }
            None => return Ok(false),
        }
    }

    Ok(true)
}
