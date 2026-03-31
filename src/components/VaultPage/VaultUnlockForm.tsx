import shared from "../shared.module.css";

interface Props {
  operation: "open" | "create";
  password: string;
  subfolderName: string;
  error?: string;
  onPasswordChange: (value: string) => void;
  onSubfolderNameChange: (value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export const VaultUnlockForm: React.FC<Props> = ({
  operation,
  password,
  subfolderName,
  error,
  onPasswordChange,
  onSubfolderNameChange,
  onSubmit,
  onCancel,
}) => (
  <div className={shared.container}>
    <p>
      {operation === "open"
        ? "Unlock vault"
        : "Set master password for new vault"}
    </p>
    <div className={shared.controls}>
      <input
        type="password"
        placeholder="Master password"
        value={password}
        onChange={(e) => onPasswordChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onSubmit()}
        autoFocus
      />
      {operation === "create" && (
        <input
          type="text"
          placeholder="Blob subfolder (optional, e.g. blobs)"
          value={subfolderName}
          onChange={(e) => onSubfolderNameChange(e.target.value)}
        />
      )}
      {error && (
        <p className={shared.text} data-text-type="failure">
          {error}
        </p>
      )}
      <div className={shared["button-group"]}>
        <button onClick={onSubmit} disabled={!password}>
          {operation === "open" ? "Unlock" : "Create"}
        </button>
        <button onClick={onCancel}>Cancel</button>
      </div>
    </div>
  </div>
);
