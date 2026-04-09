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
pnd-cli --vault-add <file>... [--vault-path <vault-path>] [<vault-dir>]
```

Encrypts one or more files and adds them to the vault.
- `--vault-path` is the virtual folder inside the vault where the files will be placed
  (e.g. `photos/summer`). Defaults to root (`""`).
- `<vault-dir>` defaults to the current directory.
- When adding a single file, `<vault-path>` and `<vault-dir>` may also be given as
  positional arguments for convenience: `pnd-cli --vault-add file [<vault-path>] [<vault-dir>]`.
  When multiple files are given, `--vault-path` and `--vault-dir` must be named flags to
  avoid ambiguity.

The index is saved atomically after a successful add (write to `.tmp` then rename).
If multiple files are given and one fails, files added before the failure are kept; the
failed file and any remaining files are skipped.

**Options:**

| Flag | Description |
|------|-------------|
| `--vault-path <path>` | Virtual folder inside the vault (default: root) |
| `--vault-dir <dir>` | Vault directory (default: `.`) |
| `-f`, `--force` | If a file with the same name already exists at `<vault-path>`, replace it |

**Edge cases:**

| Situation | Behaviour |
|-----------|-----------|
| A `<file>` does not exist | Print error for that file, skip it, continue with the rest; exit 2 at the end |
| Name collision and `--force` not given | Print error: "A file named `<name>` already exists at `<vault-path>`", skip it; exit 4 |
| `<file>` is a directory | Skip with exit 3 message (directories not supported) |
| Disk full while writing blob | Partial blob is deleted; index is not updated for that file; exit 2 |
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

## Decisions

1. **Argument parsing: use `clap`**. The command surface is large enough that manual
   parsing would duplicate validation, `--help` generation, and error messages that
   `clap` provides for free. Compile-time cost is acceptable; binary size increase
   (~300–500 KB) is fine for a CLI tool.

2. **`PND_PASSWORD` env var: honour it with a stderr warning**. An explicit opt-in flag
   (`--allow-env-password`) would add scripting friction without meaningful security
   benefit — anyone who can set env vars already has equivalent access. The warning is
   sufficient disclosure.

3. **Multiple files for `--vault-add`: variadic positionals**. `pnd-cli --vault-add
   file1.jpg file2.pdf` is supported. To avoid positional ambiguity when multiple files
   are given, `--vault-path` and `--vault-dir` must be named flags in that case. Single-
   file invocations retain positional convenience. See the `--vault-add` section above.

4. **`--vault-dir` default from config: out of scope**. The planned future mechanism is
   a `PND_VAULT` env var (consistent with `PND_PASSWORD`). No config file in the initial
   implementation.

5. **Progress in non-interactive mode**: write `Encrypting… 45%\r` to stderr on a TTY;
   suppress entirely when stderr is not a TTY (piped/redirected). Already reflected in
   the roadmap.

---

## Implementation Checklist

### Phase 1 — Argument parsing skeleton
- [ ] Add `clap` to `Cargo.toml`
- [ ] Define top-level `Cli` struct with all subcommands/flags
- [ ] Zero args → launch TUI (existing behaviour, no change)
- [ ] `--help` prints usage and exits 0
- [ ] `--version` prints version string and exits 0
- [ ] Unknown flags exit 3

### Phase 2 — Single-file encrypt/decrypt (non-interactive)
- [ ] Password prompt (hidden stdin)
- [ ] `PND_PASSWORD` env var support with stderr warning
- [ ] Auto-detect encrypt vs decrypt from `.lock` extension
- [ ] Default output path logic (append / strip `.lock`)
- [ ] `-o <path>` override for output path
- [ ] `-f` / `--force` to allow overwriting existing output
- [ ] Exit codes: 0 success, 1 wrong password, 2 I/O error, 3 bad args, 4 file exists
- [ ] Partial output deleted on failure
- [ ] Progress line on stderr when stderr is a TTY (`\r` overwrite)

### Phase 3 — `-t` / `--tui` flag
- [ ] `pnd-cli -t <file>` opens TUI EncDec screen with path pre-loaded
- [ ] `pnd-cli -p -t <file>` opens TUI Preview screen with path pre-loaded
- [ ] `-o` ignored with a warning when combined with `-t`

### Phase 4 — Preview non-interactive (`pnd-cli -p <file>`)
- [ ] Decrypt to memory (never to disk)
- [ ] Plain (non-encrypted) file bypasses password prompt and crypto
- [ ] Dispatch to existing `render_preview` pipeline (Kitty / mpv / bat / gallery)
- [ ] Graceful messages for unsupported type, missing mpv, non-Kitty terminal
- [ ] Ctrl-C during decrypt leaves no bytes on disk; exit 130

### Phase 5 — `--vault` (open vault in TUI)
- [ ] Parse optional `<vault-dir>` (default `.`)
- [ ] Validate directory exists and contains `index.lock`
- [ ] Prompt for password, call `start_unlock()`
- [ ] Launch TUI event loop starting on the Vault screen

### Phase 6 — `--vault-list`
- [ ] Decrypt `index.lock`, print entries (path, size) one per line
- [ ] `--json` flag outputs a JSON array
- [ ] `--path <vault-path>` filters to a virtual subfolder
- [ ] Empty vault prints nothing / `[]`; exit 0

### Phase 7 — `--vault-preview` and `--vault-export`
- [ ] `--vault-preview`: decrypt blob to memory, dispatch to `render_preview`
- [ ] `--vault-export`: decrypt blob, write to `--dest` dir (default `.`)
- [ ] `--vault-export` respects `-f` / `--force` for collision handling
- [ ] Shared "decrypt blob" logic extracted into a standalone function

### Phase 8 — `--vault-add`
- [ ] Accept one or more `<file>` arguments
- [ ] `--vault-path` and `--vault-dir` as named flags when multiple files are given
- [ ] Single-file convenience: positional `<vault-path>` and `<vault-dir>`
- [ ] Read and parse existing `index.lock`
- [ ] Collision detection; `-f` / `--force` to replace
- [ ] Write blob UUID file atomically
- [ ] Save updated index atomically (`.tmp` → rename)
- [ ] On multi-file add, keep successful adds even if later files fail
- [ ] `PND_VAULT` env var recognised as default vault dir (future, note only)
