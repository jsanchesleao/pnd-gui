import { useState } from "react";
import type { ItemProps } from "./VaultFileList.types";
import { formatSize } from "./VaultFileList.helpers";
import classes from "./VaultFileList.module.css";

export const VaultFileItem: React.FC<ItemProps> = ({
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
