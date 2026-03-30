import { useEffect, useState } from "react";
import type { VaultIndexEntry } from "../../utils/vault";
import { getFileCategory, type FileCategory } from "../../utils/mediaTypes";
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
type ViewMode = "list" | "grid";

interface FileEntry {
  uuid: string;
  entry: VaultIndexEntry;
}

interface Props {
  entries: FileEntry[];
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onDelete: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  onMove: (uuid: string, newPath: string) => void;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  thumbnailGenerating: Set<string>;
  onEnqueueThumbnail: (uuid: string) => void;
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

// ── File icon (for non-image files in grid view) ─────────────────────────

const CATEGORY_LABELS: Record<FileCategory, string> = {
  image: "IMG",
  video: "VID",
  audio: "AUD",
  document: "DOC",
  archive: "ZIP",
  code: "CODE",
  other: "FILE",
};

const CATEGORY_COLORS: Record<FileCategory, string> = {
  image: "oklch(68% 0.15 30deg)",
  video: "oklch(52% 0.20 270deg)",
  audio: "oklch(60% 0.20 330deg)",
  document: "oklch(55% 0.15 240deg)",
  archive: "oklch(55% 0.16 145deg)",
  code: "oklch(60% 0.18 75deg)",
  other: "oklch(50% 0.05 270deg)",
};

const FileIcon: React.FC<{ category: FileCategory; generating?: boolean }> = ({ category, generating }) => (
  <div
    className={`${classes["file-icon"]}${generating ? ` ${classes["file-icon-generating"]}` : ""}`}
    style={{ backgroundColor: CATEGORY_COLORS[category] }}
  >
    {CATEGORY_LABELS[category]}
  </div>
);

// ── Thumbnail (image/video preview or file icon) ─────────────────────────

const VaultThumbnail: React.FC<{
  uuid: string;
  filename: string;
  isGenerating: boolean;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  onEnqueueThumbnail: (uuid: string) => void;
}> = ({ uuid, filename, isGenerating, onGetThumbnail, onEnqueueThumbnail }) => {
  const category = getFileCategory(filename);
  const [imgUrl, setImgUrl] = useState<string | null | "loading">("loading");

  useEffect(() => {
    if (category !== "image" && category !== "video") return;
    let active = true;
    setImgUrl("loading");
    onGetThumbnail(uuid).then((url) => {
      if (!active) return;
      if (url) {
        setImgUrl(url);
      } else {
        setImgUrl(null);
        // Request generation only when idle — the effect will re-run when
        // isGenerating transitions back to false after the thumbnail is saved.
        if (category === "video" && !isGenerating) {
          onEnqueueThumbnail(uuid);
        }
      }
    });
    return () => { active = false; };
  }, [uuid, category, onGetThumbnail, isGenerating, onEnqueueThumbnail]);

  // Non-media files always show a static badge
  if (category !== "image" && category !== "video") {
    return <FileIcon category={category} />;
  }
  // While fetching / decrypting, show a gray pulsing placeholder
  if (imgUrl === "loading") {
    return <div className={classes["file-icon-placeholder"]} />;
  }
  // Thumbnail available — show it
  if (imgUrl) {
    return <img className={classes["file-grid-thumb-img"]} src={imgUrl} alt={filename} />;
  }
  // No thumbnail yet — show the category badge (pulse while generating)
  return <FileIcon category={category} generating={isGenerating} />;
};

// ── Grid item ────────────────────────────────────────────────────────────

interface GridItemProps {
  uuid: string;
  entry: VaultIndexEntry;
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onDelete: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  onMove: (uuid: string, newPath: string) => void;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  isGenerating: boolean;
  onEnqueueThumbnail: (uuid: string) => void;
}

const VaultGridItem: React.FC<GridItemProps> = ({
  uuid,
  entry,
  onPreview,
  onExport,
  onDelete,
  onRename,
  onMove,
  onGetThumbnail,
  isGenerating,
  onEnqueueThumbnail,
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
    <div className={classes["file-grid-item"]}>
      <div
        className={classes["file-grid-thumb"]}
        onDoubleClick={() => onPreview(uuid)}
      >
        <VaultThumbnail
          uuid={uuid}
          filename={entry.name}
          isGenerating={isGenerating}
          onGetThumbnail={onGetThumbnail}
          onEnqueueThumbnail={onEnqueueThumbnail}
        />
        <div className={classes["file-grid-actions"]}>
          <button onClick={() => onPreview(uuid)}>Preview</button>
          <button onClick={() => onExport(uuid)}>Save</button>
          <button onClick={() => { setRenameValue(entry.name); setRenaming(true); }}>Rename</button>
          <button onClick={() => { setMoveValue(entry.path); setMoving(true); }}>Move</button>
          <button onClick={() => { if (confirm(`Delete "${entry.name}"?`)) onDelete(uuid); }}>Delete</button>
        </div>
      </div>
      <div className={classes["file-grid-bottom"]}>
        {renaming ? (
          <>
            <input
              className={classes["file-grid-rename-input"]}
              value={renameValue}
              onChange={(e) => { setRenameValue(e.target.value); setRenameError(null); }}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleRenameSubmit();
                if (e.key === "Escape") { setRenaming(false); setRenameError(null); }
              }}
              autoFocus
            />
            <div style={{ display: "flex", gap: "0.2rem" }}>
              <button style={{ fontSize: "0.65rem", padding: "0.1rem 0.3rem" }} onClick={handleRenameSubmit} title="Confirm">✓</button>
              <button style={{ fontSize: "0.65rem", padding: "0.1rem 0.3rem" }} onClick={() => { setRenaming(false); setRenameError(null); }} title="Cancel">✕</button>
            </div>
            {renameError && <span style={{ color: "red", fontSize: "0.65rem" }}>{renameError}</span>}
          </>
        ) : moving ? (
          <>
            <input
              className={classes["file-grid-rename-input"]}
              value={moveValue}
              onChange={(e) => setMoveValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleMoveSubmit();
                if (e.key === "Escape") setMoving(false);
              }}
              placeholder="Folder path"
              autoFocus
            />
            <div style={{ display: "flex", gap: "0.2rem" }}>
              <button style={{ fontSize: "0.65rem", padding: "0.1rem 0.3rem" }} onClick={handleMoveSubmit} title="Confirm">✓</button>
              <button style={{ fontSize: "0.65rem", padding: "0.1rem 0.3rem" }} onClick={() => setMoving(false)} title="Cancel">✕</button>
            </div>
          </>
        ) : (
          <>
            <span className={classes["file-grid-name"]} title={entry.name}>{entry.name}</span>
            <span className={classes["file-grid-size"]}>{formatSize(entry.size)}</span>
          </>
        )}
      </div>
    </div>
  );
};

// ── List item (unchanged from original) ─────────────────────────────────

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

// ── Main component ───────────────────────────────────────────────────────

export const VaultFileList: React.FC<Props> = ({
  entries,
  onPreview,
  onExport,
  onDelete,
  onRename,
  onMove,
  onGetThumbnail,
  thumbnailGenerating,
  onEnqueueThumbnail,
}) => {
  const [sortBy, setSortBy] = useState<SortMode>("name");
  const [viewMode, setViewMode] = useState<ViewMode>("list");

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
        <div className={classes["view-toggle"]}>
          <button
            data-active={viewMode === "list"}
            onClick={() => setViewMode("list")}
            title="List view"
          >
            List
          </button>
          <button
            data-active={viewMode === "grid"}
            onClick={() => setViewMode("grid")}
            title="Grid view"
          >
            Grid
          </button>
        </div>
      </div>
      {viewMode === "list" ? (
        sortedEntries.map(({ uuid, entry }) => (
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
        ))
      ) : (
        <div className={classes["file-grid"]}>
          {sortedEntries.map(({ uuid, entry }) => (
            <VaultGridItem
              key={uuid}
              uuid={uuid}
              entry={entry}
              onPreview={onPreview}
              onExport={onExport}
              onDelete={onDelete}
              onRename={onRename}
              onMove={onMove}
              onGetThumbnail={onGetThumbnail}
              isGenerating={thumbnailGenerating.has(uuid)}
              onEnqueueThumbnail={onEnqueueThumbnail}
            />
          ))}
        </div>
      )}
    </div>
  );
};
