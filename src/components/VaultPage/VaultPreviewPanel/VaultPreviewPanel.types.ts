export type PreviewState =
  | { type: "loading"; uuid: string }
  | { type: "image"; uuid: string; objectUrl: string; name: string }
  | { type: "video"; uuid: string; objectUrl: string; name: string }
  | { type: "text"; uuid: string; name: string; text: string }
  | { type: "unsupported"; uuid: string; name: string };
