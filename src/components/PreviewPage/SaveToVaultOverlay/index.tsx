import { useEffect, useState } from "react";
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
  ) {
    setPhase({ phase: "saving" });
    try {
      let bytes: Uint8Array;
      if (isEncrypted(file)) {
        try {
          bytes = await decryptFileToBytes(file, filePassword);
        } catch {
          setPhase({
            phase: "pick-folder",
            vault,
            entry,
            selectedPath,
            filePassword,
            fileError: "Wrong file password.",
          });
          return;
        }
      } else {
        bytes = new Uint8Array(await file.arrayBuffer());
      }

      const filename = file.name.replace(/\.lock$/, "");
      await addFileToVault(vault, bytes, filename, selectedPath);
      await saveVault(vault);
      if (entry.id !== -1) {
        await touchRecentVault(entry.id).catch(() => {});
      }
      setPhase({ phase: "done" });
      setTimeout(onClose, 1500);
    } catch (e) {
      // Re-open pick-folder with a generic error
      setPhase({
        phase: "pick-folder",
        vault,
        entry,
        selectedPath,
        filePassword,
        fileError: e instanceof Error ? e.message : String(e),
      });
    }
  }

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <div className={classes.overlay} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
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
            entry={phase.entry}
            selectedPath={phase.selectedPath}
            filePassword={phase.filePassword}
            fileError={phase.fileError}
            onSelectedPathChange={(p) =>
              setPhase({ ...phase, selectedPath: p })
            }
            onFilePasswordChange={(pw) =>
              setPhase({ ...phase, filePassword: pw })
            }
            onSave={() =>
              handleSave(phase.vault, phase.entry, phase.selectedPath, phase.filePassword)
            }
            onBack={() => {
              setPassword("");
              setPhase({ phase: "unlock", entry: phase.entry });
            }}
          />
        )}

        {phase.phase === "saving" && (
          <p>Saving to vault…</p>
        )}

        {phase.phase === "done" && (
          <p>Saved!</p>
        )}
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
        onKeyDown={(e) => { if (e.key === "Enter") onSubmit(); }}
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
  entry: RecentVaultEntry;
  selectedPath: string;
  filePassword: string;
  fileError?: string;
  onSelectedPathChange: (path: string) => void;
  onFilePasswordChange: (pw: string) => void;
  onSave: () => void;
  onBack: () => void;
}

const PickFolderPhase: React.FC<PickFolderPhaseProps> = ({
  file,
  vault,
  selectedPath,
  filePassword,
  fileError,
  onSelectedPathChange,
  onFilePasswordChange,
  onSave,
  onBack,
}) => {
  const tree = buildFolderTree(vault.index);
  const encrypted = isEncrypted(file);

  return (
    <>
      <h2>Choose folder</h2>
      <div className={classes["folder-panel"]}>
        <VaultFolderTree
          tree={tree}
          currentPath={selectedPath}
          onNavigate={onSelectedPathChange}
        />
        <div className={classes["folder-detail"]}>
          <p>
            Saving as: <strong>{file.name.replace(/\.lock$/, "")}</strong>
          </p>
          <p>
            Into: <strong>{selectedPath === "" ? "(root)" : selectedPath}</strong>
          </p>
        </div>
      </div>
      {encrypted && (
        <div className={classes.field}>
          <label>File password</label>
          <input
            type="password"
            value={filePassword}
            onChange={(e) => onFilePasswordChange(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") onSave(); }}
          />
        </div>
      )}
      {fileError && <p className={classes.error}>{fileError}</p>}
      <div className={classes.actions}>
        <button className={classes["btn-secondary"]} onClick={onBack}>
          Back
        </button>
        <button onClick={onSave} disabled={encrypted && !filePassword}>
          Save
        </button>
      </div>
    </>
  );
};
