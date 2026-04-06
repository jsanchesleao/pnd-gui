import { useState } from "react";
import type { RecentPreviewEntry } from "../../../utils/recentPreviews";
import classes from "./RecentPreviewList.module.css";

interface Props {
  entries: RecentPreviewEntry[];
  onOpen: (entry: RecentPreviewEntry) => void;
  onRemove: (id: number) => void;
  onRename: (id: number, alias: string) => void;
}

function displayName(entry: RecentPreviewEntry): string {
  if (entry.alias) return entry.alias;
  if (entry.type === "local") return entry.handle?.name ?? "Unknown file";
  return entry.url ?? "Unknown URL";
}

export const RecentPreviewList: React.FC<Props> = ({
  entries,
  onOpen,
  onRemove,
  onRename,
}) => {
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingValue, setEditingValue] = useState("");

  function startEditing(entry: RecentPreviewEntry) {
    setEditingId(entry.id);
    setEditingValue(displayName(entry));
  }

  function commitEdit(id: number) {
    onRename(id, editingValue);
    setEditingId(null);
  }

  function cancelEdit() {
    setEditingId(null);
  }

  if (entries.length === 0) return null;

  return (
    <div className={classes.list}>
      <p className={classes.heading}>Recent previews</p>
      {entries.map((entry) => (
        <div key={entry.id} className={classes.item}>
          <span
            className={classes.badge}
            data-type={entry.type}
            title={entry.type === "remote" ? (entry.url ?? "") : "Local file"}
          >
            {entry.type === "local" ? "File" : "URL"}
          </span>
          {editingId === entry.id ? (
            <input
              className={classes["rename-input"]}
              value={editingValue}
              autoFocus
              onChange={(e) => setEditingValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitEdit(entry.id);
                else if (e.key === "Escape") cancelEdit();
              }}
              onBlur={() => commitEdit(entry.id)}
            />
          ) : (
            <button
              className={classes["name-btn"]}
              onClick={() => onOpen(entry)}
              title={displayName(entry)}
            >
              {displayName(entry)}
            </button>
          )}
          <button
            className={classes["rename-btn"]}
            onClick={() => startEditing(entry)}
            title="Rename"
          >
            ✎
          </button>
          <button
            className={classes["remove-btn"]}
            onClick={() => onRemove(entry.id)}
            title="Remove from recent"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
};
