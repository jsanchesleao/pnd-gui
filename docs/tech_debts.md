# Technical Debts

## What is considered a tech debt

We'll consider any part of the code as a tech debt if it has at least one of the following characteristics

- Large size with multiple things going on
- Components that can be extracted as subcomponents
- Embedded functions that can be extracted as helpers or even at util files
  - Small and component specific functions are fine, only complex functions or general purpose utilities
- Duplicated code
- Anything with an explicit comment asking for refactor

This list is not exhaustive, so anything that can harm the structure and clarity of the code can be documented in this file for future reference

## Known debts

List of known, yet to be fixed, tech debts:

---

### Decryption boilerplate duplicated across three page components

**Files:** `ImageViewerPage/index.tsx`, `VideoPlayerPage/index.tsx`, `GalleryPage/index.tsx`

All three `handleDecrypt`/`handlePlay` functions are near character-for-character copies (~55 lines each): build a `progressStream` TransformStream, pipe through `createDecryptedStream`, collect chunks with a `reader.read()` loop, concatenate into a `combined Uint8Array`, catch errors. Every variable name is identical. Only what happens with `combined` afterwards differs.

**Suggestion:** Extract `decryptFileToBytes(file, password, onProgress)` → `Promise<Uint8Array>` into `src/utils/`, reducing each call site to ~5 lines.

---


### Loading and error UI duplicated across three page components

**Files:** `ImageViewerPage/index.tsx`, `VideoPlayerPage/index.tsx`, `GalleryPage/index.tsx`

The `loading` render block (`<p>Decrypting…</p>` + `<progress>`) and the `error` render block (`Error: {message}` + Try again + Change File buttons) are structurally identical markup in all three components.

**Suggestion:** Extract shared sub-components — e.g. `DecryptingProgress` and `DecryptError` — into a common location.

---

### Write-file pattern repeated three times in `vault.ts`

**Files:** `utils/vault.ts`

The same four-line sequence — `getFileHandle → createWritable → write → close` — appears verbatim in `addFileToVault`, `saveVaultThumbnail`, and `saveVault`.

**Suggestion:** Extract a private `writeFileHandle(dirHandle, name, data)` helper inside `vault.ts`.

---

### Unused salt bytes in `encryptBytesWithKey` / `decryptBytesWithKey`

**Files:** `utils/crypto.ts`

`encryptBytesWithKey` prepends a 16-byte salt to every output blob, but `decryptBytesWithKey` skips it with a hardcoded offset and never reads it — the key is pre-derived so there is nothing to re-derive. The salt is dead bytes in every vault block file and the byte layout is inconsistently different from the stream-based encrypt/decrypt functions in the same file.

**Suggestion:** Remove the salt prefix from `encryptBytesWithKey` and adjust `decryptBytesWithKey` to read `iv` at offset 0. **Breaking format change** — requires a migration path or format version guard before fixing.

---

### Potentially redundant double-decrypt in thumbnail queue

**Files:** `VaultPage/index.tsx` (`processThumbnailQueue`)

The queue processor calls `decryptFirstVaultPart` and passes the result to `generateVideoThumbnail`. If that returns `null` it falls back to `decryptVaultFile` (full file) and tries again. For single-part files these two calls return identical bytes, making the fallback a full redundant decrypt. The `videoThumbnail.ts` utility is documented as designed to work on just the first part.

**Suggestion:** Skip the fallback when `entry.parts.length === 1`, or remove it entirely if the first-part strategy is always sufficient.
