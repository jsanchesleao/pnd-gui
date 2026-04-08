# Vault CLI Implementation Plan

## Overview

The Vault page gives the CLI access to the same encrypted vault format used by the GUI.
A vault is a folder on disk containing an encrypted index (`index.lock`) and UUID-named
encrypted blob files. All cryptographic parameters are shared with the web/Tauri frontend
so vaults created in one tool can be read by the other.

---

## On-Disk Format (read-only reference)

```
my-vault/
├── index.lock          ← AES-GCM encrypted JSON index, key derived from master password
├── <uuid>              ← encrypted file blob (no extension)
├── <uuid>
└── blobs/              ← optional; present only if index.json specifies blobsDir
    └── <uuid>
```

### index.lock

Binary layout: `[salt 16 B][IV 12 B][AES-256-GCM ciphertext]`

Plaintext is UTF-8 JSON:

```json
{
  "version": 1,
  "blobsDir": "blobs",
  "entries": {
    "<fileUuid>": {
      "name": "photo.jpg",
      "path": "photos/summer",
      "size": 3145728,
      "parts": [
        { "uuid": "<blobUuid>", "keyBase64": "<base64 AES-256 key>" }
      ],
      "thumbnailUuid": "<blobUuid>",
      "thumbnailKeyBase64": "<base64 AES-256 key>"
    }
  }
}
```

Key derivation for `index.lock`: PBKDF2-HMAC-SHA256, 100 000 iterations, 32-byte output,
using the 16-byte salt embedded at the front of the file.

### Blob files

Each blob is also `[salt 16 B][IV 12 B][AES-256-GCM ciphertext]`.
The salt in a blob is present for format uniformity but is **not** used for key derivation —
the key comes from `keyBase64` in the index entry.
Decryption: extract IV from bytes `[16:28]`, ciphertext from `[28:]`, decrypt with the
stored key.

### Paths

- Root-level files: `path = ""`
- Nested: `path = "photos/summer"` (forward slashes, no leading/trailing slash)
- File names are stored without path component in the `name` field.

---

## Rust Data Structures

```rust
// pages/vault/types.rs

#[derive(Debug, Deserialize, Serialize)]
pub struct VaultIndex {
    pub version: u32,
    pub blobs_dir: Option<String>,
    pub entries: HashMap<String, VaultEntry>, // keyed by file UUID
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub parts: Vec<VaultPart>,
    pub thumbnail_uuid: Option<String>,
    pub thumbnail_key_base64: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultPart {
    pub uuid: String,
    pub key_base64: String,
}

// ── In-memory vault handle ────────────────────────────────────────────────

pub struct VaultHandle {
    pub root: PathBuf,
    pub blobs_dir: PathBuf,   // == root unless blobsDir is set
    pub password: String,
    pub index: VaultIndex,
}

// ── Errors ────────────────────────────────────────────────────────────────

pub enum VaultError {
    WrongPassword,
    InvalidFormat(String),
    NotFound(String),
    DuplicateName,
    Io(io::Error),
}
```

---

## Crypto Module Additions (`crypto.rs`)

The existing `crypto.rs` handles per-frame PBKDF2 for the single-file format. Vault crypto
is slightly different — the key for blobs comes from the index, not from the password.
Add these functions (can live in a new `pages/vault/crypto.rs` or alongside existing ones):

```rust
/// Derive a 32-byte AES-256 key from a password and a 16-byte salt using
/// PBKDF2-HMAC-SHA256 with 100 000 iterations. Used for index.lock only.
fn pbkdf2_key(password: &str, salt: &[u8; 16]) -> [u8; 32]

/// Decrypt a blob that was encrypted with a known raw AES-256-GCM key.
/// Layout: [salt 16 B (ignored)][IV 12 B][ciphertext + 16 B tag]
/// Returns the plaintext bytes.
fn decrypt_blob(encrypted: &[u8], key_base64: &str) -> Result<Vec<u8>, VaultError>

/// Encrypt bytes with a raw AES-256-GCM key.
/// Writes [salt 16 B (zeros, ignored)][IV 12 B][ciphertext + tag].
fn encrypt_blob(plaintext: &[u8], key_base64: &str) -> Result<Vec<u8>, VaultError>

/// Generate a fresh random base64-encoded AES-256 key.
fn generate_key_base64() -> String

/// Decrypt index.lock with the master password.
fn decrypt_index(path: &Path, password: &str) -> Result<VaultIndex, VaultError>

/// Encrypt and write the index back to index.lock.
fn save_index(handle: &VaultHandle) -> Result<(), VaultError>
```

---

## Module Structure

Following the `pages/preview/` pattern, the vault page becomes a submodule:

```
src/pages/vault/
├── mod.rs          ← public re-exports, draw/handle dispatch
├── types.rs        ← VaultIndex, VaultEntry, VaultPart, VaultHandle, VaultError, Phase
├── crypto.rs       ← pbkdf2_key, decrypt_blob, encrypt_blob, generate_key_base64,
│                      decrypt_index, save_index
├── state.rs        ← VaultState, Phase enum, operations (open, list, move, delete, rename)
├── draw.rs         ← draw_vault(frame, &VaultState)
└── handler.rs      ← handle_vault(&mut App, KeyCode)
```

The existing `pages/vault.rs` file should be replaced by this folder.

---

## Page State Machine

```rust
pub enum Phase {
    // ── Locked ───────────────────────────────────────────────────────────────
    /// Waiting for the user to provide a vault path and master password.
    Locked { vault_path: String, password: String, focus: usize, error: Option<String> },

    // ── Unlocked — browsing ──────────────────────────────────────────────────
    /// Vault is open; user is browsing.
    Browse(BrowseState),

    // ── Overlays on top of Browse ────────────────────────────────────────────
    /// Rename dialog (single input field, pre-filled with current name).
    Rename { uuid: String, input: String },
    /// Confirm deletion dialog.
    ConfirmDelete { uuids: Vec<String> },
    /// Move-destination picker: user navigates the folder tree to choose a target path.
    Move { uuids: Vec<String>, target_path: String },
    /// Preview overlay (reuses the existing preview pipeline).
    Preview(PreviewOverlayState),
}

pub struct BrowseState {
    pub handle: VaultHandle,
    pub current_path: String,       // "" = root
    pub entries: Vec<VaultEntry>,   // files in current_path (re-derived on path change)
    pub folders: Vec<String>,       // immediate child folder names
    pub selected_uuids: HashSet<String>,
    pub list_cursor: usize,         // index in the combined folders+files list
    pub panel_focus: PanelFocus,    // Left (folder tree) or Right (file list)
    pub clipboard: Clipboard,       // cut/paste state
    pub status: BrowseStatus,       // Idle | Saving | Error(String)
}

pub enum PanelFocus { Tree, List }

pub struct Clipboard {
    pub uuids: Vec<String>,
    pub op: ClipOp,
}
pub enum ClipOp { Cut }

pub enum BrowseStatus { Idle, Saving, Error(String) }
```

---

## UI Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Vault — my-vault                              [s]save  [?]  │
├────────────────┬────────────────────────────────────────────┤
│ /              │  Name              Size    Date             │
│ ├ photos/      │  ▶ notes.txt       4 KB    2024-11-01       │
│ │ └ summer/    │  ▶ report.pdf      128 KB  2024-10-28       │
│ └ documents/   │  ▶ archive.zip     2.1 MB  2024-10-15       │
│                │                                             │
│                │                                             │
├────────────────┴────────────────────────────────────────────┤
│  Tab panels  ↑↓ navigate  Enter preview  r rename  d delete  │
│  x cut  p paste  m move  Space select  Esc back             │
└─────────────────────────────────────────────────────────────┘
```

- **Left panel (~25% width):** Folder tree. Arrows expand/collapse; Enter navigates into folder.
- **Right panel (~75% width):** File list for the current path. Shows name, size, and date.
  Selected items are highlighted. Multi-select via `Space`.
- **Hint bar (bottom):** Context-sensitive, updates based on current focus and selection count.
- **Status bar (top right):** Shows `[s] save` when `index` is dirty.

### Folder tree rendering

The tree is built by collecting all distinct path prefixes from the index and sorting them.
Each node is displayed with an indentation level equal to the number of `/` separators in
its path. The currently active path is highlighted.

```
/                    ← path = ""
├ photos/            ← path = "photos"
│ └ summer/          ← path = "photos/summer"
└ documents/         ← path = "documents"
```

---

## Keyboard Bindings

### Locked phase

| Key | Action |
|-----|--------|
| `Tab` / `BackTab` | Cycle focus (vault path → password) |
| `Enter` on path | Open file browser to pick vault folder |
| `Enter` on password | Unlock (decrypt `index.lock`) |
| `Esc` | Back to main menu |

### Browse phase — shared

| Key | Action |
|-----|--------|
| `Tab` | Switch focus between tree panel and list panel |
| `Esc` / `Backspace` | Go up one folder level; if at root, back to Locked |
| `s` | Save index (write `index.lock`) |
| `?` | Toggle help overlay |

### Browse phase — tree panel (left)

| Key | Action |
|-----|--------|
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Enter` / `l` | Navigate into selected folder |
| `h` | Go up one level |

### Browse phase — list panel (right)

| Key | Action |
|-----|--------|
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Space` | Toggle selection on current item |
| `Enter` | Preview file (or enter folder) |
| `r` | Rename (opens Rename overlay); only when single item |
| `d` | Delete (opens ConfirmDelete overlay) |
| `x` | Cut selected items to clipboard |
| `p` | Paste clipboard items (move to current path) |
| `m` | Move selected items (opens Move overlay) |

### Rename overlay

| Key | Action |
|-----|--------|
| character input | Edit the name |
| `Backspace` | Delete last character |
| `Enter` | Confirm rename |
| `Esc` | Cancel |

### ConfirmDelete overlay

| Key | Action |
|-----|--------|
| `y` / `Enter` | Confirm — remove entries from index, **does not delete blob files** |
| `n` / `Esc` | Cancel |

> **Blob files are not deleted from disk** — only the index entry is removed. This matches
> the GUI behaviour. A future "compact vault" command can scrub orphaned blobs.

### Move overlay

The Move overlay reuses the folder tree navigation. The user browses the tree and presses
`Enter` to confirm the destination path. `Esc` cancels.

---

## Operations

### Open vault

```
1. User provides vault folder path + master password.
2. Read index.lock from the folder.
3. Extract salt from bytes [0:16], derive key with PBKDF2.
4. Decrypt ciphertext, parse JSON into VaultIndex.
5. Populate BrowseState with the root listing.
```

### List folder

```
1. Collect all entries where entry.path == current_path.
2. Collect unique immediate child folders (see Folder paths section above).
3. Sort: folders first (alphabetical), then files (alphabetical by name).
```

### Preview file

```
1. Resolve the entry's blob directory (root or blobsDir subfolder).
2. For each part: read blob file by UUID, decrypt with part.keyBase64.
3. Concatenate parts into Vec<u8>.
4. Determine file type from entry.name extension.
5. Dispatch to the existing preview pipeline (render_preview from pages/preview).
   - Images → Kitty / xdg-open
   - Media  → mpv
   - Text   → bat / ratatui viewer
   - ZIP    → gallery
```

The preview is triggered inline from the vault handler; the terminal suspend/resume
pattern is the same as in the Preview page.

### Rename

```
1. Check no other entry in the same path has the new name.
2. Update entry.name in the in-memory index.
3. Mark index as dirty (show [s] save indicator).
4. User must press s to persist.
```

### Delete

```
1. Show ConfirmDelete overlay with item count.
2. On confirm: remove UUIDs from index.entries.
3. Clear any matching UUIDs from the clipboard.
4. Mark index dirty; auto-save immediately (same policy as GUI).
```

### Move (cut + paste / explicit move)

```
1. Collect UUIDs to move.
2. Validate no name collision in destination path.
3. Update entry.path for each UUID.
4. Mark index dirty; auto-save immediately.
```

### Save index

```
1. Serialize index to JSON bytes.
2. Generate fresh random 16-byte salt.
3. Derive key with PBKDF2(password, salt).
4. Generate fresh random 12-byte IV.
5. Encrypt with AES-256-GCM.
6. Write [salt][IV][ciphertext] to index.lock (atomic: write to .tmp then rename).
```

---

## Auto-save Policy

Mirror the GUI: **auto-save immediately** after delete, move, and paste. Rename does not
auto-save — it sets a dirty flag and shows `[s] save` in the status bar, requiring an
explicit `s` keypress. This matches the GUI's behaviour.

---

## Dependencies

No new crates are required beyond what is already in `Cargo.toml`:

| Need | Crate already present |
|------|-----------------------|
| AES-256-GCM | `aes-gcm` |
| PBKDF2 | `pbkdf2` + `hmac` + `sha2` |
| Base64 encode/decode | `base64` |
| UUID generation | `uuid` |
| JSON | `serde` + `serde_json` |
| Temp files | `tempfile` |

Verify that `uuid` has the `v4` feature enabled and `base64` is present.
If either is missing, add them — they are small and compile quickly.

---

## Integration with `main.rs`

### App struct

```rust
pub(crate) struct App {
    // existing fields …
    pub(crate) vault: pages::vault::VaultState,
}
```

### `enter_page` for Vault

```rust
MenuItem::Vault => {
    app.vault = pages::vault::VaultState::new();
    app.file_browser = Some(FileBrowser::open(None, FileBrowserTarget::VaultDir));
}
```

### `apply_browser_selection`

```rust
FileBrowserTarget::VaultDir => {
    app.vault.set_path(path.to_string_lossy().as_ref());
}
```

The `FileBrowser` must support selecting directories in addition to files.
Add a `FileBrowserTarget::VaultDir` variant and a directory-selection mode to
`file_browser.rs` (currently it only selects files).

---

## Nice-to-Have: Folder Gallery

When the user presses `g` on a folder in the list panel (or a dedicated key in the tree
panel), open a gallery overlay that shows all image entries recursively under that folder.

```
Phase::FolderGallery {
    images: Vec<(String, Vec<u8>)>,  // (name, decrypted bytes) pre-loaded in background
    index: usize,
}
```

Rendering reuses `pages/preview/image.rs` (Kitty path) or `pages/preview/gallery.rs`
event loop logic. Navigation: `←`/`→` or `h`/`l`. Loading happens in a background thread
with a progress indicator; the browse UI remains responsive.

---

## Implementation Order

1. (done) **`types.rs`** — data structures and error type (no I/O, easy to test)
2. (done) **`crypto.rs`** — `pbkdf2_key`, `decrypt_blob`, `decrypt_index` (verify against a vault
   created by the GUI before proceeding)
3. (done) **`state.rs`** — `VaultState`, open/list logic, Phase transitions
4. (done) **`draw.rs`** — locked form first, then two-panel browse layout
5. (done) **`handler.rs`** — key routing, starting with navigation only
6. (done) **Wire into `main.rs`** — add `vault` field, update `apply_browser_selection`,
   add `FileBrowserTarget::VaultDir`
7. (done) **Create Vault** — add option to create a vault by choosing a directory
8. **Operations** — rename, delete, move/paste, save
9. **Preview integration** — decrypt and forward to existing preview pipeline
10. **Folder gallery** (optional, last)
