import type { FileCategory } from "../../../utils/mediaTypes";

export const CATEGORY_LABELS: Record<FileCategory, string> = {
  image: "IMG",
  video: "VID",
  audio: "AUD",
  document: "DOC",
  archive: "ZIP",
  code: "CODE",
  other: "FILE",
};

export const CATEGORY_COLORS: Record<FileCategory, string> = {
  image: "oklch(68% 0.15 30deg)",
  video: "oklch(52% 0.20 270deg)",
  audio: "oklch(60% 0.20 330deg)",
  document: "oklch(55% 0.15 240deg)",
  archive: "oklch(55% 0.16 145deg)",
  code: "oklch(60% 0.18 75deg)",
  other: "oklch(50% 0.05 270deg)",
};
