export type Viewer = "video" | "image" | "gallery";

export type State =
  | { type: "idle" }
  | { type: "viewing"; file: File; viewer: Viewer }
  | { type: "unknown"; file: File };
