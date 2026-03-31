import type { FileEntry, SortMode, SortOrder } from "./VaultFileList.types";

export function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
  if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
  return (bytes / 1024).toFixed(1) + " KB";
}

export function getExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function sortEntries(entries: FileEntry[], mode: SortMode, order: SortOrder): FileEntry[] {
  let result: FileEntry[];
  if (mode === "date") {
    result = order === "desc" ? [...entries].reverse() : entries;
  } else {
    result = [...entries].sort((a, b) => {
      if (mode === "name") {
        return a.entry.name.localeCompare(b.entry.name, undefined, { sensitivity: "base" });
      }
      if (mode === "size") {
        return a.entry.size - b.entry.size;
      }
      // type
      const extA = getExtension(a.entry.name);
      const extB = getExtension(b.entry.name);
      if (extA === extB) {
        return a.entry.name.localeCompare(b.entry.name, undefined, { sensitivity: "base" });
      }
      if (extA === "") return 1;
      if (extB === "") return -1;
      return extA.localeCompare(extB);
    });
    if (order === "desc") result.reverse();
  }
  return result;
}
