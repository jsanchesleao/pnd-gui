# CLI Arguments Specification

This document specifies the command-line arguments for `pnd-cli` and how each mode
should behave, including edge cases and an implementation roadmap.

---

## Conventions

- **Non-interactive mode** (default when a subcommand is given): runs, prints output to
  stdout/stderr, exits with a code. No TUI.
- **TUI mode** (no args, or `-t` flag): launches the ratatui interface.
- **Password input**: always read from the terminal via a hidden prompt (no echo).
  Never accepted as a plain CLI argument (security: visible in `ps`, shell history).
  For scripting, the environment variable `PND_PASSWORD` is honoured — if set, it is
  used instead of prompting. Warn on stderr when `PND_PASSWORD` is set.
- **Output paths**: encryption appends `.lock`; decryption strips `.lock`. See per-command
  notes for collision handling.
- **Exit codes**:
  - `0` — success
  - `1` — wrong password / decryption authentication failure
  - `2` — I/O or filesystem error
  - `3` — bad arguments / usage error
  - `4` — output file already exists (only when `--force` is not given)

---

## Commands

### No arguments — interactive TUI

```
pnd-cli
```

Opens the main menu TUI (current behaviour).

---

### Encrypt / Decrypt a single file

```
pnd-cli [OPTIONS] <file>
```

Detects mode from the file extension:
- `.lock` extension → **decrypt** → output is `<file>` with `.lock` stripped
- any other extension → **encrypt** → output is `<file>.lock`

**Options:**

| Flag | Description |
|------|-------------|
| `-o <path>` | Write output to `<path>` instead of the default location |
| `-f`, `--force` | Overwrite the output file if it already exists |
| `-t`, `--tui` | Open the TUI Encrypt/Decrypt screen with `<file>` pre-loaded instead of running non-interactively |

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| `<file>` does not exist | Print error to stderr, exit 2 |
| Output path already exists and `--force` not given | Print error to stderr, exit 4 |
| Wrong password (decryption auth fails) | Print error to stderr, exit 1 |
| `<file>` is a directory | Print error to stderr, exit 3 |
| `-o` and `-t` combined | `-t` wins; `-o` is ignored with a warning |
| Disk full mid-write | Partial output is deleted; exit 2 |
| `<file>` is already open (locked on Windows) | Exit 2 with a descriptive message |

---

### Preview a file

```
pnd-cli -p [OPTIONS] <file>
```

Decrypts `<file>` into memory (never to disk) and opens a preview:
- Images → Kitty inline protocol, or `xdg-open` fallback
- Video/audio → mpv
- Text/code/markdown → `bat` or inline ratatui viewer
- ZIP → image gallery

`<file>` may be a plain (non-encrypted) file; in that case no password is needed and
the preview is launched directly.

**Options:**

| Flag | Description |
|------|-------------|
| `-t`, `--tui` | Open the TUI Preview screen with `<file>` pre-loaded instead |

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| `<file>` does not exist | Print error to stderr, exit 2 |
| Wrong password | Print error to stderr, exit 1 |
| File too large to hold in memory | Print error to stderr, exit 2; suggest `--vault` for large files |
| Unsupported file type | Print message: "No previewer for `.<ext>` files", exit 0 |
| mpv not installed (media file) | Print install hint to stderr, exit 2 |
| Non-Kitty terminal (image file) | Fall back to `xdg-open`; if `xdg-open` unavailable, exit 2 with hint |
| Interrupted mid-decrypt (Ctrl-C) | Decrypted bytes are never written; exit 130 |

---

### Vault — open interactively

```
pnd-cli --vault [<vault-dir>]
```

Opens the vault at `<vault-dir>` (defaults to the current directory if omitted) in the
TUI vault browser with the password pre-prompted. Equivalent to `pnd-cli`, navigating to
Vault, and choosing Open.

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| `<vault-dir>` does not exist | Print error to stderr, exit 2 |
| `<vault-dir>` is not a vault (no `index.lock`) | Print error: "No vault found at `<path>`", exit 2 |
| Wrong password | Print error to stderr, exit 1 (or show the TUI error form if in TUI mode) |
| `<vault-dir>` is a file, not a directory | Print error to stderr, exit 3 |

---

### Vault — list contents

```
pnd-cli --vault-list [<vault-dir>]
```

Non-interactive. Decrypts `index.lock`, prints all vault entries to stdout.

Default output (human-readable, one entry per line):
```
photos/summer/beach.jpg   (3.1 MB)
documents/report.pdf      (128 KB)
notes.txt                 (4 KB)
```

**Options:**

| Flag | Description |
|------|-------------|
| `--json` | Print entries as a JSON array: `[{"path":"...","name":"...","size":...}, ...]` |
| `--path <vault-path>` | List only entries under this virtual folder (e.g. `photos/summer`) |

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| Vault is empty (no entries) | Print nothing (or `[]` for `--json`); exit 0 |
| `<vault-dir>` missing or not a vault | Same as `--vault` |
| `--path` does not match any folder | Print nothing / `[]`; exit 0 |
| Wrong password | Print error to stderr, exit 1 |

---

### Vault — preview a file

```
pnd-cli --vault-preview <vault-path> [<vault-dir>]
```

Non-interactive decrypt + preview. `<vault-path>` is the virtual path inside the vault
(e.g. `photos/summer/beach.jpg` or just `notes.txt` for a root-level file).
`<vault-dir>` defaults to the current directory.

Behaviour after decryption mirrors `pnd-cli -p` (Kitty / mpv / bat / gallery).

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| `<vault-path>` not found in index | Print error to stderr, exit 2 |
| `<vault-path>` is ambiguous (matches multiple entries) | Should not happen — vault entries are keyed by UUID, paths are unique. Print the first match and warn. |
| Blob file missing from disk | Print error to stderr (corrupted vault), exit 2 |
| Wrong password | Exit 1 |
| Unsupported file type, no previewer | Exit 0 with a message |

---

### Vault — add a file

```
pnd-cli --vault-add <file> [<vault-path>] [<vault-dir>]
```

Encrypts `<file>` and adds it to the vault.
- `<vault-path>` is the virtual folder inside the vault where the file will be placed
  (e.g. `photos/summer`). Defaults to root (`""`).
- `<vault-dir>` defaults to the current directory.

The index is saved atomically after a successful add (write to `.tmp` then rename).

**Options:**

| Flag | Description |
|------|-------------|
| `-f`, `--force` | If a file with the same name already exists at `<vault-path>`, replace it |

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| `<file>` does not exist | Exit 2 |
| Name collision in vault and `--force` not given | Print error: "A file named `<name>` already exists at `<vault-path>`", exit 4 |
| `<file>` is a directory | Exit 3 (directories not supported; suggest adding files individually) |
| Disk full while writing blob | Partial blob is deleted; index is not updated; exit 2 |
| Wrong password | Exit 1 |
| `<vault-path>` contains a leading or trailing `/` | Normalise silently (strip them) |

---

### Vault — export (decrypt to disk)

```
pnd-cli --vault-export <vault-path> [--dest <dest-dir>] [<vault-dir>]
```

Decrypts the file at `<vault-path>` in the vault and writes it to `<dest-dir>` (defaults
to the current directory). The output filename is taken from the vault entry's `name`
field.

**Options:**

| Flag | Description |
|------|-------------|
| `--dest <dir>` | Destination directory (default: `.`) |
| `-f`, `--force` | Overwrite the destination file if it already exists |

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| `<vault-path>` not found | Exit 2 |
| Destination file already exists and `--force` not given | Exit 4 |
| `--dest` directory does not exist | Exit 2 with a message; do not create it automatically |
| Wrong password | Exit 1 |
| Blob file missing | Exit 2 (corrupted vault) |

---

## Global Options

These flags apply to all commands:

| Flag | Description |
|------|-------------|
| `-h`, `--help` | Print usage and exit 0 |
| `--version` | Print version string and exit 0 |
| `--vault-dir <path>` | Alternative to positional `<vault-dir>` for all vault commands |

---

## Argument Parsing Notes

- Positional `<vault-dir>` is always the **last** positional argument for vault commands,
  to avoid ambiguity with `<vault-path>` and `<file>`.
- If both `--vault-dir` and a positional `<vault-dir>` are given, the flag wins and the
  positional is an error (exit 3).
- Unknown flags always exit 3.

---

## Implementation Roadmap

The commands are ordered from simplest to most complex. Each phase can be merged and
used independently.

### Phase 1 — Argument parsing skeleton

Add argument parsing to `main.rs` (using `std::env::args()` directly or the `clap`
crate). Detect whether any arguments are present:
- Zero args → launch TUI (current behaviour, no change needed).
- One or more args → parse and dispatch.

Implement `--help` and `--version` (always non-interactive, no crypto).

**Acceptance:** `pnd-cli --help` and `pnd-cli --version` print and exit.

---

### Phase 2 — Single-file encrypt/decrypt (non-interactive)

Implement `pnd-cli <file>` without `-t`.

Requires:
- Password prompt (hidden stdin, or `PND_PASSWORD` env var).
- Call the existing `crypto.rs` streaming encrypt/decrypt functions.
- Determine output path (append/strip `.lock`).
- Handle the edge cases in the table above.

This does **not** require any TUI code. It is purely I/O + crypto.

**Acceptance:** `pnd-cli file.txt` encrypts to `file.txt.lock`; `pnd-cli file.txt.lock`
decrypts back to `file.txt`. Wrong password exits 1.

---

### Phase 3 — `-t` / `--tui` flag for single-file and preview

Add a `--tui` flag that, when present, launches the TUI with the given file pre-loaded
into the relevant page.

For EncDec: pre-populate the path field and advance focus to the password field.
For Preview: pre-populate the path field and start decryption immediately.

This reuses existing TUI code; the new work is pre-loading state before entering the
event loop.

**Acceptance:** `pnd-cli -t file.txt` opens the TUI Encrypt/Decrypt screen with
`file.txt` already in the path field.

---

### Phase 4 — Preview non-interactive (`pnd-cli -p <file>`)

Implement non-interactive preview. Decrypt into memory, then dispatch to the existing
preview rendering pipeline (`pages::preview::render_preview`).

Note: `render_preview` already handles suspending the TUI; in non-interactive mode a
minimal `Terminal` is still needed to call it. The simplest approach is to initialise
the terminal, call `render_preview`, then tear it down — the same pattern used by the
current TUI when a preview is triggered.

**Acceptance:** `pnd-cli -p image.jpg.lock` decrypts and renders the image inline on
a Kitty terminal.

---

### Phase 5 — `--vault` (open vault in TUI)

Implement `pnd-cli --vault [<vault-dir>]`. This is mostly wiring:
1. Parse `<vault-dir>` (default `.`).
2. Prompt for password.
3. Initialise `VaultState`, call `start_unlock()`.
4. Launch the TUI event loop starting on the Vault screen.

The vault TUI is already fully implemented; this phase just skips the main menu.

**Acceptance:** `pnd-cli --vault ~/vaults/myvault` opens the vault browser directly.

---

### Phase 6 — `--vault-list`

Non-interactive. Read index only (no blob I/O). Print entries.

Requires extracting vault-open logic (PBKDF2 + AES decrypt of `index.lock`) into a
function callable without the TUI. This already exists in `pages/vault/crypto.rs`
(`decrypt_index`).

**Acceptance:** `pnd-cli --vault-list` prints all file names with paths and sizes.
`--json` flag outputs structured JSON.

---

### Phase 7 — `--vault-preview`, `--vault-export`

Both share a common "decrypt blob(s) to memory" step, already implemented in the vault
state machine's preview/export workers. Extract these into standalone functions (or call
the existing `decrypt_blob_with_key` directly) for non-interactive use.

`--vault-preview` then reuses the same rendering dispatch as Phase 4.
`--vault-export` writes the decrypted bytes to disk with the standard output-collision
logic.

---

### Phase 8 — `--vault-add`

Requires encrypting a file into a vault blob and updating `index.lock`. The encryption
side (`encrypt_blob`) and index-save (`save_index`) already exist in
`pages/vault/crypto.rs`. The new work is:
- Reading and parsing the existing index.
- Collision detection.
- Writing the new blob UUID file.
- Saving the updated index atomically.

This is the only write command and should be tested carefully with corrupted-vault
recovery in mind (partial write → index not updated).

---

## Open Questions

1. **`clap` vs manual parsing**: `clap` gives `--help` generation, validation, and shell
   completion for free but adds a compile-time dependency. Manual parsing keeps the
   binary lean. Decide before Phase 1.

2. **`PND_PASSWORD` env var**: Convenient for scripting but a security risk if the
   environment is visible to other processes. Consider requiring an explicit opt-in flag
   (e.g. `--allow-env-password`) before reading it.

3. **Multiple files**: `--vault-add` taking a single file is a limitation. A glob or
   repeated flag (`--vault-add file1 --vault-add file2`) would be more useful. Defer to
   a later iteration.

4. **`--vault-dir` as a default from config**: A `~/.config/pnd/config.toml` or
   `PND_VAULT` env var pointing to the default vault directory would avoid repeating
   the path. Out of scope for the initial implementation but worth noting.

5. **Progress output in non-interactive mode**: For large files, a simple progress line
   (`Encrypting… 45%\r`) on stderr keeps the UX reasonable without requiring the TUI.
   Use `\r` to overwrite in place on a TTY; suppress entirely when stderr is not a TTY
   (i.e. when piped).
