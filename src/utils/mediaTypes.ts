export const IMAGE_EXTS = new Set([
  "jpg", "jpeg", "png", "gif", "webp", "avif", "bmp", "svg",
]);

export const TEXT_EXTS = new Set(["txt", "md"]);

export const VIDEO_EXTS = new Set(["mp4", "webm", "mkv", "mov", "avi"]);

const AUDIO_EXTS = new Set(["mp3", "wav", "ogg", "flac", "aac", "m4a", "opus"]);

const DOCUMENT_EXTS = new Set([
  "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
  "odt", "ods", "odp", "txt", "rtf", "md",
]);

const ARCHIVE_EXTS = new Set(["zip", "tar", "gz", "bz2", "xz", "rar", "7z", "zst"]);

const CODE_EXTS = new Set([
  "js", "ts", "jsx", "tsx", "py", "rs", "go", "java",
  "c", "cpp", "h", "hpp", "cs", "rb", "php",
  "html", "css", "json", "yaml", "yml", "toml", "sh", "bash",
]);

export type FileCategory = "image" | "video" | "audio" | "document" | "archive" | "code" | "other";

function getExt(filename: string): string {
  const base = filename.endsWith(".lock") ? filename.slice(0, -5) : filename;
  return base.split(".").pop()?.toLowerCase() ?? "";
}

export function getFileCategory(filename: string): FileCategory {
  const ext = getExt(filename);
  if (IMAGE_EXTS.has(ext)) return "image";
  if (VIDEO_EXTS.has(ext)) return "video";
  if (AUDIO_EXTS.has(ext)) return "audio";
  if (DOCUMENT_EXTS.has(ext)) return "document";
  if (ARCHIVE_EXTS.has(ext)) return "archive";
  if (CODE_EXTS.has(ext)) return "code";
  return "other";
}

export function getMediaType(filename: string): "image" | "video" | "other" {
  const ext = getExt(filename);
  if (IMAGE_EXTS.has(ext)) return "image";
  if (VIDEO_EXTS.has(ext)) return "video";
  return "other";
}

export function isTextFile(filename: string): boolean {
  return TEXT_EXTS.has(getExt(filename));
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
