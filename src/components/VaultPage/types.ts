import type { RecentVaultEntry } from "../../utils/recentVaults";

export type Phase =
  | { phase: "idle" }
  | {
      phase: "unlocking";
      operation: "open" | "create" | "create-private";
      handle: FileSystemDirectoryHandle;
      error?: string;
      vaultName?: string;
      privateAlias?: string;
      entryId?: number;
    }
  | { phase: "saving" }
  | { phase: "browsing"; currentPath: string }
  | {
      phase: "confirm-delete-private";
      entry: RecentVaultEntry;
      error?: string;
    };
