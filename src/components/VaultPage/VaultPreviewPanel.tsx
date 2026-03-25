import { getMimeType, getMediaType } from "../../utils/mediaTypes";
import classes from "./VaultPage.module.css";

export type PreviewState =
  | { type: "loading"; uuid: string }
  | { type: "image"; uuid: string; objectUrl: string; name: string }
  | { type: "video"; uuid: string; objectUrl: string; name: string }
  | { type: "unsupported"; uuid: string; name: string };

interface Props {
  preview: PreviewState;
  onClose: () => void;
}

export const VaultPreviewPanel: React.FC<Props> = ({ preview, onClose }) => {
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

export function buildPreviewState(
  uuid: string,
  name: string,
  bytes: Uint8Array,
): PreviewState {
  const mediaType = getMediaType(name);
  if (mediaType === "other") {
    return { type: "unsupported", uuid, name };
  }
  const mimeType = getMimeType(name);
  const objectUrl = URL.createObjectURL(new Blob([bytes], { type: mimeType }));
  return mediaType === "image"
    ? { type: "image", uuid, objectUrl, name }
    : { type: "video", uuid, objectUrl, name };
}
