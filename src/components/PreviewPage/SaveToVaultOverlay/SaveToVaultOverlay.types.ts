import type { RecentVaultEntry } from "../../../utils/recentVaults";
import type { VaultState } from "../../../utils/vault";

export type Phase =
  | { phase: "pick-vault" }
  | { phase: "unlock"; entry: RecentVaultEntry; error?: string }
  | {
      phase: "pick-folder";
      vault: VaultState;
      entry: RecentVaultEntry;
      selectedPath: string;
      filePassword: string;
      fileError?: string;
      importMode: "zip" | "extracted";
    }
  | { phase: "saving" }
  | { phase: "done" };
