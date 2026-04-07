export type Viewer = "video" | "image" | "gallery" | "text";

export type State =
  | { type: "idle"; error?: string }
  | { type: "fetching-url"; url: string }
  | { type: "viewing"; file: File; viewer: Viewer }
  | { type: "unknown"; file: File };
