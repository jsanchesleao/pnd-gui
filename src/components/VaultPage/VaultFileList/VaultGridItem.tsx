import { useState } from "react";
import type { GridItemProps } from "./VaultFileList.types";
import { formatSize } from "./VaultFileList.helpers";
import { VaultThumbnail } from "./VaultThumbnail";
import classes from "./VaultFileList.module.css";

export const VaultGridItem: React.FC<GridItemProps> = ({
  uuid,
  entry,
  onPreview,
  onExport,
  onRename,
  onGetThumbnail,
  isGenerating,
  onEnqueueThumbnail,
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
    <div className={classes["file-grid-item"]} data-selected={isSelected} onClick={() => onSelect(uuid)}>
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
        <div className={classes["file-grid-actions"]} onClick={(e) => e.stopPropagation()}>
          <button onClick={() => onPreview(uuid)}>Preview</button>
          <button onClick={() => onExport(uuid)}>Save</button>
          <button onClick={() => { setRenameValue(entry.name); setRenaming(true); }}>Rename</button>
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
