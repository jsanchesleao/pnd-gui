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

PWA / GitHub Pages build:
```bash
GITHUB_PAGES=true npm run build   # Sets base="/pnd-gui/" for GitHub Pages deployment
npm run preview                   # Serve dist/ locally to verify the built PWA
```

Run a single test file:
```bash
npx vitest run src/utils/crypto.test.ts
```

There is no separate linter — `tsc` (run as part of `npm run build`) enforces strict TypeScript including `noUnusedLocals` and `noUnusedParameters`.

## Architecture

**pnd-gui** is a Tauri 2 desktop app (React + TypeScript frontend, minimal Rust backend) for password-based file encryption with three features: Encrypt/Decrypt, Preview, and Vault.

### Core utilities (`src/utils/`)

- **`crypto.ts`** — All cryptography. PBKDF2 (100k iterations, SHA-256) for key derivation, AES-256-GCM for encrypt/decrypt. Exposes stream-based APIs (`createEncryptedStream`, `createDecryptedStream`), a high-level helper `decryptFileToBytes(file, password, onProgress?)` that collects a decrypted stream into a `Uint8Array`, and byte-level APIs (`encryptBytesWithKey`, `decryptBytesWithKey`) for vault use.
- **`frames.ts`** — Stream chunking protocol. Encryption splits data into fixed 1 MB frames; each frame gets its own IV and is prefixed with a 4-byte big-endian size header. Decryption reassembles via matching transforms.
- **`vault.ts`** — Vault state machine. A vault is a folder containing `index.lock` (an AES-encrypted JSON index mapping file UUIDs to block keys and metadata) plus UUID-named encrypted block files (256 MB max each). Blobs may optionally live in a subfolder named by `VaultIndex.blobsDir`; `VaultState.blobsDirHandle` always points to the correct location. `VaultState` is kept in memory; call `saveVault()` to persist.
- **`mediaTypes.ts`** — Maps extensions to `FileCategory` (`image | video | audio | document | archive | code | other`) used throughout the UI.
- **`videoThumbnail.ts`** — Seeks a video blob to 2 s, captures a canvas frame, exports as WebP.
- **`recentVaults.ts`** — Persists recently opened vaults in IndexedDB (`pnd-recent-vaults`). Each `RecentVaultEntry` stores the directory handle, a last-opened timestamp, a favorite flag, an optional user-defined `alias` (display name only — never written to vault files), and an optional `type` (`"regular" | "private"`). Non-favorite regular entries are capped at 5; favorites and private entries are unlimited. Private vaults are registered once via `createPrivateVaultEntry` and deleted via `deletePrivateVaultEntry`; they are listed separately at the bottom of `VaultRecentList`.
- **`platform.ts`** — Runtime compatibility detection. Exports `fsaSupported` (boolean, evaluated once at module load) and two file-picker helpers: `pickFile()` (single file) and `pickFiles()` (multi-select). Both use `showOpenFilePicker` when `fsaSupported` is true, otherwise fall back to a programmatic `<input type="file">` click.
- **`download.ts`** — Browser-download helpers. `downloadBlob(blob, filename)` creates an object URL, clicks an `<a download>` element, and revokes the URL. `collectAndDownload(stream, filename, signal)` collects a `ReadableStream` into a `Blob` then calls `downloadBlob`; used by `GenericPage` for the compat-mode save path.

### Component structure (`src/components/`)

Each component lives in its own folder. Complex components are further split into sibling files (see **Folder conventions** below).

- **`GenericPage/`** — Encrypt/Decrypt tab. Auto-detects mode from the file extension (`.lock` → decrypt, otherwise encrypt). Streams through `createEncryptedStream`/`createDecryptedStream` with progress tracking and mid-operation cancellation via `AbortController`.
- **`PreviewPage/`** — Detects file type (strips `.lock` suffix) then delegates to `ImageViewerPage`, `VideoPlayerPage`, or `GalleryPage` (ZIP decompressed with fflate).
- **`ImageViewerPage/`** — Decrypts a single image file to memory and displays it.
- **`VideoPlayerPage/`** — Same pattern as ImageViewerPage but renders a `<video>` element.
- **`GalleryPage/`** — Decrypts a ZIP archive and shows a keyboard-navigable image carousel.
- **`DecryptingProgress/`** and **`DecryptError/`** — Shared UI components used by all three preview pages. `DecryptingProgress` takes `{ filename, progress: number }` and renders a progress bar; `DecryptError` takes `{ message, onTryAgain, onChangeFile }`.
- **`VaultPage/`** — Most complex area:
  - `index.tsx` — Lifecycle: idle → unlock → browse. Holds `vaultRef` (stable ref to avoid stale closures), manages the serial thumbnail generation queue. Contains `autoSave()` which calls `saveVault` silently (no phase change) — invoked automatically after add, delete, and paste. Cut/paste clipboard (`string[]`) lives here; selection state (`Set<string>`) lives in `VaultBrowser`.
  - `types.ts` — `Phase` discriminated union for the page state machine.
  - `VaultBrowser.tsx` — Toolbar + two-panel shell (folder tree left, file list right). Owns `selectedUuids` state; clears selection via `useEffect` on `currentPath` changes. Toolbar has Cut, Paste, and Delete buttons (each enabled/disabled based on selection or clipboard state); Delete shows a `confirm` dialog with the item count before calling `onDeleteSelected`.
  - `VaultRecentList/` — Shown in the idle phase. Displays recently opened vaults from IndexedDB with favorite toggle, inline rename (sets `alias`), and remove actions. Private vault entries render with a lock badge and trigger `PrivateVaultDeleteDialog` instead of a plain remove.
  - `PrivateVaultDeleteDialog.tsx` — Password-verified deletion dialog for private vaults. On confirm, removes the OPFS directory (`opfsRoot.removeEntry`) and then the IndexedDB entry — this is a destructive, irrecoverable operation.
  - `VaultFolderTree/` — Virtual folder hierarchy derived from `entry.path` fields in the vault index.
  - `VaultFileList/` — File list/grid with sort (name/type/size/date × asc/desc) and list/grid view modes. Contains sub-components `FileIcon`, `VaultThumbnail`, `VaultFileItem`, `VaultGridItem`. Items support single-click selection (toggled via `onSelect`); action buttons (Preview, Save, Rename) stop click propagation to avoid accidentally toggling selection. Move and Delete are toolbar-only operations.
  - `VaultPreviewPanel/` — Full-screen overlay that decrypts a vault entry on demand.

### Data flow summary

**Single file encrypt/decrypt:** File → `createFixedSizeFramesStream` → `createFrameMapperStream` (PBKDF2 + AES-GCM per frame) → `createVariableSizeFrameJoinStream` → write via File System Access API.

**Vault add file:** Bytes → split into 256 MB blocks → each block encrypted with a fresh random AES key → UUID-named file written to vault folder → block UUIDs + base64 keys recorded in in-memory index → `index.lock` auto-saved immediately after the operation completes.

**Vault save:** `saveVault()` encrypts the in-memory index JSON and writes `index.lock`. Called automatically (`autoSave()`) after add, delete, and paste. Rename and thumbnail generation set `vault.modified = true` but require the user to click the Save button.

### Patterns to follow

- **Folder conventions:** Each component folder may contain sibling files named `ComponentName.types.ts` (discriminated unions, interfaces), `ComponentName.constants.ts` (lookup tables, option arrays), `ComponentName.helpers.ts` (pure utility functions), and individual sub-component files (`SubComponent.tsx`). The `index.tsx` imports from these siblings and contains only the component body. CSS Modules are co-located as `ComponentName.module.css`. Page-level state branches that render non-trivial UI are also extracted to named sub-components (e.g. `GalleryCarousel.tsx`, `VaultUnlockForm.tsx`) rather than returned inline from `if (state.type === "...")` blocks; only truly minimal branches (a single button, a one-line message) stay inline.
- **Error handling:** Vault operations throw `VaultError` with typed codes (`WRONG_PASSWORD`, `INVALID_FORMAT`, `DUPLICATE_NAME`, `NOT_FOUND`). Crypto decryption returns `null` on failure.
- **Styling:** CSS Modules (`.module.css`) per component; shared button/input/progress styles in `components/shared.module.css`. Icons in `VaultPage` use `lucide-react` (MIT, tree-shaken); import individual named exports (e.g. `import { Eye, Download } from "lucide-react"`).
- **Compatibility mode:** All File System Access API calls must be guarded by `fsaSupported` from `src/utils/platform.ts`. Use `pickFile()` / `pickFiles()` instead of calling `showOpenFilePicker` directly. For save operations when `!fsaSupported`, use `downloadBlob` from `src/utils/download.ts` instead of `showSaveFilePicker`. Regular (FSAA-backed) vault UI is hidden entirely when `!fsaSupported`; only private (OPFS-backed) vaults are available.
- **State machines:** Use union types for phase/state (e.g., `"idle" | "processing" | "done" | "error"`), not boolean flags.
- **Refs for callbacks:** In VaultPage, callbacks passed to child components are stored in refs to avoid stale closures in `useCallback` dependencies.
- **Shared CSS classes:** `components/shared.module.css` defines `.container`, `.controls`, `.button-group`, `.progress`, and `.text`. Color variants on `.text` use the `data-text-type="success" | "failure"` attribute rather than separate classes.
- **Tech debt tracking:** Known refactoring targets are documented in `docs/tech_debts.md`.

### PWA and deployment

The app is also deployable as a PWA (targeting iOS Safari). The strategy and full feature roadmap are in `docs/ios_pwa_port_plan.md`. Steps 1–5 of that plan are complete: PWA scaffolding, `platform.ts` detection, GenericPage compat mode, Preview tab compat mode, and Vault page compat mode.

- **`vite-plugin-pwa`** generates `sw.js`, `workbox-*.js`, and `manifest.webmanifest` into `dist/` at build time. The manifest is configured inline in `vite.config.ts` (not as a separate file).
- **Base path:** `vite.config.ts` reads `GITHUB_PAGES=true` to switch `base` (and the manifest's `start_url` and icon paths) from `/` to `/pnd-gui/`. Local dev and Tauri builds are unaffected.
- **Icons:** `public/icons/` holds the four PWA icon sizes. They are derived from `src-tauri/icons/icon.png` (512×512 RGBA source). Regenerate them by running the inline Node.js resize script used during initial setup — it decodes the PNG, does bilinear downscaling, and re-encodes without external dependencies.
- **CI:** `.github/workflows/deploy.yml` builds with `GITHUB_PAGES=true` and deploys `dist/` to GitHub Pages via `actions/deploy-pages` on every push to `master`. Requires **Settings → Pages → Source → GitHub Actions** enabled in the repo once.
