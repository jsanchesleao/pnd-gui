export type State =
  | { type: "idle" }
  | { type: "password"; file: File }
  | { type: "loading"; file: File; progress: number }
  | { type: "viewing"; file: File; textContent: string }
  | { type: "error"; file: File; message: string };

export interface Props {
  initialFile?: File;
  onReset?: () => void;
}
