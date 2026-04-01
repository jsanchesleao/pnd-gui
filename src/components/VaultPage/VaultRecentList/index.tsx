import { useState } from "react";
import type { RecentVaultEntry } from "../../../utils/recentVaults";
import classes from "./VaultRecentList.module.css";

interface Props {
  entries: RecentVaultEntry[];
  onOpen: (entry: RecentVaultEntry) => void;
  onRemove: (id: number) => void;
  onToggleFavorite: (id: number) => void;
  onRename: (id: number, alias: string) => void;
}

export const VaultRecentList: React.FC<Props> = ({
  entries,
  onOpen,
  onRemove,
  onToggleFavorite,
  onRename,
}) => {
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingValue, setEditingValue] = useState("");

  function startEditing(entry: RecentVaultEntry) {
    setEditingId(entry.id);
    setEditingValue(entry.alias ?? entry.name);
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
      <p className={classes.heading}>Recent vaults</p>
      {entries.map((entry) => (
        <div key={entry.id} className={classes.item}>
          <button
            className={classes["fav-btn"]}
            onClick={() => onToggleFavorite(entry.id)}
            title={entry.favorite ? "Unfavorite" : "Favorite"}
            data-active={entry.favorite}
          >
            ★
          </button>
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
              title={entry.alias ?? entry.name}
            >
              {entry.alias ?? entry.name}
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
