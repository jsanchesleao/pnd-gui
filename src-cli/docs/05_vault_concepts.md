# Vault Concepts

A **vault** is a directory on disk that stores an arbitrary number of encrypted files.
Unlike the single-file format, a vault manages many files under one password and
organises them into a virtual folder hierarchy.

## On-disk layout

```
my-vault/
├── index.lock          ← encrypted JSON index (the vault's directory)
├── 3f2a1b04-...        ← encrypted blob file (UUID filename)
├── 9c7e5d12-...        ← another blob file
└── ...
```

Optionally, blobs may live in a subdirectory (controlled by the `blobsDir` field in the
index). When present, the layout looks like:

```
my-vault/
├── index.lock
└── blobs/
    ├── 3f2a1b04-...
    └── 9c7e5d12-...
```

### `index.lock`

The index is a JSON object that is encrypted with a password-derived AES-256-GCM key
(PBKDF2-HMAC-SHA256, 100 000 iterations). It contains:

- `version` — format version number
- `blobsDir` — optional subdirectory name for blob files
- `entries` — a map of UUID → entry metadata

Each **entry** records:

| Field | Description |
|---|---|
| `name` | Filename only, no path component (`"photo.jpg"`) |
| `path` | Virtual folder path, no leading/trailing slash (`"photos/summer"` or `""` for root) |
| `size` | Original plaintext byte count |
| `parts` | Array of blob parts (UUID + base64 AES-256 key) |
| `thumbnailUuid` | Optional UUID of a thumbnail blob |
| `thumbnailKeyBase64` | Optional base64 key for the thumbnail blob |

### Blob files

Each blob file holds one part of a file's encrypted content. Large files (> 256 MiB) are
split across multiple blobs. The blob's AES-256 key is stored (base64-encoded) in the
`index.lock` entry — blobs do **not** use the vault password directly.

Blob layout: `[salt 16 B (zeroed)][IV 12 B][AES-256-GCM ciphertext + 16 B tag]`

The salt field is present for format uniformity but is zeroed for blobs because their key
is not password-derived.

## Virtual folder hierarchy

Folders are **virtual**: they exist only as the `path` prefix of index entries. There are
no empty-folder blobs on disk. The path `"photos/summer"` on an entry means that entry
lives in the `photos` folder, inside the `summer` subfolder.

Path conventions:
- The **root** is the empty string `""`.
- Segments are separated by `/`.
- No leading or trailing `/`.
- Leading and trailing slashes in user-supplied paths are silently normalised.

## Cryptographic separation

The vault uses two distinct key types:

| Key type | Used for | Derivation |
|---|---|---|
| Password-derived key | `index.lock` | PBKDF2-HMAC-SHA256 (100 k iters, fresh random salt) |
| Per-blob raw key | Each blob file | 32 random bytes, stored in `index.lock` |

This means compromising a single blob does not expose other blobs — each has its own
independent random key — but the index is the single point that ties everything together
and is protected by the vault password.

## Atomic index saves

Every command that modifies the vault (add, rename, move, delete) writes the updated
index to a temporary file in the vault directory and then renames it to `index.lock`.
This ensures the vault is never left in a state where `index.lock` is partially written.

## Compatibility with pnd-gui

Vaults created and modified by `pnd-cli` are fully compatible with the pnd-gui desktop
application and vice versa. Both tools read and write the same `index.lock` format and
the same blob layout.
