import { getMimeType, getMediaType, isTextFile } from "../../../utils/mediaTypes";
import type { PreviewState } from "./VaultPreviewPanel.types";

export function buildPreviewState(
  uuid: string,
  name: string,
  bytes: Uint8Array,
): PreviewState {
  if (isTextFile(name)) {
    const text = new TextDecoder().decode(bytes);
    return { type: "text", uuid, name, text };
  }
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
