import type { PreviewState } from "./VaultPreviewPanel.types";
import classes from "./VaultPreviewPanel.module.css";
import { VaultTextEditor } from "./VaultTextEditor";

export type { PreviewState } from "./VaultPreviewPanel.types";
export { buildPreviewState } from "./VaultPreviewPanel.helpers";

interface Props {
  preview: PreviewState;
  onClose: () => void;
  onSaveText?: (uuid: string, text: string) => Promise<void>;
}

export const VaultPreviewPanel: React.FC<Props> = ({ preview, onClose, onSaveText }) => {
  return (
    <div className={classes["preview-panel"]} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      {preview.type === "loading" && (
        <p className={classes["preview-loading"]}>Decrypting…</p>
      )}

      {preview.type === "image" && (
        <>
          <img src={preview.objectUrl} alt={preview.name} />
          <button onClick={onClose}>Close</button>
        </>
      )}

      {preview.type === "video" && (
        <>
          <video
            src={preview.objectUrl}
            controls
            autoPlay
            style={{ maxWidth: "90%", maxHeight: "80vh" }}
          />
          <button onClick={onClose}>Close</button>
        </>
      )}

      {preview.type === "text" && (
        <VaultTextEditor
          key={preview.uuid}
          uuid={preview.uuid}
          name={preview.name}
          text={preview.text}
          onClose={onClose}
          onSave={onSaveText}
        />
      )}

      {preview.type === "unsupported" && (
        <>
          <p style={{ color: "white" }}>
            Preview not supported for "{preview.name}"
          </p>
          <button onClick={onClose}>Close</button>
        </>
      )}
    </div>
  );
};
