import { strFromU8, zipSync } from "fflate";
import {
  decryptBytes,
  decryptBytesWithKey,
  encryptBytes,
  encryptBytesWithKey,
  exportKeyToBase64,
  generateFileKey,
  importKeyFromBase64,
} from "./crypto";
import { type ZipEntry, parseZipDirectory, readZipEntry } from "./zipReader";

export type { ZipEntry };

// ── Types ──────────────────────────────────────────────────────────────────

export interface VaultIndexEntry {
  name: string;
  path: string; // folder path without leading/trailing slash, e.g. "photos/summer" or "" for root
  keyBase64: string;
}

export interface VaultIndex {
  version: 1;
  entries: Record<string, VaultIndexEntry>;
}

export interface VaultState {
  fileHandle: FileSystemFileHandle;
  masterPassword: string;
  index: VaultIndex;
  zipDirectory: Map<string, ZipEntry>; // only metadata, no file data in memory
  pendingFiles: Map<string, Uint8Array>; // uuid → encrypted bytes for newly added files
  deletedUuids: Set<string>;
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
  handle: FileSystemFileHandle,
  masterPassword: string,
): Promise<VaultState> {
  const file = await handle.getFile();

  let zipDirectory: Map<string, ZipEntry>;
  try {
    zipDirectory = await parseZipDirectory(file);
  } catch {
    throw new VaultError("INVALID_FORMAT", "Not a valid vault file");
  }

  const indexEntry = zipDirectory.get("index.lock");
  if (!indexEntry) {
    throw new VaultError("INVALID_FORMAT", "Vault is missing index.lock");
  }

  const indexBytes = await readZipEntry(file, indexEntry);
  const decrypted = await decryptBytes(new Blob([indexBytes]), masterPassword);
  if (decrypted === null) {
    throw new VaultError("WRONG_PASSWORD", "Wrong master password");
  }

  let index: VaultIndex;
  try {
    index = JSON.parse(strFromU8(decrypted));
  } catch {
    throw new VaultError("INVALID_FORMAT", "Vault index is corrupted");
  }

  return {
    fileHandle: handle,
    masterPassword,
    index,
    zipDirectory,
    pendingFiles: new Map(),
    deletedUuids: new Set(),
    modified: false,
  };
}

export function createEmptyVault(
  handle: FileSystemFileHandle,
  masterPassword: string,
): VaultState {
  return {
    fileHandle: handle,
    masterPassword,
    index: { version: 1, entries: {} },
    zipDirectory: new Map(),
    pendingFiles: new Map(),
    deletedUuids: new Set(),
    modified: true,
  };
}

// ── Mutations ──────────────────────────────────────────────────────────────

export async function addFileToVault(
  vault: VaultState,
  plainBytes: Uint8Array,
  name: string,
  path: string,
): Promise<string> {
  const resolvedName = resolveUniqueName(vault, name, path);
  const uuid = crypto.randomUUID();
  const key = await generateFileKey();
  const encryptedBlob = await encryptBytesWithKey(plainBytes, key);
  const encryptedBytes = new Uint8Array(await encryptedBlob.arrayBuffer());
  const keyBase64 = await exportKeyToBase64(key);

  vault.pendingFiles.set(uuid, encryptedBytes);
  vault.index.entries[uuid] = { name: resolvedName, path, keyBase64 };
  vault.modified = true;
  return uuid;
}

export function removeFileFromVault(vault: VaultState, uuid: string): void {
  if (!vault.index.entries[uuid]) {
    throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);
  }
  delete vault.index.entries[uuid];
  vault.deletedUuids.add(uuid);
  vault.pendingFiles.delete(uuid);
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

// ── Decrypt for preview ────────────────────────────────────────────────────

export async function decryptVaultFile(
  vault: VaultState,
  uuid: string,
): Promise<Uint8Array | null> {
  const entry = vault.index.entries[uuid];
  if (!entry) throw new VaultError("NOT_FOUND", `Entry ${uuid} not found`);

  let encryptedBytes: Uint8Array;
  if (vault.pendingFiles.has(uuid)) {
    encryptedBytes = vault.pendingFiles.get(uuid)!;
  } else {
    const zipEntry = vault.zipDirectory.get(uuid);
    if (!zipEntry) throw new VaultError("NOT_FOUND", `ZIP entry for ${uuid} not found`);
    const file = await vault.fileHandle.getFile();
    encryptedBytes = await readZipEntry(file, zipEntry);
  }

  const key = await importKeyFromBase64(entry.keyBase64);
  return decryptBytesWithKey(new Blob([encryptedBytes]), key);
}

// ── Save ───────────────────────────────────────────────────────────────────

export async function saveVault(vault: VaultState): Promise<void> {
  const encoder = new TextEncoder();
  const indexJson = encoder.encode(JSON.stringify(vault.index));
  const encryptedIndexBlob = await encryptBytes(indexJson, vault.masterPassword);
  const encryptedIndexBytes = new Uint8Array(await encryptedIndexBlob.arrayBuffer());

  const zipInput: Record<string, [Uint8Array, { level: 0 | 9 }]> = {
    "index.lock": [encryptedIndexBytes, { level: 9 }],
  };

  const file = await vault.fileHandle.getFile();
  for (const uuid of Object.keys(vault.index.entries)) {
    if (vault.deletedUuids.has(uuid)) continue;

    if (vault.pendingFiles.has(uuid)) {
      zipInput[uuid] = [vault.pendingFiles.get(uuid)!, { level: 0 }];
    } else {
      const zipEntry = vault.zipDirectory.get(uuid);
      if (zipEntry) {
        const bytes = await readZipEntry(file, zipEntry);
        zipInput[uuid] = [bytes, { level: 0 }];
      }
    }
  }

  const zipped = zipSync(zipInput);
  const writable = await vault.fileHandle.createWritable();
  await writable.write(zipped);
  await writable.close();

  // Refresh state after save
  vault.modified = false;
  vault.pendingFiles.clear();
  vault.deletedUuids.clear();
  const freshFile = await vault.fileHandle.getFile();
  vault.zipDirectory = await parseZipDirectory(freshFile);
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
