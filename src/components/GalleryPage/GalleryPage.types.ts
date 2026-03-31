export type GalleryImage = { name: string; objectUrl: string };

export type State =
  | { type: "idle" }
  | { type: "password"; file: File }
  | { type: "loading"; file: File; progress: number }
  | { type: "viewing"; file: File; images: GalleryImage[]; index: number }
  | { type: "error"; file: File; message: string };
