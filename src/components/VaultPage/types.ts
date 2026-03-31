export type Phase =
  | { phase: "idle" }
  | {
      phase: "unlocking";
      operation: "open" | "create";
      handle: FileSystemDirectoryHandle;
      error?: string;
      vaultName?: string;
    }
  | { phase: "saving" }
  | { phase: "browsing"; currentPath: string };
