import { useState } from "react";
import type { RecentVaultEntry } from "../../../utils/recentVaults";
import classes from "./VaultRecentList.module.css";

interface Props {
  entries: RecentVaultEntry[];
  onOpen: (entry: RecentVaultEntry) => void;
  onRemove: (id: number) => void;
  onToggleFavorite: (id: number) => void;
  onRename: (id: number, alias: string) => void;
  onDeletePrivate: (entry: RecentVaultEntry) => void;
}

export const VaultRecentList: React.FC<Props> = ({
  entries,
  onOpen,
  onRemove,
  onToggleFavorite,
  onRename,
  onDeletePrivate,
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

  const privateEntries = entries.filter((e) => e.type === "private");
  const regularEntries = entries.filter((e) => e.type !== "private");

  if (privateEntries.length === 0 && regularEntries.length === 0) return null;

  return (
    <div className={classes.list}>
      {privateEntries.length > 0 && (
        <>
          <p className={classes.heading}>Private vaults</p>
          {privateEntries.map((entry) => (
            <div key={entry.id} className={`${classes.item} ${classes["item--private"]}`}>
              <span className={classes["private-badge"]}>Private</span>
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
                className={classes["delete-btn"]}
                onClick={() => onDeletePrivate(entry)}
                title="Delete private vault"
              >
                🗑
              </button>
            </div>
          ))}
        </>
      )}
      {regularEntries.length > 0 && (
        <>
          <p className={classes.heading}>Recent vaults</p>
          {regularEntries.map((entry) => (
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
        </>
      )}
    </div>
  );
};
