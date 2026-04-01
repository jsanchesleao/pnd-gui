import shared from "../shared.module.css";
import type { RecentVaultEntry } from "../../utils/recentVaults";

interface Props {
  entry: RecentVaultEntry;
  password: string;
  error?: string;
  onPasswordChange: (value: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
}

export const PrivateVaultDeleteDialog: React.FC<Props> = ({
  entry,
  password,
  error,
  onPasswordChange,
  onConfirm,
  onCancel,
}) => (
  <div className={shared.container}>
    <p>Delete private vault "{entry.alias ?? entry.name}"?</p>
    <p>This action is permanent and cannot be undone. Enter the master password to confirm.</p>
    <div className={shared.controls}>
      <input
        type="password"
        placeholder="Master password"
        value={password}
        onChange={(e) => onPasswordChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onConfirm()}
        autoFocus
      />
      {error && (
        <p className={shared.text} data-text-type="failure">
          {error}
        </p>
      )}
      <div className={shared["button-group"]}>
        <button onClick={onConfirm} disabled={!password}>
          Delete
        </button>
        <button onClick={onCancel}>Cancel</button>
      </div>
    </div>
  </div>
);
