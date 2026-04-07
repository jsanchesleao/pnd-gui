# pnd-gui Encryption Protocol Specification

This document fully specifies the encryption formats used by pnd-gui. A developer in any language who follows this spec can produce and read files that are byte-for-byte compatible with this application.

There are two independent protocols:

- **Single-file encryption** — a streaming, frame-based format for `.lock` files produced by the Encrypt/Decrypt tab.
- **Vault storage** — a directory-based encrypted container holding many files with a shared password-protected index.

---

## 1. Constants

| Name | Value | Notes |
|---|---|---|
| `SALT_SIZE` | 16 bytes | Per encrypted blob |
| `IV_SIZE` | 12 bytes | Per encrypted blob |
| `KEY_SIZE` | 256 bits (32 bytes) | AES-GCM key length |
| `PBKDF2_ITERATIONS` | 100 000 | SHA-256 |
| `GCM_TAG_SIZE` | 16 bytes | Implicit, appended by AES-GCM |
| `FRAME_SIZE` | 1 048 576 bytes (1 MiB) | Default chunk size for single-file streaming |
| `PART_SIZE` | 268 435 456 bytes (256 MiB) | Max block size for vault file parts |
| `SIZE_PREFIX_BYTES` | 4 bytes | Frame size header, big-endian uint32 |

---

## 2. Cryptographic Primitives

### 2.1 Key Derivation (password → key)

Used when encrypting or decrypting with a user-supplied password.

- **Algorithm**: PBKDF2
- **PRF**: HMAC-SHA-256
- **Iterations**: 100 000
- **Salt**: 16 random bytes, generated fresh for each encryption operation and stored in the output
- **Output**: 32-byte AES-256-GCM key
- **Password encoding**: UTF-8

```
key = PBKDF2_HMAC_SHA256(password_utf8, salt, iterations=100000, dklen=32)
```

### 2.2 Encryption

- **Algorithm**: AES-256-GCM
- **Key size**: 256 bits
- **IV size**: 12 bytes, generated fresh per encryption call via a CSPRNG
- **Tag size**: 128 bits (16 bytes), appended to the ciphertext by the GCM mode automatically
- **Additional authenticated data (AAD)**: none

```
ciphertext_and_tag = AES_256_GCM_Encrypt(key, iv, plaintext)
# ciphertext_and_tag length = len(plaintext) + 16
```

### 2.3 Random Generation

All salts, IVs, and file keys must be generated with a cryptographically secure random number generator (CSPRNG). UUIDs used as filenames are RFC 4122 version 4 random UUIDs.

---

## 3. Encrypted Blob Format

This is the lowest-level storage unit. It is used both for password-derived encryption (single-file and vault index) and for key-derived encryption (vault file parts and thumbnails).

### Binary layout

```
Offset   Size      Field
──────────────────────────────────────────────────
0        16        Salt (random, used for PBKDF2 when password-based;
                         present but ignored when a raw key is used)
16       12        IV (random, used for AES-GCM)
28       N+16      AES-GCM ciphertext + 16-byte authentication tag
                   (N = length of plaintext)
```

**Total size**: `28 + len(plaintext) + 16` bytes

### Notes

- Every encryption call generates a fresh salt and IV. Never reuse an IV with the same key.
- For key-based blobs (vault parts and thumbnails), bytes 0–15 (the salt field) are still written as 16 random bytes but are not used during decryption — the key is stored externally in the vault index. The reader must still skip these 16 bytes to locate the IV at offset 16.
- Authentication tag verification is mandatory. A mismatch means the wrong key was used or the data is corrupted; the operation must fail.

### Password-based blob: encrypt

```
salt    = random_bytes(16)
iv      = random_bytes(12)
key     = PBKDF2_HMAC_SHA256(password_utf8, salt, 100000, 32)
ct+tag  = AES_256_GCM_Encrypt(key, iv, plaintext)
output  = salt || iv || ct+tag
```

### Password-based blob: decrypt

```
salt    = input[0:16]
iv      = input[16:28]
ct+tag  = input[28:]
key     = PBKDF2_HMAC_SHA256(password_utf8, salt, 100000, 32)
plaintext = AES_256_GCM_Decrypt(key, iv, ct+tag)
# returns error / null on authentication failure
```

### Key-based blob: encrypt

```
salt    = random_bytes(16)   # written but not used for decryption
iv      = random_bytes(12)
ct+tag  = AES_256_GCM_Encrypt(key, iv, plaintext)
output  = salt || iv || ct+tag
```

### Key-based blob: decrypt

```
# key is provided externally (decoded from base64 stored in vault index)
iv      = input[16:28]
ct+tag  = input[28:]
plaintext = AES_256_GCM_Decrypt(key, iv, ct+tag)
# returns error / null on authentication failure
```

---

## 4. Single-File Encryption Protocol

Single encrypted files use a **streaming frame protocol**. The plaintext is split into fixed-size frames, each encrypted independently as a blob, and then written sequentially with a size prefix. This allows streaming decryption of arbitrarily large files.

The output file conventionally carries a `.lock` suffix (e.g. `photo.jpg.lock`).

### 4.1 Encryption

```
Input:  plaintext byte stream, password string

1. Split plaintext into consecutive frames of exactly FRAME_SIZE bytes.
   The final frame may be shorter. An empty file produces a single frame
   of 0 bytes.

2. For each frame (in order, 0-based):
   a. Encrypt the frame data as a password-based blob:
      blob = salt(16) || iv(12) || AES_GCM_Encrypt(key, iv, frame_data)
   b. Write the blob to the output stream prefixed by its length:
      output += uint32_big_endian(len(blob)) || blob

Output: concatenation of all prefixed blobs
```

#### Frame size prefix encoding

The 4-byte prefix is a **big-endian unsigned 32-bit integer** representing the byte length of the following blob (not the plaintext frame).

```
# Example: blob length 1048620
bytes = [0x00, 0x10, 0x00, 0x2C]   # 0x0010002C = 1048620
```

### 4.2 Decryption

```
Input: encrypted byte stream, password string

1. Read the stream sequentially:
   a. Read 4 bytes → frame_size = uint32_big_endian(bytes)
   b. Read exactly frame_size bytes → blob
   c. Decrypt blob as a password-based blob → frame_plaintext
      (fail immediately if authentication fails)
   d. Repeat until end of stream

2. Concatenate all frame_plaintext values in order.

Output: original plaintext
```

### 4.3 Visual diagram

```
Encrypted file on disk:
┌────────────┬───────────────────────────────────┬────────────┬─────────────────────┬──────┐
│ 4B size    │ blob (salt+iv+ct) for frame 0     │ 4B size    │ blob for frame 1    │ ...  │
└────────────┴───────────────────────────────────┴────────────┴─────────────────────┴──────┘

Each blob:
┌──────────┬──────────┬─────────────────────────────────────────┐
│ 16B salt │ 12B IV   │ AES-GCM ciphertext + 16B tag            │
└──────────┴──────────┴─────────────────────────────────────────┘
```

---

## 5. Vault Storage Protocol

A vault is a **directory** (folder) on disk. It contains:

- `index.lock` — the encrypted vault index (one password-based blob)
- Blob files named by UUID — the encrypted file content parts and thumbnails

Blob files may optionally be stored in a subdirectory named by the `blobsDir` field in the index. If `blobsDir` is absent, blobs live in the same directory as `index.lock`.

### 5.1 Vault directory layout

```
vault/
├── index.lock            ← encrypted vault index
└── <blobsDir>/           ← optional subfolder (name stored in index)
    ├── 550e8400-...      ← encrypted file part (UUID filename)
    ├── f47ac10b-...      ← another part or thumbnail
    └── ...
```

If `blobsDir` is absent the UUID blob files sit directly inside `vault/`.

### 5.2 Vault Index Format

`index.lock` is a single **password-based blob** whose plaintext is a UTF-8 encoded JSON document.

#### Open vault

```
1. Read index.lock → bytes
2. Decrypt as password-based blob using the master password → utf8_json
3. JSON.parse(utf8_json) → VaultIndex object
```

#### Save vault

```
1. JSON.stringify(vault_index) → utf8_json (UTF-8)
2. Encrypt as password-based blob using the master password → bytes
3. Write bytes to index.lock (overwrite)
```

### 5.3 VaultIndex JSON Schema

```typescript
interface VaultIndex {
  version: 1;                                   // always 1
  blobsDir?: string;                            // subfolder name for blobs; absent = vault root
  entries: Record<string, VaultIndexEntry>;     // keys are RFC 4122 v4 UUIDs (entry identifiers)
}

interface VaultIndexEntry {
  name: string;            // original filename, e.g. "photo.jpg"
  path: string;            // virtual folder path — no leading or trailing slash
                           //   root = ""
                           //   nested = "photos/summer"
  size: number;            // original plaintext byte count (≥ 0)
  parts: VaultIndexPart[]; // ordered list of encrypted content blocks
  thumbnailUuid?: string;  // UUID filename of the encrypted thumbnail blob (optional)
  thumbnailKeyBase64?: string; // base64-encoded raw AES-256 key for the thumbnail (optional)
}

interface VaultIndexPart {
  uuid: string;        // UUID filename of this part's blob file in blobsDir
  keyBase64: string;   // base64-encoded raw AES-256 key for this part
}
```

#### Example

```json
{
  "version": 1,
  "blobsDir": "blobs",
  "entries": {
    "550e8400-e29b-41d4-a716-446655440000": {
      "name": "photo.jpg",
      "path": "vacation/2025",
      "size": 2097152,
      "parts": [
        {
          "uuid": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
          "keyBase64": "vF9d8vX2folDMmlVIGqDr2YK3KCMAbc123...=="
        }
      ],
      "thumbnailUuid": "a8e91d18-0f25-4a88-850f-2d1a8f5c9d3b",
      "thumbnailKeyBase64": "xyz789...=="
    }
  }
}
```

### 5.4 Key Encoding

File part keys and thumbnail keys are stored as **standard base64** (RFC 4648) strings — the raw 32-byte AES-256 key encoded with `btoa` / `base64.encode`.

```
# Encode
key_bytes = AES_256_GCM_key_raw_bytes  (32 bytes)
key_base64 = base64_encode(key_bytes)  # RFC 4648 standard alphabet, with padding

# Decode
key_bytes = base64_decode(key_base64)
key = AES_256_GCM_import(key_bytes)
```

### 5.5 Adding a File

```
Input: plaintext_bytes, entry_name, path, master_password

1. Generate a new RFC 4122 v4 UUID as the entry identifier: entry_uuid
2. Split plaintext_bytes into consecutive chunks of PART_SIZE bytes.
   If plaintext_bytes is empty, produce exactly one chunk of 0 bytes.

3. For each chunk (in order):
   a. Generate a fresh AES-256 key: part_key = random_aes256_key()
   b. Encrypt as a key-based blob:
      blob = random_salt(16) || random_iv(12) || AES_GCM_Encrypt(part_key, iv, chunk)
   c. Generate a UUID filename: part_uuid
   d. Write blob to blobsDir/part_uuid
   e. Append { uuid: part_uuid, keyBase64: base64(part_key) } to parts list

4. Add entry to index:
   entries[entry_uuid] = { name, path, size: len(plaintext_bytes), parts }

5. Call save vault (section 5.2) to persist index.lock
```

### 5.6 Reading a File

```
Input: entry_uuid, vault_index, blobsDir_path

1. Look up entry = index.entries[entry_uuid]
2. For each part in entry.parts (in array order):
   a. Read file at blobsDir/part.uuid → encrypted_bytes
   b. Decode part.keyBase64 → part_key
   c. Decrypt as key-based blob:
      iv       = encrypted_bytes[16:28]
      ct+tag   = encrypted_bytes[28:]
      chunk    = AES_256_GCM_Decrypt(part_key, iv, ct+tag)
      (fail if authentication fails)
   d. Append chunk to output buffer

3. Output buffer is the original plaintext (total length should equal entry.size)
```

### 5.7 Reading a Thumbnail

Thumbnails are optional. If `thumbnailUuid` and `thumbnailKeyBase64` are both present:

```
1. Read file at blobsDir/entry.thumbnailUuid → encrypted_bytes
2. Decode entry.thumbnailKeyBase64 → thumb_key
3. Decrypt as key-based blob (same as section 5.6 step 2c)
4. Output is the thumbnail bytes (typically a WebP image)
```

### 5.8 Deleting a File

```
1. For each part in entry.parts:
   - Delete file blobsDir/part.uuid
2. If entry.thumbnailUuid exists:
   - Delete file blobsDir/entry.thumbnailUuid
3. Remove entry_uuid from index.entries
4. Save vault
```

### 5.9 Creating a New (Empty) Vault

```
1. Create the vault directory and (optionally) the blobsDir subdirectory
2. Construct initial index:
   {
     "version": 1,
     "blobsDir": "<chosen_subfolder_or_omit>",
     "entries": {}
   }
3. Encrypt and write index.lock (section 5.2)
```

---

## 6. Virtual Folder Hierarchy

Folders are **not** stored as real directories in the vault. They are a virtual hierarchy derived entirely from the `path` field of each `VaultIndexEntry`.

- Root folder: `path = ""`
- Nested folder: `path = "photos/summer"` (no leading or trailing slash)
- Path segments are separated by `/`

To list all files in a given virtual folder, filter entries where `entry.path === target_path` (exact match; subdirectory files are excluded).

---

## 7. Error Conditions

| Condition | Expected behavior |
|---|---|
| Wrong master password | AES-GCM decryption of `index.lock` fails authentication — treat as wrong password |
| Corrupted blob | AES-GCM authentication tag mismatch — fail the operation |
| Missing `index.lock` | Vault is invalid / not a vault directory |
| Part file missing | File is corrupted or partially deleted |
| Thumbnail absent | Both `thumbnailUuid` and `thumbnailKeyBase64` must be present; treat absence of either as no thumbnail |

---

## 8. Implementation Checklist

- [ ] PBKDF2-HMAC-SHA256, 100 000 iterations, 16-byte salt, 32-byte output
- [ ] AES-256-GCM, 12-byte IV, 128-bit tag, no AAD
- [ ] Blob layout: `salt(16) || iv(12) || ciphertext+tag`
- [ ] For key-based blobs, skip bytes 0–15 (salt) and read IV from bytes 16–27
- [ ] 4-byte big-endian uint32 size prefix before each encrypted frame (single-file only)
- [ ] Default frame size is 1 MiB; last frame may be smaller; empty file → one 0-byte frame
- [ ] Vault index is UTF-8 JSON encrypted as a password-based blob in `index.lock`
- [ ] Part/thumbnail keys are raw 32 bytes encoded as standard base64 (RFC 4648)
- [ ] Parts reassembled in array order; total length equals `entry.size`
- [ ] Empty files produce exactly one part containing 0 bytes of plaintext
- [ ] Virtual folders derived from `path` field; root is empty string `""`
- [ ] UUIDs are lowercase RFC 4122 v4 strings (e.g. `"f47ac10b-58cc-4372-a567-0e02b2c3d479"`)
- [ ] A fresh random salt and IV must be generated for every encryption call
