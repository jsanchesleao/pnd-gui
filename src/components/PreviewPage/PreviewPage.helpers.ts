import type { Viewer } from "./PreviewPage.types";
import { IMAGE_EXTS, VIDEO_EXTS } from "./PreviewPage.constants";

export function detectViewer(filename: string): Viewer | null {
  const base = filename.endsWith(".lock") ? filename.slice(0, -5) : filename;
  const ext = base.split(".").pop()?.toLowerCase() ?? "";
  if (ext === "zip") return "gallery";
  if (VIDEO_EXTS.has(ext)) return "video";
  if (IMAGE_EXTS.has(ext)) return "image";
  return null;
}
