import { useEffect, useState } from "react";
import { unzipSync } from "fflate";
import classes from "./SaveToVaultOverlay.module.css";
import type { Phase } from "./SaveToVaultOverlay.types";
import {
  VaultError,
  addFileToVault,
  buildFolderTree,
  openVault,
  saveVault,
} from "../../../utils/vault";
import { decryptFileToBytes } from "../../../utils/crypto";
import {
  getRecentVaults,
  touchRecentVault,
  type RecentVaultEntry,
} from "../../../utils/recentVaults";
import { fsaSupported } from "../../../utils/platform";
import { VaultFolderTree } from "../../VaultPage/VaultFolderTree";

interface Props {
  file: File;
  onClose: () => void;
}

const isEncrypted = (file: File) => file.name.endsWith(".lock");

function isGallery(file: File): boolean {
  const base = file.name.endsWith(".lock") ? file.name.slice(0, -5) : file.name;
  return base.toLowerCase().endsWith(".zip");
}

export const SaveToVaultOverlay: React.FC<Props> = ({ file, onClose }) => {
  const [phase, setPhase] = useState<Phase>({ phase: "pick-vault" });
  const [recentVaults, setRecentVaults] = useState<RecentVaultEntry[]>([]);
  const [password, setPassword] = useState("");

  useEffect(() => {
    getRecentVaults()
      .then(setRecentVaults)
      .catch(() => {});
  }, []);

  async function handleBrowse() {
    if (!fsaSupported) return;
    try {
      const handle = await window.showDirectoryPicker();
      const syntheticEntry: RecentVaultEntry = {
        id: -1,
        name: handle.name,
        handle,
        lastOpened: Date.now(),
        favorite: false,
      };
      setPassword("");
      setPhase({ phase: "unlock", entry: syntheticEntry });
    } catch {
      // user cancelled
    }
  }

  function handlePickVault(entry: RecentVaultEntry) {
    setPassword("");
    setPhase({ phase: "unlock", entry });
  }

  async function handleUnlock(entry: RecentVaultEntry) {
    let dirHandle: FileSystemDirectoryHandle;
    try {
      if (entry.type === "private") {
        const opfsRoot = await navigator.storage.getDirectory();
        dirHandle = await opfsRoot.getDirectoryHandle(entry.name);
      } else {
        dirHandle = entry.handle;
      }
      const vault = await openVault(dirHandle, password);
      setPhase({
        phase: "pick-folder",
        vault,
        entry,
        selectedPath: "",
        filePassword: "",
        importMode: "zip",
      });
    } catch (e) {
      const msg =
        e instanceof VaultError && e.code === "WRONG_PASSWORD"
          ? "Wrong password."
          : e instanceof Error
            ? e.message
            : String(e);
      setPhase({ phase: "unlock", entry, error: msg });
    }
  }

  async function handleSave(
    vault: Parameters<typeof saveVault>[0],
    entry: RecentVaultEntry,
    selectedPath: string,
    filePassword: string,
    importMode: "zip" | "extracted",
  ) {
    setPhase({ phase: "saving" });

    const restorePickFolder = (fileError: string) =>
      setPhase({
        phase: "pick-folder",
        vault,
        entry,
        selectedPath,
        filePassword,
        importMode,
        fileError,
      });

    try {
      let bytes: Uint8Array;
      if (isEncrypted(file)) {
        try {
          bytes = await decryptFileToBytes(file, filePassword);
        } catch {
          restorePickFolder("Wrong file password.");
          return;
        }
      } else {
        bytes = new Uint8Array(await file.arrayBuffer());
      }

      if (importMode === "extracted") {
        const entries = unzipSync(bytes);
        for (const [name, entryBytes] of Object.entries(entries)) {
          // Skip directory entries (zero-length names or trailing slash)
          const basename = name.split("/").pop();
          if (!basename) continue;
          await addFileToVault(vault, entryBytes, basename, selectedPath);
        }
      } else {
        const filename = file.name.replace(/\.lock$/, "");
        await addFileToVault(vault, bytes, filename, selectedPath);
      }

      await saveVault(vault);
      if (entry.id !== -1) {
        await touchRecentVault(entry.id).catch(() => {});
      }
      setPhase({ phase: "done" });
      setTimeout(onClose, 1500);
    } catch (e) {
      restorePickFolder(e instanceof Error ? e.message : String(e));
    }
  }

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <div
      className={classes.overlay}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className={classes.card}>
        {phase.phase === "pick-vault" && (
          <PickVaultPhase
            recentVaults={recentVaults}
            onPick={handlePickVault}
            onBrowse={handleBrowse}
            onCancel={onClose}
          />
        )}

        {phase.phase === "unlock" && (
          <UnlockPhase
            entry={phase.entry}
            password={password}
            error={phase.error}
            onPasswordChange={setPassword}
            onSubmit={() => handleUnlock(phase.entry)}
            onBack={() => setPhase({ phase: "pick-vault" })}
          />
        )}

        {phase.phase === "pick-folder" && (
          <PickFolderPhase
            file={file}
            vault={phase.vault}
            selectedPath={phase.selectedPath}
            filePassword={phase.filePassword}
            fileError={phase.fileError}
            importMode={phase.importMode}
            onSelectedPathChange={(p) => setPhase({ ...phase, selectedPath: p })}
            onFilePasswordChange={(pw) => setPhase({ ...phase, filePassword: pw })}
            onImportModeChange={(m) => setPhase({ ...phase, importMode: m })}
            onSave={(effectivePath) =>
              handleSave(
                phase.vault,
                phase.entry,
                effectivePath,
                phase.filePassword,
                phase.importMode,
              )
            }
            onBack={() => {
              setPassword("");
              setPhase({ phase: "unlock", entry: phase.entry });
            }}
          />
        )}

        {phase.phase === "saving" && <p>Saving to vault…</p>}

        {phase.phase === "done" && <p>Saved!</p>}
      </div>
    </div>
  );
};

// ── Sub-phase components ──────────────────────────────────────────────────

interface PickVaultPhaseProps {
  recentVaults: RecentVaultEntry[];
  onPick: (entry: RecentVaultEntry) => void;
  onBrowse: () => void;
  onCancel: () => void;
}

const PickVaultPhase: React.FC<PickVaultPhaseProps> = ({
  recentVaults,
  onPick,
  onBrowse,
  onCancel,
}) => (
  <>
    <h2>Save to Vault</h2>
    {recentVaults.length > 0 ? (
      <div className={classes["vault-list"]}>
        {recentVaults.map((entry) => (
          <button
            key={entry.id}
            className={classes["vault-item"]}
            onClick={() => onPick(entry)}
          >
            <span className={classes["vault-item-name"]}>
              {entry.alias ?? entry.name}
            </span>
            {entry.type === "private" && (
              <span className={classes["private-badge"]}>private</span>
            )}
          </button>
        ))}
      </div>
    ) : (
      <p className={classes["empty-hint"]}>No recent vaults.</p>
    )}
    <div className={classes.actions}>
      <button className={classes["btn-secondary"]} onClick={onCancel}>
        Cancel
      </button>
      {fsaSupported && (
        <button onClick={onBrowse}>Browse filesystem…</button>
      )}
    </div>
  </>
);

interface UnlockPhaseProps {
  entry: RecentVaultEntry;
  password: string;
  error?: string;
  onPasswordChange: (pw: string) => void;
  onSubmit: () => void;
  onBack: () => void;
}

const UnlockPhase: React.FC<UnlockPhaseProps> = ({
  entry,
  password,
  error,
  onPasswordChange,
  onSubmit,
  onBack,
}) => (
  <>
    <h2>{entry.alias ?? entry.name}</h2>
    <div className={classes.field}>
      <label>Vault password</label>
      <input
        type="password"
        value={password}
        autoFocus
        onChange={(e) => onPasswordChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onSubmit();
        }}
      />
    </div>
    {error && <p className={classes.error}>{error}</p>}
    <div className={classes.actions}>
      <button className={classes["btn-secondary"]} onClick={onBack}>
        Back
      </button>
      <button onClick={onSubmit} disabled={!password}>
        Open
      </button>
    </div>
  </>
);

interface PickFolderPhaseProps {
  file: File;
  vault: Parameters<typeof saveVault>[0];
  selectedPath: string;
  filePassword: string;
  fileError?: string;
  importMode: "zip" | "extracted";
  onSelectedPathChange: (path: string) => void;
  onFilePasswordChange: (pw: string) => void;
  onImportModeChange: (mode: "zip" | "extracted") => void;
  onSave: (effectivePath: string) => void;
  onBack: () => void;
}

const PickFolderPhase: React.FC<PickFolderPhaseProps> = ({
  file,
  vault,
  selectedPath,
  filePassword,
  fileError,
  importMode,
  onSelectedPathChange,
  onFilePasswordChange,
  onImportModeChange,
  onSave,
  onBack,
}) => {
  const [newSubfolder, setNewSubfolder] = useState("");
  useEffect(() => setNewSubfolder(""), [selectedPath]);

  const tree = buildFolderTree(vault.index);
  const encrypted = isEncrypted(file);
  const gallery = isGallery(file);

  const trimmed = newSubfolder.trim().replace(/\//g, "");
  const effectivePath = trimmed
    ? (selectedPath === "" ? trimmed : `${selectedPath}/${trimmed}`)
    : selectedPath;

  return (
    <>
      <h2>Choose folder</h2>

      {gallery && (
        <div className={classes["import-mode"]}>
          <button
            className={classes["import-mode-btn"]}
            data-active={String(importMode === "zip")}
            onClick={() => onImportModeChange("zip")}
          >
            Import as ZIP
          </button>
          <button
            className={classes["import-mode-btn"]}
            data-active={String(importMode === "extracted")}
            onClick={() => onImportModeChange("extracted")}
          >
            Import extracted files
          </button>
        </div>
      )}

      <div className={classes["folder-panel"]}>
        <VaultFolderTree
          tree={tree}
          currentPath={selectedPath}
          onNavigate={onSelectedPathChange}
        />
        <div className={classes["folder-detail"]}>
          {importMode === "extracted" ? (
            <p>
              Each file from the archive will be saved into{" "}
              <strong>{effectivePath === "" ? "(root)" : effectivePath}</strong>
            </p>
          ) : (
            <>
              <p>
                Saving as: <strong>{file.name.replace(/\.lock$/, "")}</strong>
              </p>
              <p>
                Into: <strong>{effectivePath === "" ? "(root)" : effectivePath}</strong>
              </p>
            </>
          )}
        </div>
      </div>

      <div className={classes.field}>
        <label>New subfolder (optional)</label>
        <input
          type="text"
          placeholder="e.g. summer"
          value={newSubfolder}
          onChange={(e) => setNewSubfolder(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !(encrypted && !filePassword)) onSave(effectivePath);
          }}
        />
      </div>

      {encrypted && (
        <div className={classes.field}>
          <label>File password</label>
          <input
            type="password"
            value={filePassword}
            onChange={(e) => onFilePasswordChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onSave(effectivePath);
            }}
          />
        </div>
      )}

      {fileError && <p className={classes.error}>{fileError}</p>}

      <div className={classes.actions}>
        <button className={classes["btn-secondary"]} onClick={onBack}>
          Back
        </button>
        <button onClick={() => onSave(effectivePath)} disabled={encrypted && !filePassword}>
          Save
        </button>
      </div>
    </>
  );
};
