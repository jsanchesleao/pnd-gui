# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
npm run dev        # Start Vite dev server (browser only, no Tauri native APIs)
npm run build      # tsc + Vite production build
npm run test       # Run Vitest unit tests
npm run tauri dev  # Start full Tauri desktop app (requires Rust toolchain)
npm run tauri build # Build distributable desktop app
```

Run a single test file:
```bash
npx vitest run src/utils/crypto.test.ts
```

## Architecture

**pnd-gui** is a Tauri 2 desktop app (React + TypeScript frontend, minimal Rust backend) for password-based file encryption with three features: Encrypt/Decrypt, Preview, and Vault.

### Core utilities (`src/utils/`)

- **`crypto.ts`** — All cryptography. PBKDF2 (100k iterations, SHA-256) for key derivation, AES-256-GCM for encrypt/decrypt. Exposes stream-based APIs (`createEncryptedStream`, `createDecryptedStream`) and byte-level APIs (`encryptBytesWithKey`, `decryptBytesWithKey`) for vault use.
- **`frames.ts`** — Stream chunking protocol. Encryption splits data into fixed 1 MB frames; each frame gets its own IV and is prefixed with a 4-byte big-endian size header. Decryption reassembles via matching transforms.
- **`vault.ts`** — Vault state machine. A vault is a folder containing `index.lock` (an AES-encrypted JSON index mapping file UUIDs to block keys and metadata) plus UUID-named encrypted block files (256 MB max each). `VaultState` is kept in memory; call `saveVault()` to persist.
- **`mediaTypes.ts`** — Maps extensions to `FileCategory` (`image | video | audio | document | archive | code | other`) used throughout the UI.
- **`videoThumbnail.ts`** — Seeks a video blob to 2 s, captures a canvas frame, exports as WebP.

### Component structure (`src/components/`)

- **`GenericPage/`** — Encrypt/Decrypt tab. Streams a file through `createEncryptedStream`/`createDecryptedStream` with progress tracking.
- **`PreviewPage/`** — Detects file type (strips `.lock` suffix), decrypts to memory, then delegates to `ImageViewerPage`, `VideoPlayerPage`, or `GalleryPage` (ZIP decompressed with fflate).
- **`VaultPage/`** — Most complex area:
  - `index.tsx` — Lifecycle: idle → unlock → browse. Holds `vaultRef` (stable ref to avoid stale closures), manages thumbnail queue processed serially.
  - `VaultBrowser.tsx` — Two-panel layout (folder tree left, file list right).
  - `VaultFolderTree.tsx` — Virtual folder hierarchy derived from `entry.path` fields in the index.
  - `VaultFileList.tsx` — File list/grid with sort (name/type/size/date × asc/desc) and view mode (list/grid). Thumbnail loading is lazy via `onEnqueueThumbnail`.
  - `VaultPreviewPanel.tsx` — In-vault preview overlay (decrypts on demand, same viewers as PreviewPage).

### Data flow summary

**Single file encrypt/decrypt:** File → `createFixedSizeFramesStream` → `createFrameMapperStream` (PBKDF2 + AES-GCM per frame) → `createVariableSizeFrameJoinStream` → write via File System Access API.

**Vault add file:** Bytes → split into 256 MB blocks → each block encrypted with a fresh random AES key → UUID-named file written to vault folder → block UUIDs + base64 keys recorded in in-memory index → index serialized and encrypted to `index.lock` on save.

### Patterns to follow

- **Error handling:** Vault operations throw `VaultError` with typed codes (`WRONG_PASSWORD`, `INVALID_FORMAT`, `DUPLICATE_NAME`, `NOT_FOUND`). Crypto decryption returns `null` on failure.
- **Styling:** CSS Modules (`.module.css`) per component; shared button/input/progress styles in `components/shared.module.css`.
- **State machines:** Use union types for phase/state (e.g., `"idle" | "processing" | "done" | "error"`), not boolean flags.
- **Refs for callbacks:** In VaultPage, callbacks passed to child components are stored in refs to avoid stale closures in `useCallback` dependencies.
