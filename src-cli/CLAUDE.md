# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo run              # debug build and run
cargo run --release    # release build (enables AES-NI via .cargo/config.toml target-cpu=native)
cargo build --release  # produce ./target/release/pnd-cli
```

There are automated tests. Also consider build errors in the feedback loop — always run `cargo test` and `cargo build` after changes.

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

### Page structure

Each page follows the same three-file split inside `src/pages/<page>/`:

| File | Responsibility |
|---|---|
| `state.rs` | Data types, `WorkerMsg` enums, background thread spawning, `poll_progress()` |
| `draw.rs` | Pure `draw_*(frame, &State)` functions — no mutation |
| `handler.rs` | `handle_*(app, KeyCode)` — mutates app state, opens file browsers |

`src/pages/vault.rs` (not a directory `mod.rs`) is the vault module root; it uses `#[path]` attributes to pull in the `vault/` subfolder files. This is the pattern to follow when adding vault-related code.

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

**`VaultState` phase machine** (`state.rs`):
- `VaultMenu` → `Locked` / `Creating` → `Opening` → `Browse`
- Overlay phases on top of Browse: `Rename`, `ConfirmDelete`, `Move`, `NewFolder`, `Adding`, `Previewing`, `PreviewReady`, `Exporting`
- `BrowseState` is separate from `VaultState` and present whenever the vault is unlocked.
- Virtual folders exist only as path prefixes of index entries. Session-only empty folders live in `BrowseState::extra_folders` and are merged into `all_folders` during `refresh()`.
- Status messages are timed: `set_status(msg)` records `Instant::now()`; `tick(secs)` called each loop iteration auto-clears them.

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
