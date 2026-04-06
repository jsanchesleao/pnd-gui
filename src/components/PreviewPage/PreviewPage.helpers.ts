import { invoke } from "@tauri-apps/api/core";
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

export function isValidHttpUrl(value: string): boolean {
  try {
    const u = new URL(value);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}

/**
 * Converts a Google Drive share/view URL to a direct download URL.
 * Returns null if the URL is not a recognised Google Drive link.
 *
 * Supported input patterns:
 *   https://drive.google.com/file/d/<ID>/view?usp=sharing
 *   https://drive.google.com/open?id=<ID>
 *   https://drive.google.com/uc?id=<ID>[&export=download]
 */
function toGoogleDriveDownloadUrl(url: string): string | null {
  try {
    const u = new URL(url);
    if (u.hostname !== "drive.google.com") return null;

    // /file/d/<ID>/...
    const fileMatch = u.pathname.match(/\/file\/d\/([^/]+)/);
    if (fileMatch) {
      return `https://drive.usercontent.google.com/download?id=${fileMatch[1]}&export=download&authuser=0&confirm=t`;
    }

    // ?id=<ID>  (used by /open and /uc)
    const id = u.searchParams.get("id");
    if (id) {
      return `https://drive.usercontent.google.com/download?id=${id}&export=download&authuser=0&confirm=t`;
    }
  } catch {
    // fall through
  }
  return null;
}

function resolveDownloadUrl(url: string): string {
  return toGoogleDriveDownloadUrl(url) ?? url;
}

function extractFilenameFromUrl(url: string): string {
  try {
    const pathname = new URL(url).pathname;
    const segments = pathname.split("/").filter(Boolean);
    const last = segments[segments.length - 1];
    if (last) return decodeURIComponent(last);
  } catch {
    // fall through
  }
  return "download.lock";
}

export async function fetchFileFromUrl(url: string): Promise<File> {
  const downloadUrl = resolveDownloadUrl(url);
  const result = await invoke<{ data: string; filename: string | null }>(
    "download_url",
    { url: downloadUrl },
  );
  const binary = atob(result.data);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  const filename = result.filename ?? extractFilenameFromUrl(url);
  return new File([bytes], filename);
}
