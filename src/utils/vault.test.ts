import { describe, expect, it } from "vitest";
import { encryptBytes } from "./crypto";
import {
  VaultError,
  addFileToVault,
  buildFolderTree,
  createEmptyVault,
  decryptVaultFile,
  getEntriesInPath,
  getSubfolders,
  moveFileInVault,
  openVault,
  removeFileFromVault,
  renameFileInVault,
  saveVault,
  type VaultIndex,
} from "./vault";

// Fake FileSystemDirectoryHandle backed by an in-memory Map.
function makeFakeDirHandle(): FileSystemDirectoryHandle {
  const files = new Map<string, Uint8Array>();

  function makeFakeFileHandle(name: string): FileSystemFileHandle {
    return {
      getFile: async () => new File([files.get(name) ?? new Uint8Array()], name),
      createWritable: async () => {
        const chunks: BlobPart[] = [];
        return {
          write: async (data: BlobPart) => { chunks.push(data); },
          close: async () => {
            files.set(name, new Uint8Array(await new Blob(chunks).arrayBuffer()));
          },
        };
      },
    } as unknown as FileSystemFileHandle;
  }

  return {
    getFileHandle: async (name: string, opts?: { create?: boolean }) => {
      if (!opts?.create && !files.has(name)) {
        throw new DOMException(`${name} not found`, "NotFoundError");
      }
      if (!files.has(name)) files.set(name, new Uint8Array());
      return makeFakeFileHandle(name);
    },
    removeEntry: async (name: string) => { files.delete(name); },
  } as unknown as FileSystemDirectoryHandle;
}

// Build a vault directory pre-populated with an encrypted index and optional extra files.
async function buildVaultDir(
  index: VaultIndex,
  masterPassword: string,
  extraFiles: Record<string, Uint8Array> = {},
): Promise<FileSystemDirectoryHandle> {
  const dirHandle = makeFakeDirHandle();
  const indexJson = new TextEncoder().encode(JSON.stringify(index));
  const encryptedIndex = await encryptBytes(indexJson, masterPassword);
  const encryptedIndexBytes = new Uint8Array(await encryptedIndex.arrayBuffer());
  const fh = await dirHandle.getFileHandle("index.lock", { create: true });
  const w = await fh.createWritable();
  await w.write(encryptedIndexBytes);
  await w.close();
  for (const [name, bytes] of Object.entries(extraFiles)) {
    const fh2 = await dirHandle.getFileHandle(name, { create: true });
    const w2 = await fh2.createWritable();
    await w2.write(bytes);
    await w2.close();
  }
  return dirHandle;
}

// ── openVault ──────────────────────────────────────────────────────────────

describe("openVault", () => {
  it("opens a valid vault", async () => {
    const index: VaultIndex = { version: 1, entries: {} };
    const dirHandle = await buildVaultDir(index, "secret");
    const vault = await openVault(dirHandle, "secret");
    expect(vault.index.version).toBe(1);
    expect(vault.modified).toBe(false);
  });

  it("throws WRONG_PASSWORD on bad password", async () => {
    const dirHandle = await buildVaultDir({ version: 1, entries: {} }, "correct");
    await expect(openVault(dirHandle, "wrong")).rejects.toThrow(VaultError);
    await expect(openVault(dirHandle, "wrong")).rejects.toMatchObject({ code: "WRONG_PASSWORD" });
  });

  it("throws INVALID_FORMAT when index.lock is missing", async () => {
    const dirHandle = makeFakeDirHandle();
    await expect(openVault(dirHandle, "pw")).rejects.toMatchObject({ code: "INVALID_FORMAT" });
  });
});

// ── addFileToVault ─────────────────────────────────────────────────────────

describe("addFileToVault", () => {
  it("adds an entry to the index and writes to directory", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    const data = new Uint8Array([1, 2, 3]);
    const uuid = await addFileToVault(vault, data, "test.jpg", "photos");
    expect(vault.index.entries[uuid]).toBeDefined();
    expect(vault.index.entries[uuid].name).toBe("test.jpg");
    expect(vault.index.entries[uuid].path).toBe("photos");
    expect(vault.modified).toBe(true);
  });

  it("auto-suffixes duplicate names in the same path", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    await addFileToVault(vault, new Uint8Array([1]), "img.jpg", "");
    const uuid2 = await addFileToVault(vault, new Uint8Array([2]), "img.jpg", "");
    expect(vault.index.entries[uuid2].name).toBe("img (1).jpg");
  });

  it("allows same name in different paths", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    const uuid1 = await addFileToVault(vault, new Uint8Array([1]), "img.jpg", "a");
    const uuid2 = await addFileToVault(vault, new Uint8Array([2]), "img.jpg", "b");
    expect(vault.index.entries[uuid1].name).toBe("img.jpg");
    expect(vault.index.entries[uuid2].name).toBe("img.jpg");
  });
});

// ── decryptVaultFile ───────────────────────────────────────────────────────

describe("decryptVaultFile", () => {
  it("decrypts a file correctly", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    const data = new Uint8Array([10, 20, 30, 40]);
    const uuid = await addFileToVault(vault, data, "file.bin", "");
    const decrypted = await decryptVaultFile(vault, uuid);
    expect(decrypted).toEqual(data);
  });
});

// ── removeFileFromVault ────────────────────────────────────────────────────

describe("removeFileFromVault", () => {
  it("removes an entry", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    const uuid = await addFileToVault(vault, new Uint8Array([1]), "a.txt", "");
    await removeFileFromVault(vault, uuid);
    expect(vault.index.entries[uuid]).toBeUndefined();
  });

  it("throws NOT_FOUND for unknown uuid", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    await expect(removeFileFromVault(vault, "nonexistent")).rejects.toThrow(VaultError);
  });
});

// ── renameFileInVault ──────────────────────────────────────────────────────

describe("renameFileInVault", () => {
  it("renames an entry", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    const uuid = await addFileToVault(vault, new Uint8Array([1]), "old.jpg", "");
    renameFileInVault(vault, uuid, "new.jpg");
    expect(vault.index.entries[uuid].name).toBe("new.jpg");
  });

  it("throws DUPLICATE_NAME when sibling has same name", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    await addFileToVault(vault, new Uint8Array([1]), "a.jpg", "");
    const uuid2 = await addFileToVault(vault, new Uint8Array([2]), "b.jpg", "");
    expect(() => renameFileInVault(vault, uuid2, "a.jpg")).toThrow(VaultError);
    expect(() => renameFileInVault(vault, uuid2, "a.jpg")).toThrow(
      expect.objectContaining({ code: "DUPLICATE_NAME" }),
    );
  });
});

// ── moveFileInVault ────────────────────────────────────────────────────────

describe("moveFileInVault", () => {
  it("moves an entry to a new path", async () => {
    const vault = createEmptyVault(makeFakeDirHandle(), "pw");
    const uuid = await addFileToVault(vault, new Uint8Array([1]), "img.jpg", "old");
    moveFileInVault(vault, uuid, "new/path");
    expect(vault.index.entries[uuid].path).toBe("new/path");
  });
});

// ── save + reopen round-trip ───────────────────────────────────────────────

describe("saveVault + openVault round-trip", () => {
  it("saves and reopens with all entries intact", async () => {
    const dirHandle = await buildVaultDir({ version: 1, entries: {} }, "pw");
    const vault = await openVault(dirHandle, "pw");

    const data1 = new Uint8Array([1, 2, 3]);
    const data2 = new Uint8Array([4, 5, 6]);
    const uuid1 = await addFileToVault(vault, data1, "a.jpg", "pics");
    const uuid2 = await addFileToVault(vault, data2, "b.jpg", "");
    await saveVault(vault);

    expect(vault.modified).toBe(false);

    // Re-open
    const vault2 = await openVault(dirHandle, "pw");
    expect(Object.keys(vault2.index.entries)).toHaveLength(2);
    expect(vault2.index.entries[uuid1].name).toBe("a.jpg");
    expect(vault2.index.entries[uuid2].name).toBe("b.jpg");

    // Decrypt from saved vault
    const decrypted1 = await decryptVaultFile(vault2, uuid1);
    expect(decrypted1).toEqual(data1);
  });

  it("excludes deleted entries on save", async () => {
    const dirHandle = await buildVaultDir({ version: 1, entries: {} }, "pw");
    const vault = await openVault(dirHandle, "pw");

    const uuid1 = await addFileToVault(vault, new Uint8Array([1]), "a.jpg", "");
    await addFileToVault(vault, new Uint8Array([2]), "b.jpg", "");
    await removeFileFromVault(vault, uuid1);
    await saveVault(vault);

    const vault2 = await openVault(dirHandle, "pw");
    expect(Object.keys(vault2.index.entries)).toHaveLength(1);
    expect(vault2.index.entries[uuid1]).toBeUndefined();
  });
});

// ── buildFolderTree ────────────────────────────────────────────────────────

describe("buildFolderTree", () => {
  it("builds a tree from entry paths", () => {
    const index: VaultIndex = {
      version: 1,
      entries: {
        a: { name: "a.jpg", path: "photos/summer", keyBase64: "" },
        b: { name: "b.jpg", path: "photos/winter", keyBase64: "" },
        c: { name: "c.jpg", path: "", keyBase64: "" },
        d: { name: "d.jpg", path: "docs", keyBase64: "" },
      },
    };
    const tree = buildFolderTree(index);
    expect(tree.fullPath).toBe("");
    expect(tree.children.map((c) => c.name).sort()).toEqual(["docs", "photos"]);
    const photos = tree.children.find((c) => c.name === "photos")!;
    expect(photos.children.map((c) => c.name).sort()).toEqual(["summer", "winter"]);
  });

  it("returns empty root for empty index", () => {
    const tree = buildFolderTree({ version: 1, entries: {} });
    expect(tree.children).toHaveLength(0);
  });
});

// ── getEntriesInPath ───────────────────────────────────────────────────────

describe("getEntriesInPath", () => {
  it("returns only entries at the given path", () => {
    const index: VaultIndex = {
      version: 1,
      entries: {
        a: { name: "a.jpg", path: "photos", keyBase64: "" },
        b: { name: "b.jpg", path: "photos", keyBase64: "" },
        c: { name: "c.jpg", path: "docs", keyBase64: "" },
        d: { name: "d.jpg", path: "", keyBase64: "" },
      },
    };
    const result = getEntriesInPath(index, "photos");
    expect(result.map((r) => r.uuid).sort()).toEqual(["a", "b"]);
  });
});

// ── getSubfolders ──────────────────────────────────────────────────────────

describe("getSubfolders", () => {
  it("returns immediate subfolders", () => {
    const index: VaultIndex = {
      version: 1,
      entries: {
        a: { name: "a", path: "photos/summer", keyBase64: "" },
        b: { name: "b", path: "photos/winter", keyBase64: "" },
        c: { name: "c", path: "photos/summer/beach", keyBase64: "" },
        d: { name: "d", path: "docs", keyBase64: "" },
      },
    };
    const subs = getSubfolders(index, "photos");
    expect(subs.sort()).toEqual(["photos/summer", "photos/winter"]);
  });

  it("returns top-level folders from root", () => {
    const index: VaultIndex = {
      version: 1,
      entries: {
        a: { name: "a", path: "photos", keyBase64: "" },
        b: { name: "b", path: "docs", keyBase64: "" },
        c: { name: "c", path: "", keyBase64: "" },
      },
    };
    const subs = getSubfolders(index, "");
    expect(subs.sort()).toEqual(["docs", "photos"]);
  });
});
