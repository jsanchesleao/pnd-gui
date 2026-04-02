import { useState } from "react";
import { Download, Eye, Pencil } from "lucide-react";
import type { ItemProps } from "./VaultFileList.types";
import { formatSize } from "./VaultFileList.helpers";
import classes from "./VaultFileList.module.css";

export const VaultFileItem: React.FC<ItemProps> = ({
  uuid,
  entry,
  onPreview,
  onExport,
  onRename,
  isSelected,
  onSelect,
}) => {
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(entry.name);
  const [renameError, setRenameError] = useState<string | null>(null);

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

  return (
    <div className={classes["file-item"]} data-selected={isSelected} onClick={() => onSelect(uuid)}>
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
          <div className={classes["file-item-actions"]} onClick={(e) => e.stopPropagation()}>
            <button onClick={() => onPreview(uuid)} title="Preview"><Eye size={14} /></button>
            <button onClick={() => onExport(uuid)} title="Save to disk"><Download size={14} /></button>
            <button
              onClick={() => { setRenameValue(entry.name); setRenaming(true); }}
              title="Rename"
            >
              <Pencil size={14} />
            </button>
          </div>
        </>
      )}
    </div>
  );
};
