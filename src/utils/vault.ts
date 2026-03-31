import {
  decryptBytes,
  decryptBytesWithKey,
  encryptBytes,
  encryptBytesWithKey,
  exportKeyToBase64,
  generateFileKey,
  importKeyFromBase64,
} from "./crypto";

// ── Types ──────────────────────────────────────────────────────────────────

const PART_SIZE = 256 * 1024 * 1024; // 256 MB per encrypted part

export interface VaultIndexPart {
  uuid: string;
  keyBase64: string;
}

export interface VaultIndexEntry {
  name: string;
  path: string; // folder path without leading/trailing slash, e.g. "photos/summer" or "" for root
  size: number; // original plaintext byte count
  parts: VaultIndexPart[];
  thumbnailUuid?: string;      // UUID filename of the encrypted thumbnail blob in the vault folder
  thumbnailKeyBase64?: string; // base64 AES-256 key for decrypting only the thumbnail
}

export interface VaultIndex {
  version: 1;
  blobsDir?: string; // relative subfolder name for blobs; absent = same folder as index.lock
  entries: Record<string, VaultIndexEntry>;
}

export interface VaultState {
  dirHandle: FileSystemDirectoryHandle;
  blobsDirHandle: FileSystemDirectoryHandle;
  masterPassword: string;
  index: VaultIndex;
  modified: boolean;
}

export interface FolderNode {
  name: string;
  fullPath: string;
  children: FolderNode[];
}

export class VaultError extends Error {
  constructor(
    public readonly code:
      | "WRONG_PASSWORD"
      | "INVALID_FORMAT"
      | "DUPLICATE_NAME"
      | "NOT_FOUND",
    message: string,
  ) {
    super(message);
    this.name = "VaultError";
  }
}

// ── Open / Create ──────────────────────────────────────────────────────────

export async function openVault(
  dirHandle: FileSystemDirectoryHandle,
  masterPassword: string,
): Promise<VaultState> {
  let indexBytes: Uint8Array;
  try {
    const fh = await dirHandle.getFileHandle("index.lock");
    const file = await fh.getFile();
    indexBytes = new Uint8Array(await file.arrayBuffer());
  } catch {
    throw new VaultError("INVALID_FORMAT", "Vault is missing index.lock");
  }

  const decrypted = await decryptBytes(new Blob([indexBytes]), masterPassword);
  if (decrypted === null) {
    throw new VaultError("WRONG_PASSWORD", "Wrong master password");
  }

  let index: VaultIndex;
  try {
    index = JSON.parse(new TextDecoder().decode(decrypted));
  } catch {
    throw new VaultError("INVALID_FORMAT", "Vault index is corrupted");
  }

  const blobsDirHandle = index.blobsDir
    ? await dirHandle.getDirectoryHandle(index.blobsDir)
    : dirHandle;
  return { dirHandle, blobsDirHandle, masterPassword, index, modified: false };
}

export function createEmptyVault(
  dirHandle: FileSystemDirectoryHandle,
  blobsDirHandle: FileSystemDirectoryHandle,
  masterPassword: string,
  blobsDir?: string,
): VaultState {
  return {
    dirHandle,
    blobsDirHandle,
    masterPassword,
    index: { version: 1, ...(blobsDir ? { blobsDir } : {}), entries: {} },
    modified: true,
  };
}

// ── Mutations ──────────────────────────────────────────────────────────────

export async function addFileToVault(
  vault: VaultState,
  plainBytes: Uint8Array,
  name: string,
  path: string,
  onProgress?: (pct: number) => void,
): Promise<string> {
  const resolvedName = resolveUniqueName(vault, name, path);
  const entryUuid = crypto.randomUUID();
  const totalBytes = plainBytes.length || 1; // avoid divide-by-zero for empty files
  const fileParts: VaultIndexPart[] = [];
  let bytesProcessed = 0;

  for (let offset = 0; offset < plainBytes.length || offset === 0; offset += PART_SIZE) {
    const chunk = plainBytes.subarray(offset, offset + PART_SIZE);
    const key = await generateFileKey();
    const encryptedBlob = await encryptBytesWithKey(chunk, key);
    const encryptedBytes = new Uint8Array(await encryptedBlob.arrayBuffer());
    const keyBase64 = await exportKeyToBase64(key);

    const partUuid = crypto.randomUUID();
    const fh = await vault.blobsDirHandle.getFileHandle(partUuid, { create: true });
    const writable = await fh.createWritable();
    await writable.write(encryptedBytes);
    await writable.close();

    fileParts.push({ uuid: partUuid, keyBase64 });
    bytesProcessed += chunk.length;
    onProgress?.(Math.min(100, Math.round((bytesProcessed / totalBytes) * 100)));

    if (offset === 0 && plainBytes.length === 0) break; // empty file: one part written, done
  }

  vault.index.entries[entryUuid] = { name: resolvedName, path, size: plainBytes.length, parts: fileParts };
  vault.modified = true;
  return entryUuid;
}

export async function removeFileFromVault(vault: VaultState, uuid: string): Promise<void> {
  const entry = vault.index.entries[uuid];
  if (!entry) {
    throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);
  }
  for (const part of entry.parts) {
    await vault.blobsDirHandle.removeEntry(part.uuid);
  }
  if (entry.thumbnailUuid) {
    try { await vault.blobsDirHandle.removeEntry(entry.thumbnailUuid); } catch { /* already gone */ }
  }
  delete vault.index.entries[uuid];
  vault.modified = true;
}

export function renameFileInVault(
  vault: VaultState,
  uuid: string,
  newName: string,
): void {
  const entry = vault.index.entries[uuid];
  if (!entry) throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);

  const siblings = getSiblingsInPath(vault.index, entry.path, uuid);
  if (siblings.some((e) => e.name === newName)) {
    throw new VaultError(
      "DUPLICATE_NAME",
      `A file named "${newName}" already exists in this folder`,
    );
  }
  entry.name = newName;
  vault.modified = true;
}

export function moveFileInVault(
  vault: VaultState,
  uuid: string,
  newPath: string,
): void {
  const entry = vault.index.entries[uuid];
  if (!entry) throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);
  entry.path = newPath;
  vault.modified = true;
}

// ── Decrypt ────────────────────────────────────────────────────────────────

async function decryptPart(
  blobsDirHandle: FileSystemDirectoryHandle,
  part: VaultIndexPart,
): Promise<Uint8Array | null> {
  const fh = await blobsDirHandle.getFileHandle(part.uuid);
  const file = await fh.getFile();
  const encryptedBytes = new Uint8Array(await file.arrayBuffer());
  const key = await importKeyFromBase64(part.keyBase64);
  return decryptBytesWithKey(new Blob([encryptedBytes]), key);
}

/** Reassembles all parts into a single Uint8Array. Used for preview. */
export async function decryptVaultFile(
  vault: VaultState,
  uuid: string,
): Promise<Uint8Array | null> {
  const entry = vault.index.entries[uuid];
  if (!entry) throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);

  const decryptedParts: Uint8Array[] = [];
  for (const part of entry.parts) {
    const decrypted = await decryptPart(vault.blobsDirHandle, part);
    if (decrypted === null) return null;
    decryptedParts.push(decrypted);
  }

  const totalLength = decryptedParts.reduce((sum, p) => sum + p.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const p of decryptedParts) {
    result.set(p, offset);
    offset += p.length;
  }
  return result;
}

/** Decrypts only the first part of a vault file. Used for thumbnail generation to avoid loading large multi-part files. */
export async function decryptFirstVaultPart(
  vault: VaultState,
  uuid: string,
): Promise<Uint8Array | null> {
  const entry = vault.index.entries[uuid];
  if (!entry || entry.parts.length === 0) return null;
  return decryptPart(vault.blobsDirHandle, entry.parts[0]);
}

/** Encrypts thumbnail bytes with a fresh key and stores them as a small file in the vault folder. */
export async function saveVaultThumbnail(
  vault: VaultState,
  entryUuid: string,
  thumbnailBytes: Uint8Array,
): Promise<void> {
  const entry = vault.index.entries[entryUuid];
  if (!entry) throw new VaultError("NOT_FOUND", `Entry ${entryUuid} not found`);

  // Remove the old thumbnail file if one already exists
  if (entry.thumbnailUuid) {
    try { await vault.blobsDirHandle.removeEntry(entry.thumbnailUuid); } catch { /* already gone */ }
  }

  const key = await generateFileKey();
  const encryptedBlob = await encryptBytesWithKey(thumbnailBytes, key);
  const encryptedBytes = new Uint8Array(await encryptedBlob.arrayBuffer());
  const keyBase64 = await exportKeyToBase64(key);
  const thumbUuid = crypto.randomUUID();

  const fh = await vault.blobsDirHandle.getFileHandle(thumbUuid, { create: true });
  const writable = await fh.createWritable();
  await writable.write(encryptedBytes);
  await writable.close();

  entry.thumbnailUuid = thumbUuid;
  entry.thumbnailKeyBase64 = keyBase64;
  vault.modified = true;
}

/** Reads and decrypts a stored thumbnail. Returns null if none exists or decryption fails. */
export async function getVaultThumbnail(
  vault: VaultState,
  entryUuid: string,
): Promise<Uint8Array | null> {
  const entry = vault.index.entries[entryUuid];
  if (!entry?.thumbnailUuid || !entry.thumbnailKeyBase64) return null;
  try {
    const fh = await vault.blobsDirHandle.getFileHandle(entry.thumbnailUuid);
    const file = await fh.getFile();
    const encryptedBytes = new Uint8Array(await file.arrayBuffer());
    const key = await importKeyFromBase64(entry.thumbnailKeyBase64);
    return decryptBytesWithKey(new Blob([encryptedBytes]), key);
  } catch {
    return null;
  }
}

/** Decrypts each part and writes it directly to writable. Used for export (memory-efficient). */
export async function exportVaultFile(
  vault: VaultState,
  uuid: string,
  writable: FileSystemWritableFileStream,
): Promise<void> {
  const entry = vault.index.entries[uuid];
  if (!entry) throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);

  for (const part of entry.parts) {
    const decrypted = await decryptPart(vault.blobsDirHandle, part);
    if (decrypted === null) throw new VaultError("INVALID_FORMAT", `Failed to decrypt part ${part.uuid}`);
    await writable.write(decrypted);
  }
}

// ── Save ───────────────────────────────────────────────────────────────────

export async function saveVault(vault: VaultState): Promise<void> {
  const indexJson = new TextEncoder().encode(JSON.stringify(vault.index));
  const encryptedIndexBlob = await encryptBytes(indexJson, vault.masterPassword);
  const encryptedIndexBytes = new Uint8Array(await encryptedIndexBlob.arrayBuffer());

  const fh = await vault.dirHandle.getFileHandle("index.lock", { create: true });
  const writable = await fh.createWritable();
  await writable.write(encryptedIndexBytes);
  await writable.close();

  vault.modified = false;
}

// ── Folder tree ────────────────────────────────────────────────────────────

export function buildFolderTree(index: VaultIndex): FolderNode {
  const root: FolderNode = { name: "", fullPath: "", children: [] };

  const allPaths = new Set<string>();
  for (const entry of Object.values(index.entries)) {
    if (entry.path === "") continue;
    const parts = entry.path.split("/");
    for (let i = 1; i <= parts.length; i++) {
      allPaths.add(parts.slice(0, i).join("/"));
    }
  }

  for (const fullPath of [...allPaths].sort()) {
    const parts = fullPath.split("/");
    let node = root;
    for (let depth = 0; depth < parts.length; depth++) {
      const segment = parts[depth];
      const childPath = parts.slice(0, depth + 1).join("/");
      let child = node.children.find((c) => c.fullPath === childPath);
      if (!child) {
        child = { name: segment, fullPath: childPath, children: [] };
        node.children.push(child);
      }
      node = child;
    }
  }

  return root;
}

export function getEntriesInPath(
  index: VaultIndex,
  path: string,
): Array<{ uuid: string; entry: VaultIndexEntry }> {
  return Object.entries(index.entries)
    .filter(([, e]) => e.path === path)
    .map(([uuid, entry]) => ({ uuid, entry }));
}

export function getSubfolders(index: VaultIndex, path: string): string[] {
  const prefix = path === "" ? "" : path + "/";
  const seen = new Set<string>();
  for (const entry of Object.values(index.entries)) {
    if (!entry.path.startsWith(prefix) || entry.path === path) continue;
    const rest = entry.path.slice(prefix.length);
    const nextSegment = prefix + rest.split("/")[0];
    seen.add(nextSegment);
  }
  return [...seen].sort();
}

// ── Internal helpers ───────────────────────────────────────────────────────

function getSiblingsInPath(
  index: VaultIndex,
  path: string,
  excludeUuid: string,
): VaultIndexEntry[] {
  return Object.entries(index.entries)
    .filter(([uuid, e]) => e.path === path && uuid !== excludeUuid)
    .map(([, e]) => e);
}

function resolveUniqueName(vault: VaultState, name: string, path: string): string {
  const siblings = getSiblingsInPath(vault.index, path, "");
  const siblingNames = new Set(siblings.map((e) => e.name));
  if (!siblingNames.has(name)) return name;

  const dot = name.lastIndexOf(".");
  const base = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : "";

  let counter = 1;
  let candidate = `${base} (${counter})${ext}`;
  while (siblingNames.has(candidate)) {
    counter++;
    candidate = `${base} (${counter})${ext}`;
  }
  return candidate;
}
