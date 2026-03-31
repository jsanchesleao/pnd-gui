export type PreviewState =
  | { type: "loading"; uuid: string }
  | { type: "image"; uuid: string; objectUrl: string; name: string }
  | { type: "video"; uuid: string; objectUrl: string; name: string }
  | { type: "unsupported"; uuid: string; name: string };
