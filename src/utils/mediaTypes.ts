export const IMAGE_EXTS = new Set([
  "jpg", "jpeg", "png", "gif", "webp", "avif", "bmp", "svg",
]);

export const VIDEO_EXTS = new Set(["mp4", "webm", "mkv", "mov", "avi"]);

function getExt(filename: string): string {
  const base = filename.endsWith(".lock") ? filename.slice(0, -5) : filename;
  return base.split(".").pop()?.toLowerCase() ?? "";
}

export function getMediaType(filename: string): "image" | "video" | "other" {
  const ext = getExt(filename);
  if (IMAGE_EXTS.has(ext)) return "image";
  if (VIDEO_EXTS.has(ext)) return "video";
  return "other";
}

export function getMimeType(filename: string): string {
  const ext = getExt(filename);
  switch (ext) {
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "png":
      return "image/png";
    case "gif":
      return "image/gif";
    case "webp":
      return "image/webp";
    case "avif":
      return "image/avif";
    case "bmp":
      return "image/bmp";
    case "svg":
      return "image/svg+xml";
    case "mp4":
      return "video/mp4";
    case "webm":
      return "video/webm";
    case "mkv":
      return "video/x-matroska";
    case "mov":
      return "video/quicktime";
    case "avi":
      return "video/x-msvideo";
    default:
      return "application/octet-stream";
  }
}
