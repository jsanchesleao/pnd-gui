export const fsaSupported =
  typeof window !== "undefined" && "showOpenFilePicker" in window;
