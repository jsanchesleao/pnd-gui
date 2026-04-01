# iOS PWA Port Plan

This document analyses the current pnd-gui codebase and outlines what is needed to ship it as a Progressive Web App (PWA) installable on iOS via Safari "Add to Home Screen". It covers what cannot be supported, what needs adaptation, and what works as-is.

The central strategy is **runtime compatibility mode detection**: the app checks whether the File System Access API is available at startup and, if not, activates a compatibility mode that replaces FSAA-dependent interactions with standard web alternatives and disables features that have no viable replacement.

---

## Platform constraints

iOS Safari has two relevant limitations for this app:

1. **No File System Access API** — `showOpenFilePicker`, `showSaveFilePicker`, `showDirectoryPicker`, `FileSystemDirectoryHandle`, and all related handle methods are absent. There is no roadmap entry for their addition.
2. **No `beforeinstallprompt`** — iOS does not fire the standard PWA install event. The only install path is the user manually tapping "Add to Home Screen" in Safari's share sheet.

Everything else the app relies on — Web Crypto (`crypto.subtle`, AES-GCM, PBKDF2), Streams (`TransformStream`, `ReadableStream`, `pipeTo`, `pipeThrough`), IndexedDB, Origin Private File System (`navigator.storage.getDirectory()`), Blob, `URL.createObjectURL`, Canvas 2D, `<video>`, `<input type="file">`, and `AbortController` — is supported on iOS 14+.

---

## Compatibility mode

### Detection

A single boolean evaluated once at module load time is sufficient:

```ts
const fsaSupported = typeof window !== "undefined" && "showOpenFilePicker" in window;
```

This can be exported from a new `src/utils/platform.ts` file and imported wherever conditional behaviour is needed. No dynamic feature detection libraries are required.

### What compatibility mode changes

When `fsaSupported` is `false`:

| Area | Full mode | Compatibility mode |
|---|---|---|
| Encrypt/Decrypt — file input | `showOpenFilePicker()` | `<input type="file">` |
| Encrypt/Decrypt — file output | `showSaveFilePicker()` + `createWritable()` | Blob collected in memory → `<a download>` trigger |
| Preview — file input | `showOpenFilePicker()` | `<input type="file">` |
| Vault — regular vaults | Available | **Not available** (hidden from UI) |
| Vault — private vaults | Available (OPFS-backed) | Available unchanged (OPFS is supported) |

Nothing else changes. All encryption logic, streaming, in-memory vault state, and preview rendering are identical in both modes.

### Where to apply the flag

- `GenericPage/index.tsx` — branch on `fsaSupported` for file pick and save.
- `PreviewPage` and its sub-pages (`ImageViewerPage`, `VideoPlayerPage`, `GalleryPage`) — branch on `fsaSupported` for file pick only.
- `VaultPage/index.tsx` — hide the "Create Vault" and "Open Vault" buttons when `!fsaSupported`; show only "Create Private Vault".
- `VaultRecentList` — regular vault entries should not appear in compatibility mode (they are FSAA-backed and cannot be reopened). Private vault entries are unaffected.

---

## Features that will not be supported

### Vault drag-and-drop
`handleDropFiles` in `VaultPage/index.tsx` responds to files dropped onto the vault file list. iOS Safari does not support drag-and-drop of files from outside the browser. There is no adaptation path; the drop target can be hidden in compatibility mode.

### Regular vaults in compatibility mode
Regular vaults are backed by a user-chosen directory on disk (accessed via `showDirectoryPicker()`). This API is absent on iOS. Rather than attempting a partial adaptation, regular vaults are simply not available in compatibility mode. The "Create Vault" and "Open Vault" buttons are hidden; the recent vaults list shows only private vault entries. Users on iOS work exclusively with private vaults.

---

## Features that need adaptation

These adaptations apply **only when `fsaSupported` is `false`**. On platforms where the File System Access API is available the existing code paths run unchanged.

### Encrypt / Decrypt tab (`GenericPage`)

**Current behaviour:** `showOpenFilePicker()` selects the source file; `showSaveFilePicker()` + `createWritable()` writes the output directly to a user-chosen location.

**Compatibility mode:**
- Replace `showOpenFilePicker()` with `<input type="file" accept="*/*">`. The selected `File` object is identical to what `handle.getFile()` returns; no changes to the encryption pipeline are needed.
- Replace `showSaveFilePicker()` + `createWritable()` with: collect the output stream into a `Blob`, then create an `<a href={objectURL} download={suggestedName}>` element and programmatically click it. iOS Safari honours the `download` attribute for blobs.
- The `AbortController` cancellation flow needs to cancel the in-memory collection instead of aborting the `WritableStream`. The abort signal can still be passed to an `AbortController` that rejects the collection promise.
- Progress tracking via the `TransformStream` wrapper is unchanged.

**Effort:** Medium. The crypto pipeline is untouched; only the file I/O bookends change.

### Preview tab (`PreviewPage`, `ImageViewerPage`, `VideoPlayerPage`, `GalleryPage`)

**Current behaviour:** A file picker (`showOpenFilePicker`) selects the encrypted file; the rest of the tab decrypts to memory and renders via blob URL or canvas.

**Compatibility mode:**
- Replace `showOpenFilePicker()` with `<input type="file">`.
- Everything downstream (decryption, `URL.createObjectURL`, `<img>`, `<video>`, fflate ZIP decompression, `GalleryCarousel`) works without changes.
- Video codec support varies on iOS. H.264/AAC in MP4 containers is safe. VP8/VP9 in WebM is not supported on iOS Safari. This is a display limitation, not a decryption one; the vault correctly decrypts the bytes — iOS just cannot play them.

**Effort:** Low.

---

## Features that work as-is

The following require zero changes to run on iOS, in either mode:

| Area | Details |
|---|---|
| All of `crypto.ts` | PBKDF2, AES-GCM, key generation, export/import, random values — fully supported by iOS WebKit |
| All of `frames.ts` | Fixed-size frame chunking via `TransformStream` — supported iOS 14+ |
| Vault business logic in `vault.ts` | Key derivation, block splitting, index encryption, move/rename/delete entry logic — all in-memory, no platform dependency |
| `mediaTypes.ts` | Pure lookup table |
| `videoThumbnail.ts` | Canvas 2D + video element seeking — both supported on iOS |
| In-memory vault state machine | `VaultState`, `VaultIndex`, `VaultError` — no platform dependency |
| React component tree | React 19, CSS Modules, Vite — all standard web |
| `fflate` ZIP decompression | Pure JS, works everywhere |
| `URL.createObjectURL` / Blob | Fully supported |
| `AbortController` | Supported since iOS 13.4 |
| IndexedDB | Fully supported; quota is the only concern |
| OPFS (`navigator.storage.getDirectory()`) | Fully supported on iOS Safari; OPFS-derived handles are IDB-serialisable |
| Private vault create / unlock / browse / delete | Uses OPFS + IDB; works on iOS without changes |
| `VaultBrowser` cut/paste clipboard | In-memory `string[]`, no platform dependency |
| `VaultFolderTree` | Derived from in-memory index, no platform dependency |
| `VaultFileList` sort/view modes | Pure UI state |
| `VaultPreviewPanel` | Decrypts to blob URL, displays in overlay — no platform dependency |
| `GalleryCarousel` keyboard nav | Standard keyboard events |
| `DecryptingProgress`, `DecryptError` | Pure UI components |
| `App.tsx` nav (in-memory routing) | No `react-router` dependency, just `useState` |

---

## PWA scaffolding required

None of this currently exists in the repo:

### `manifest.json`
```json
{
  "name": "pnd",
  "short_name": "pnd",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#000000",
  "theme_color": "#000000",
  "icons": [
    { "src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/icons/icon-512.png", "sizes": "512x512", "type": "image/png" },
    { "src": "/icons/icon-512-maskable.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
  ]
}
```
Referenced in `index.html` with `<link rel="manifest" href="/manifest.json">`.

### iOS-specific `<meta>` tags (in `index.html`)
```html
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
<meta name="apple-mobile-web-app-title" content="pnd">
<link rel="apple-touch-icon" href="/icons/apple-touch-icon.png">
```
The `apple-touch-icon` should be 180×180 px.

### Service Worker
A minimal service worker is required for iOS to treat the app as installable. It must at minimum cache the app shell (HTML, JS, CSS bundles) so the app opens when offline. Workbox (via `vite-plugin-pwa`) is the standard approach for Vite projects. The service worker must not attempt to cache vault blob URLs or IDB contents.

### HTTPS
iOS only allows PWA installation from HTTPS origins. Any deployment must be behind TLS.

### Viewport meta tag
Already likely present but must include `viewport-fit=cover` for safe-area insets on notched iPhones:
```html
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
```

---

## iOS PWA storage caveats

- Safari may evict IndexedDB and OPFS data for a PWA that has not been opened in a while, especially under storage pressure. Private vault data is therefore at risk of loss with no recovery path. An in-app notice explaining this to users is recommended.
- The iOS PWA install prompt does not exist programmatically. Users must be told to tap the Share icon → "Add to Home Screen" manually. An in-app banner explaining this is recommended.
- Storage quota for PWAs on iOS is approximately 50 MB by default and can grow via a permission prompt up to ~unlimited, but this is browser-managed. Large private vaults may require the user to approve expanded quota.

---

## Implementation order

1. **PWA scaffolding** — `manifest.json`, iOS meta tags, service worker, HTTPS. Unblocks installation testing with no feature changes. (done)
2. **`src/utils/platform.ts`** — export the `fsaSupported` boolean. Single source of truth for all conditional branches. (done)
3. **Encrypt / Decrypt tab** — branch on `fsaSupported`: keep existing path when `true`, render `<input type="file">` + download trigger when `false`. Crypto pipeline untouched. (done)
4. **Preview tab** — same `fsaSupported` branch for the file input only; everything else unchanged.
5. **Vault page** — hide "Create Vault" and "Open Vault" buttons when `!fsaSupported`; hide regular vault entries in `VaultRecentList`. Private vault UI is already functional.
