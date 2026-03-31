import type { RecentVaultEntry } from "../../../utils/recentVaults";
import classes from "./VaultRecentList.module.css";

interface Props {
  entries: RecentVaultEntry[];
  onOpen: (entry: RecentVaultEntry) => void;
  onRemove: (id: number) => void;
  onToggleFavorite: (id: number) => void;
}

export const VaultRecentList: React.FC<Props> = ({
  entries,
  onOpen,
  onRemove,
  onToggleFavorite,
}) => {
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
          <button
            className={classes["name-btn"]}
            onClick={() => onOpen(entry)}
            title={entry.name}
          >
            {entry.name}
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
