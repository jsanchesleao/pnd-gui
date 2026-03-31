import { getMimeType, getMediaType } from "../../../utils/mediaTypes";
import type { PreviewState } from "./VaultPreviewPanel.types";

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
