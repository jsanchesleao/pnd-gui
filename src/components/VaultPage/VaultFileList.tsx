import { useState } from "react";
import type { VaultIndexEntry } from "../../utils/vault";
import classes from "./VaultPage.module.css";

function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
  if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
  return (bytes / 1024).toFixed(1) + " KB";
}

function getExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

type SortMode = "name" | "type" | "size" | "date";

interface FileEntry {
  uuid: string;
  entry: VaultIndexEntry;
}

interface Props {
  entries: FileEntry[];
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onDelete: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null; // returns error message or null
  onMove: (uuid: string, newPath: string) => void;
}

function sortEntries(entries: FileEntry[], mode: SortMode): FileEntry[] {
  if (mode === "date") return entries;
  return [...entries].sort((a, b) => {
    if (mode === "name") {
      return a.entry.name.localeCompare(b.entry.name, undefined, { sensitivity: "base" });
    }
    if (mode === "size") {
      return b.entry.size - a.entry.size;
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
}

export const VaultFileList: React.FC<Props> = ({
  entries,
  onPreview,
  onExport,
  onDelete,
  onRename,
  onMove,
}) => {
  const [sortBy, setSortBy] = useState<SortMode>("name");

  if (entries.length === 0) {
    return (
      <div className={classes["file-list"]}>
        <p className={classes["file-list-empty"]}>This folder is empty.</p>
      </div>
    );
  }

  const sortedEntries = sortEntries(entries, sortBy);

  return (
    <div className={classes["file-list"]}>
      <div className={classes["file-list-sort-bar"]}>
        <label htmlFor="vault-sort">Sort by:</label>
        <select
          id="vault-sort"
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value as SortMode)}
        >
          <option value="name">Name</option>
          <option value="type">Type</option>
          <option value="size">Size</option>
          <option value="date">Date added</option>
        </select>
      </div>
      {sortedEntries.map(({ uuid, entry }) => (
        <VaultFileItem
          key={uuid}
          uuid={uuid}
          entry={entry}
          onPreview={onPreview}
          onExport={onExport}
          onDelete={onDelete}
          onRename={onRename}
          onMove={onMove}
        />
      ))}
    </div>
  );
};

interface ItemProps {
  uuid: string;
  entry: VaultIndexEntry;
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onDelete: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  onMove: (uuid: string, newPath: string) => void;
}

const VaultFileItem: React.FC<ItemProps> = ({
  uuid,
  entry,
  onPreview,
  onExport,
  onDelete,
  onRename,
  onMove,
}) => {
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(entry.name);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  const [moveValue, setMoveValue] = useState(entry.path);

  function handleRenameSubmit() {
    const name = renameValue.trim();
    if (!name || name === entry.name) {
      setRenaming(false);
      return;
    }
    const err = onRename(uuid, name);
    if (err) {
      setRenameError(err);
    } else {
      setRenaming(false);
      setRenameError(null);
    }
  }

  function handleMoveSubmit() {
    const path = moveValue.trim().replace(/^\/|\/$/g, "");
    onMove(uuid, path);
    setMoving(false);
  }

  return (
    <div className={classes["file-item"]}>
      {renaming ? (
        <>
          <input
            className={classes["file-item-rename-input"]}
            value={renameValue}
            onChange={(e) => { setRenameValue(e.target.value); setRenameError(null); }}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleRenameSubmit();
              if (e.key === "Escape") { setRenaming(false); setRenameError(null); }
            }}
            autoFocus
          />
          <div className={classes["file-item-actions"]}>
            <button onClick={handleRenameSubmit} title="Confirm">✓</button>
            <button onClick={() => { setRenaming(false); setRenameError(null); }} title="Cancel">✕</button>
          </div>
          {renameError && (
            <span style={{ color: "red", fontSize: "0.75rem" }}>{renameError}</span>
          )}
        </>
      ) : moving ? (
        <>
          <input
            className={classes["file-item-rename-input"]}
            value={moveValue}
            onChange={(e) => setMoveValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleMoveSubmit();
              if (e.key === "Escape") setMoving(false);
            }}
            placeholder="Folder path (e.g. photos/summer)"
            autoFocus
          />
          <div className={classes["file-item-actions"]}>
            <button onClick={handleMoveSubmit} title="Confirm">✓</button>
            <button onClick={() => setMoving(false)} title="Cancel">✕</button>
          </div>
        </>
      ) : (
        <>
          <span
            className={classes["file-item-name"]}
            title={entry.name}
            onDoubleClick={() => onPreview(uuid)}
          >
            {entry.name}
          </span>
          <span className={classes["file-item-size"]}>{formatSize(entry.size)}</span>
          <div className={classes["file-item-actions"]}>
            <button onClick={() => onPreview(uuid)} title="Preview">Preview</button>
            <button onClick={() => onExport(uuid)} title="Save to disk">Save</button>
            <button
              onClick={() => { setRenameValue(entry.name); setRenaming(true); }}
              title="Rename"
            >
              Rename
            </button>
            <button
              onClick={() => { setMoveValue(entry.path); setMoving(true); }}
              title="Move"
            >
              Move
            </button>
            <button
              onClick={() => {
                if (confirm(`Delete "${entry.name}"?`)) onDelete(uuid);
              }}
              title="Delete"
            >
              Delete
            </button>
          </div>
        </>
      )}
    </div>
  );
};
