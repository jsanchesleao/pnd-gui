# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo run              # debug build and run
cargo run --release    # release build (enables AES-NI via .cargo/config.toml target-cpu=native)
cargo build --release  # produce ./target/release/pnd-cli
cargo test             # run all unit + integration tests
```

There are automated tests. Always run `cargo test` and `cargo build` after changes.

For a portable binary not tied to the build machine's CPU, override the rustflags in `.cargo/config.toml`:
```
rustflags = ["-C", "target-feature=+aes,+ssse3,+pclmulqdq"]
```

## Architecture

### Top-level shell (`src/main.rs`)

Owns the ratatui event loop, the `App` struct, and cross-cutting concerns:

- **`App`** holds one state object per page (`enc_dec`, `preview`, `vault`) plus an optional `file_browser` overlay.
- **`Screen` / `MenuItem`** — flat enum pair that determines what draws and what handles keys.
- **Event loop** — polls every 50 ms while any background worker is running (`is_opening`, `is_adding`, `is_previewing`, `is_exporting`, `has_pending_status`), otherwise blocks on `event::read`. Background workers communicate via `mpsc::channel`.
- **`apply_browser_selection` / `apply_browser_multi_selection`** — route `FileBrowserEvent` results to the correct page handler based on `FileBrowserTarget`.
- Palette constants `ACCENT`, `DIM`, `SUCCESS`, `FAILURE` are `pub(crate)` here and imported by all page submodules via `crate::`.

### Non-interactive CLI modules

Each non-interactive command lives in its own `src/<name>_cli.rs` file. All entry points are `pub fn run(cli: &Cli) -> !` and never return — they always call `process::exit`.

| Module | Flag | Description |
|---|---|---|
| `enc_dec_cli.rs` | `<FILE>` (positional) | Encrypt or decrypt a single file. Mode auto-detected from `.lock` extension. |
| `preview_cli.rs` | `-p` / `--preview` | Decrypt a file to memory and open a preview. |
| `vault_init_cli.rs` | `--vault-init [DIR]` | Create a new empty vault. Prompts password twice. |
| `vault_list_cli.rs` | `--vault-list [DIR]` | List vault contents; supports `--json` and `--path`. |
| `vault_add_cli.rs` | `--vault-add FILE...` | Encrypt files and add them to the vault. |
| `vault_op_cli.rs` | `--vault-preview`, `--vault-export` | Preview or export a vault entry. |
| `vault_rmd_cli.rs` | `--vault-rename`, `--vault-move`, `--vault-delete` | Index-only mutations. |

**Dispatch order** in `main.rs`: `--tui` → zero-args TUI → `<FILE>` enc/dec → `-p` preview → `--vault` TUI → `--vault-list` → `--vault-preview` → `--vault-export` → `--vault-add` → `--vault-rename/move/delete` → `--vault-init`.

**Exit code conventions** (consistent across all modules):
- `0` — success
- `1` — wrong password / corrupt data
- `2` — I/O error or resource not found
- `3` — bad arguments / incompatible flags
- `4` — output collision (file/vault already exists)

### Password module (`src/password.rs`)

- `read_password()` — single prompt; honours `PND_PASSWORD` env var (warns on stderr). Used by all commands that open an existing vault or decrypt a file.
- `read_password_with_confirm()` — prompts twice and loops until both entries match; `PND_PASSWORD` bypasses confirmation. Used only by `--vault-init` where a typo would permanently lock the vault.

### `--stdout` / `-c` flag (Phase 10-A)

Added to `enc_dec_cli.rs` and `vault_op_cli.rs`. When set:
- Output is written directly to `io::stdout()` — no temp file or atomic rename.
- Progress output is suppressed unconditionally.
- `-o PATH` is ignored with a warning when combined with `--stdout`.
- `--stdout` + `--tui` → exit 3.
- `--vault-export --stdout` on a folder path → exit 3.
- `--vault-export --stdout -r` → exit 3.

### File browser overlay (`src/file_browser.rs`)

Self-contained overlay used by all pages. Key design points:
- Three constructors: `open` (file select), `open_for_dir` (directory select), `open_multi` (multi-file select with Space-toggle).
- In `select_dirs` mode a `"."` entry is prepended so users can select the CWD; `".."` allows going up; Enter always navigates into directories — Space confirms the highlighted directory.
- `FileBrowserTarget` must have an arm in both `apply_browser_selection` and `apply_browser_multi_selection` in `main.rs` whenever a new variant is added.

### Vault page (`src/pages/vault/`)

The most complex page; has its own crypto module separate from the top-level `src/crypto.rs`.

**Two distinct crypto formats:**
- `src/crypto.rs` — single-file format: 64 MiB frames, PBKDF2 key per frame, used by EncDec and Preview pages.
- `src/pages/vault/crypto.rs` — vault format: PBKDF2 only for `index.lock`; blob files use a raw AES-256 key stored (base64) in the index entry. Layout for both: `[salt 16 B][IV 12 B][ciphertext+tag]`.

**Key functions in `src/pages/vault/crypto.rs`:**
- `create_vault(root, blobs_dir_name, password)` — writes initial `index.lock`; `blobs_dir_name = None` stores blobs alongside the index.
- `open_vault(root, password)` — decrypts and parses `index.lock`.
- `save_vault(handle)` — re-encrypts the in-memory index atomically (`index.lock.tmp` → rename).
- `encrypt_file_to_vault(path, blobs_dir, vault_path)` — encrypts a file into blob(s) and returns a `VaultEntry`.

**`VaultState` phase machine** (`state.rs`):
- `VaultMenu` → `Locked` / `Creating` → `Opening` → `Browse`
- Overlay phases on top of Browse: `Rename`, `ConfirmDelete`, `Move`, `NewFolder`, `Adding`, `Previewing`, `PreviewReady`, `Exporting`
- `BrowseState` is separate from `VaultState` and present whenever the vault is unlocked.
- Virtual folders exist only as path prefixes of index entries. Session-only empty folders live in `BrowseState::extra_folders` and are merged into `all_folders` during `refresh()`.
- Status messages are timed: `set_status(msg)` records `Instant::now()`; `tick(secs)` called each loop iteration auto-clears them.

**TUI create-vault form** has three fields (focus 0/1/2): vault folder, blobs subfolder (optional — empty string → `None`), master password. This maps directly to the `--vault-init` + `--blobs-dir` CLI flags.

**Preview from vault** (`vault.rs` → `render_vault_preview`): decrypts entry bytes in a background thread, transitions to `Phase::PreviewReady`, then the main loop calls `render_vault_preview` which creates a temporary `PreviewState` and delegates to the existing `pages::preview::render_preview` pipeline.

### Preview page (`src/pages/preview/`)

Two-phase render model to keep terminal-manipulation off the draw path:
1. Worker thread sends `DecryptedBytes(Vec<u8>, ext)` → phase becomes `PendingRender`.
2. Main loop detects `PendingRender` and calls `render_preview` **before** `terminal.draw`.
3. `render_preview` dispatches by extension: `image::` (Kitty protocol or xdg-open), `media::` (mpv), `gallery::` (ZIP carousel), `text::` (bat or ratatui viewer).

### Shared drawing primitives (`src/pages/widgets.rs`)

`outer_block(title)`, `input_block(label, focused)`, `tail_fit(s, cols)` — import via `crate::pages::widgets`.

### Popup height rule

Overlay popups use `centered_popup(area, percent_w, height)`. The minimum height is `content_rows + 2` (borders) `+ 2` (margin). Short-changing this makes input widgets invisible — always verify by adding 1 row of slack.

## Integration tests (`tests/`)

| File | What it covers |
|---|---|
| `tests/stdout_smoke.rs` | `--stdout` / `-c` flag on encrypt, decrypt, and vault-export |
| `tests/vault_init_smoke.rs` | `--vault-init`, `--blobs-dir`, and vault-export `--stdout` happy path |

Integration tests invoke the compiled binary via `std::process::Command` and use `PND_PASSWORD` to avoid interactive prompts. Use this pattern when adding new integration tests for CLI commands.
