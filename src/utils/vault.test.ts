import { describe, expect, it } from "vitest";
import { encryptBytes } from "./crypto";
import {
  VaultError,
  addFileToVault,
  buildFolderTree,
  createEmptyVault,
  decryptVaultFile,
  exportVaultFile,
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
      getFile: async () =>
        new File([files.get(name) ?? new Uint8Array()], name),
      createWritable: async () => {
        const chunks: BlobPart[] = [];
        return {
          write: async (data: BlobPart) => {
            chunks.push(data);
          },
          close: async () => {
            files.set(
              name,
              new Uint8Array(await new Blob(chunks).arrayBuffer()),
            );
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
    removeEntry: async (name: string) => {
      files.delete(name);
    },
  } as unknown as FileSystemDirectoryHandle;
}

// Expose underlying files map for inspection (returned alongside the handle).
function makeFakeDirHandleWithFiles(): {
  dirHandle: FileSystemDirectoryHandle;
  files: Map<string, Uint8Array>;
} {
  const files = new Map<string, Uint8Array>();

  function makeFakeFileHandle(name: string): FileSystemFileHandle {
    return {
      getFile: async () =>
        new File([files.get(name) ?? new Uint8Array()], name),
      createWritable: async () => {
        const chunks: BlobPart[] = [];
        return {
          write: async (data: BlobPart) => {
            chunks.push(data);
          },
          close: async () => {
            files.set(
              name,
              new Uint8Array(await new Blob(chunks).arrayBuffer()),
            );
          },
        };
      },
    } as unknown as FileSystemFileHandle;
  }

  const dirHandle = {
    getFileHandle: async (name: string, opts?: { create?: boolean }) => {
      if (!opts?.create && !files.has(name)) {
        throw new DOMException(`${name} not found`, "NotFoundError");
      }
      if (!files.has(name)) files.set(name, new Uint8Array());
      return makeFakeFileHandle(name);
    },
    removeEntry: async (name: string) => {
      files.delete(name);
    },
  } as unknown as FileSystemDirectoryHandle;

  return { dirHandle, files };
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
  const encryptedIndexBytes = new Uint8Array(
    await encryptedIndex.arrayBuffer(),
  );
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

function makeEmptyVault(pw = "pw") {
  const dh = makeFakeDirHandle();
  return createEmptyVault(dh, dh, pw);
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
    const dirHandle = await buildVaultDir(
      { version: 1, entries: {} },
      "correct",
    );
    await expect(openVault(dirHandle, "wrong")).rejects.toThrow(VaultError);
    await expect(openVault(dirHandle, "wrong")).rejects.toMatchObject({
      code: "WRONG_PASSWORD",
    });
  });

  it("throws INVALID_FORMAT when index.lock is missing", async () => {
    const dirHandle = makeFakeDirHandle();
    await expect(openVault(dirHandle, "pw")).rejects.toMatchObject({
      code: "INVALID_FORMAT",
    });
  });
});

// ── addFileToVault ─────────────────────────────────────────────────────────

describe("addFileToVault", () => {
  it("adds an entry with one part for a small file", async () => {
    const vault = makeEmptyVault();
    const data = new Uint8Array([1, 2, 3]);
    const uuid = await addFileToVault(vault, data, "test.jpg", "photos");
    const entry = vault.index.entries[uuid];
    expect(entry).toBeDefined();
    expect(entry.name).toBe("test.jpg");
    expect(entry.path).toBe("photos");
    expect(entry.parts).toHaveLength(1);
    expect(entry.parts[0].uuid).toBeTruthy();
    expect(vault.modified).toBe(true);
  });

  it("auto-suffixes duplicate names in the same path", async () => {
    const vault = makeEmptyVault();
    await addFileToVault(vault, new Uint8Array([1]), "img.jpg", "");
    const uuid2 = await addFileToVault(
      vault,
      new Uint8Array([2]),
      "img.jpg",
      "",
    );
    expect(vault.index.entries[uuid2].name).toBe("img (1).jpg");
  });

  it("allows same name in different paths", async () => {
    const vault = makeEmptyVault();
    const uuid1 = await addFileToVault(
      vault,
      new Uint8Array([1]),
      "img.jpg",
      "a",
    );
    const uuid2 = await addFileToVault(
      vault,
      new Uint8Array([2]),
      "img.jpg",
      "b",
    );
    expect(vault.index.entries[uuid1].name).toBe("img.jpg");
    expect(vault.index.entries[uuid2].name).toBe("img.jpg");
  });
});

// ── decryptVaultFile / exportVaultFile ─────────────────────────────────────

describe("decryptVaultFile", () => {
  it("decrypts a single-part file correctly", async () => {
    const vault = makeEmptyVault();
    const data = new Uint8Array([10, 20, 30, 40]);
    const uuid = await addFileToVault(vault, data, "file.bin", "");
    const decrypted = await decryptVaultFile(vault, uuid);
    expect(decrypted).toEqual(data);
  });

  it("reassembles a multi-part file correctly", async () => {
    // Use a tiny PART_SIZE equivalent: we'll manufacture the multi-part condition
    // by directly adding two parts to the index and writing encrypted files.
    // Since PART_SIZE is 256 MB we can't create a real multi-part file in tests,
    // so we test the reassembly logic by building a vault with two entries and
    // treating them conceptually. Instead, rely on the round-trip test below.
    // This test verifies decryptVaultFile with a pre-built two-part entry.
    const { dirHandle, files: _files } = makeFakeDirHandleWithFiles();
    const vault = createEmptyVault(dirHandle, dirHandle, "pw");

    // Add two small files and manually combine their part arrays into one entry
    const data1 = new Uint8Array([1, 2, 3]);
    const data2 = new Uint8Array([4, 5, 6]);
    const uuid1 = await addFileToVault(vault, data1, "part1.bin", "");
    const uuid2 = await addFileToVault(vault, data2, "part2.bin", "");

    // Merge uuid2's parts into uuid1's entry to simulate a two-part file
    const combinedUuid = crypto.randomUUID();
    vault.index.entries[combinedUuid] = {
      name: "combined.bin",
      path: "",
      size: 6,
      parts: [
        ...vault.index.entries[uuid1].parts,
        ...vault.index.entries[uuid2].parts,
      ],
    };
    delete vault.index.entries[uuid1];
    delete vault.index.entries[uuid2];

    const decrypted = await decryptVaultFile(vault, combinedUuid);
    expect(decrypted).toEqual(new Uint8Array([1, 2, 3, 4, 5, 6]));
  });
});

describe("exportVaultFile", () => {
  it("writes decrypted bytes to writable", async () => {
    const vault = makeEmptyVault();
    const data = new Uint8Array([11, 22, 33]);
    const uuid = await addFileToVault(vault, data, "file.bin", "");

    const written: Uint8Array[] = [];
    const fakeWritable = {
      write: async (chunk: Uint8Array) => {
        written.push(chunk);
      },
    } as unknown as FileSystemWritableFileStream;

    await exportVaultFile(vault, uuid, fakeWritable);
    const result = new Uint8Array(
      written.reduce((sum, c) => sum + c.length, 0),
    );
    let off = 0;
    for (const c of written) {
      result.set(c, off);
      off += c.length;
    }
    expect(result).toEqual(data);
  });
});

// ── removeFileFromVault ────────────────────────────────────────────────────

describe("removeFileFromVault", () => {
  it("removes the entry and its part files from the directory", async () => {
    const { dirHandle, files } = makeFakeDirHandleWithFiles();
    const vault = createEmptyVault(dirHandle, dirHandle, "pw");
    const uuid = await addFileToVault(vault, new Uint8Array([1]), "a.txt", "");
    const partUuid = vault.index.entries[uuid].parts[0].uuid;
    expect(files.has(partUuid)).toBe(true);
    await removeFileFromVault(vault, uuid);
    expect(vault.index.entries[uuid]).toBeUndefined();
    expect(files.has(partUuid)).toBe(false);
  });

  it("throws NOT_FOUND for unknown uuid", async () => {
    const vault = makeEmptyVault();
    await expect(removeFileFromVault(vault, "nonexistent")).rejects.toThrow(
      VaultError,
    );
  });
});

// ── renameFileInVault ──────────────────────────────────────────────────────

describe("renameFileInVault", () => {
  it("renames an entry", async () => {
    const vault = makeEmptyVault();
    const uuid = await addFileToVault(
      vault,
      new Uint8Array([1]),
      "old.jpg",
      "",
    );
    renameFileInVault(vault, uuid, "new.jpg");
    expect(vault.index.entries[uuid].name).toBe("new.jpg");
  });

  it("throws DUPLICATE_NAME when sibling has same name", async () => {
    const vault = makeEmptyVault();
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
    const vault = makeEmptyVault();
    const uuid = await addFileToVault(
      vault,
      new Uint8Array([1]),
      "img.jpg",
      "old",
    );
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
        a: { name: "a.jpg", path: "photos/summer", size: 0, parts: [] },
        b: { name: "b.jpg", path: "photos/winter", size: 0, parts: [] },
        c: { name: "c.jpg", path: "", size: 0, parts: [] },
        d: { name: "d.jpg", path: "docs", size: 0, parts: [] },
      },
    };
    const tree = buildFolderTree(index);
    expect(tree.fullPath).toBe("");
    expect(tree.children.map((c) => c.name).sort()).toEqual(["docs", "photos"]);
    const photos = tree.children.find((c) => c.name === "photos")!;
    expect(photos.children.map((c) => c.name).sort()).toEqual([
      "summer",
      "winter",
    ]);
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
        a: { name: "a.jpg", path: "photos", size: 0, parts: [] },
        b: { name: "b.jpg", path: "photos", size: 0, parts: [] },
        c: { name: "c.jpg", path: "docs", size: 0, parts: [] },
        d: { name: "d.jpg", path: "", size: 0, parts: [] },
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
        a: { name: "a", path: "photos/summer", size: 0, parts: [] },
        b: { name: "b", path: "photos/winter", size: 0, parts: [] },
        c: { name: "c", path: "photos/summer/beach", size: 0, parts: [] },
        d: { name: "d", path: "docs", size: 0, parts: [] },
      },
    };
    const subs = getSubfolders(index, "photos");
    expect(subs.sort()).toEqual(["photos/summer", "photos/winter"]);
  });

  it("returns top-level folders from root", () => {
    const index: VaultIndex = {
      version: 1,
      entries: {
        a: { name: "a", path: "photos", size: 0, parts: [] },
        b: { name: "b", path: "docs", size: 0, parts: [] },
        c: { name: "c", path: "", size: 0, parts: [] },
      },
    };
    const subs = getSubfolders(index, "");
    expect(subs.sort()).toEqual(["docs", "photos"]);
  });
});
