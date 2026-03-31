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

### Write-file pattern repeated three times in `vault.ts`

**Files:** `utils/vault.ts`

The same four-line sequence — `getFileHandle → createWritable → write → close` — appears verbatim in `addFileToVault`, `saveVaultThumbnail`, and `saveVault`.

**Suggestion:** Extract a private `writeFileHandle(dirHandle, name, data)` helper inside `vault.ts`.

---

### Unused salt bytes in `encryptBytesWithKey` / `decryptBytesWithKey`

**Files:** `utils/crypto.ts`

`encryptBytesWithKey` prepends a 16-byte salt to every output blob, but `decryptBytesWithKey` skips it with a hardcoded offset and never reads it — the key is pre-derived so there is nothing to re-derive. The salt is dead bytes in every vault block file and the byte layout is inconsistently different from the stream-based encrypt/decrypt functions in the same file.

**Suggestion:** Remove the salt prefix from `encryptBytesWithKey` and adjust `decryptBytesWithKey` to read `iv` at offset 0. **Breaking format change** — requires a migration path or format version guard before fixing.
