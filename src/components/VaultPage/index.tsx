import { useRef, useState } from "react";
import shared from "../shared.module.css";
import classes from "./VaultPage.module.css";
import {
  VaultError,
  addFileToVault,
  createEmptyVault,
  decryptVaultFile,
  exportVaultFile,
  moveFileInVault,
  openVault,
  removeFileFromVault,
  renameFileInVault,
  saveVault,
  type VaultState,
} from "../../utils/vault";
import { VaultBrowser } from "./VaultBrowser";
import {
  VaultPreviewPanel,
  buildPreviewState,
  type PreviewState,
} from "./VaultPreviewPanel";

type Phase =
  | { phase: "idle" }
  | { phase: "unlocking"; operation: "open" | "create"; handle: FileSystemDirectoryHandle; error?: string }
  | { phase: "saving" }
  | { phase: "browsing"; currentPath: string };

interface Props {
  onModifiedChange?: (modified: boolean) => void;
}

export const VaultPage: React.FC<Props> = ({ onModifiedChange }) => {
  const [pageState, setPageState] = useState<Phase>({ phase: "idle" });
  const [vault, setVaultState] = useState<VaultState | null>(null);
  const [password, setPassword] = useState("");
  const [preview, setPreview] = useState<PreviewState | null>(null);
  const [addProgress, setAddProgress] = useState<number | null>(null);
  const previewUrlRef = useRef<string | null>(null);

  function updateVault(v: VaultState | null) {
    setVaultState(v);
    onModifiedChange?.(v?.modified ?? false);
  }

  function revokePreview() {
    if (previewUrlRef.current) {
      URL.revokeObjectURL(previewUrlRef.current);
      previewUrlRef.current = null;
    }
    setPreview(null);
  }

  // ── Open / Create ────────────────────────────────────────────────────────

  async function handleOpenVault() {
    try {
      const handle = await window.showDirectoryPicker();
      setPassword("");
      setPageState({ phase: "unlocking", operation: "open", handle });
    } catch {
      // user cancelled picker
    }
  }

  async function handleCreateVault() {
    try {
      const handle = await window.showDirectoryPicker();
      setPassword("");
      setPageState({ phase: "unlocking", operation: "create", handle });
    } catch {
      // user cancelled picker
    }
  }

  async function handleUnlock() {
    if (pageState.phase !== "unlocking" || !password) return;
    const { handle, operation } = pageState;

    try {
      if (operation === "open") {
        const v = await openVault(handle, password);
        updateVault(v);
        setPageState({ phase: "browsing", currentPath: "" });
      } else {
        const v = createEmptyVault(handle, password);
        setPageState({ phase: "saving" });
        await saveVault(v);
        updateVault(v);
        setPageState({ phase: "browsing", currentPath: "" });
      }
    } catch (e) {
      if (e instanceof VaultError) {
        setPageState({ ...pageState, error: e.code === "WRONG_PASSWORD" ? "Wrong password." : e.message });
      } else {
        setPageState({ ...pageState, error: e instanceof Error ? e.message : String(e) });
      }
    }
  }

  // ── Browser actions ──────────────────────────────────────────────────────

  async function handleAddFiles() {
    if (!vault || pageState.phase !== "browsing") return;
    try {
      const handles = await window.showOpenFilePicker({ multiple: true } as Parameters<typeof window.showOpenFilePicker>[0]);
      const total = handles.length;
      setAddProgress(0);
      for (let i = 0; i < handles.length; i++) {
        const file = await handles[i].getFile();
        const bytes = new Uint8Array(await file.arrayBuffer());
        await addFileToVault(vault, bytes, file.name, pageState.currentPath, (pct) => {
          setAddProgress(Math.round(((i + pct / 100) / total) * 100));
        });
      }
      setAddProgress(null);
      updateVault({ ...vault });
    } catch {
      setAddProgress(null);
    }
  }

  function handleNewFolder() {
    if (pageState.phase !== "browsing") return;
    const name = prompt("New folder name:");
    if (!name?.trim()) return;
    const base = pageState.currentPath;
    const newPath = base === "" ? name.trim() : `${base}/${name.trim()}`;
    setPageState({ ...pageState, currentPath: newPath });
  }

  async function handleSave() {
    if (!vault) return;
    setPageState({ phase: "saving" });
    try {
      await saveVault(vault);
      updateVault({ ...vault });
      setPageState({ phase: "browsing", currentPath: (pageState as { currentPath?: string }).currentPath ?? "" });
    } catch (e) {
      alert(`Save failed: ${e instanceof Error ? e.message : String(e)}`);
      setPageState({ phase: "browsing", currentPath: (pageState as { currentPath?: string }).currentPath ?? "" });
    }
  }

  function handleClose() {
    if (vault?.modified) {
      if (!confirm("You have unsaved changes. Close anyway?")) return;
    }
    revokePreview();
    updateVault(null);
    setPageState({ phase: "idle" });
  }

  async function handleDelete(uuid: string) {
    if (!vault) return;
    await removeFileFromVault(vault, uuid);
    if (preview && "uuid" in preview && preview.uuid === uuid) revokePreview();
    updateVault({ ...vault });
  }

  function handleRename(uuid: string, newName: string): string | null {
    if (!vault) return "No vault open";
    try {
      renameFileInVault(vault, uuid, newName);
      updateVault({ ...vault });
      return null;
    } catch (e) {
      return e instanceof VaultError ? e.message : String(e);
    }
  }

  function handleMove(uuid: string, newPath: string) {
    if (!vault) return;
    moveFileInVault(vault, uuid, newPath);
    updateVault({ ...vault });
  }

  async function handleExport(uuid: string) {
    if (!vault) return;
    const entry = vault.index.entries[uuid];
    if (!entry) return;
    try {
      const saveHandle = await window.showSaveFilePicker({ suggestedName: entry.name });
      const writable = await saveHandle.createWritable();
      await exportVaultFile(vault, uuid, writable);
      await writable.close();
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") return;
      alert(`Export failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  async function handlePreview(uuid: string) {
    if (!vault) return;
    const entry = vault.index.entries[uuid];
    if (!entry) return;

    revokePreview();
    setPreview({ type: "loading", uuid });

    try {
      const bytes = await decryptVaultFile(vault, uuid);
      if (!bytes) {
        setPreview({ type: "unsupported", uuid, name: entry.name });
        return;
      }
      const state = buildPreviewState(uuid, entry.name, bytes);
      if ("objectUrl" in state) previewUrlRef.current = state.objectUrl;
      setPreview(state);
    } catch {
      setPreview({ type: "unsupported", uuid, name: entry.name });
    }
  }

  // ── Render ───────────────────────────────────────────────────────────────

  if (pageState.phase === "idle") {
    return (
      <div className={shared.container}>
        <div className={shared.controls}>
          <button onClick={handleOpenVault}>Open Vault</button>
          <button onClick={handleCreateVault}>New Vault</button>
        </div>
      </div>
    );
  }

  if (pageState.phase === "unlocking") {
    return (
      <div className={shared.container}>
        <p>
          {pageState.operation === "open" ? "Unlock vault" : "Set master password for new vault"}
        </p>
        <div className={shared.controls}>
          <input
            type="password"
            placeholder="Master password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
            autoFocus
          />
          {pageState.error && (
            <p className={shared.text} data-text-type="failure">
              {pageState.error}
            </p>
          )}
          <div className={shared["button-group"]}>
            <button onClick={handleUnlock} disabled={!password}>
              {pageState.operation === "open" ? "Unlock" : "Create"}
            </button>
            <button onClick={() => setPageState({ phase: "idle" })}>Cancel</button>
          </div>
        </div>
      </div>
    );
  }

  if (pageState.phase === "saving") {
    return (
      <div className={shared.container}>
        <p>Saving vault…</p>
      </div>
    );
  }

  if (pageState.phase === "browsing" && vault) {
    return (
      <div style={{ position: "relative", height: "100%" }}>
        <VaultBrowser
          vault={vault}
          currentPath={pageState.currentPath}
          onNavigate={(path) => setPageState({ ...pageState, currentPath: path })}
          onAddFiles={handleAddFiles}
          onNewFolder={handleNewFolder}
          onSave={handleSave}
          onClose={handleClose}
          onPreview={handlePreview}
          onExport={handleExport}
          onDelete={handleDelete}
          onRename={handleRename}
          onMove={handleMove}
        />
        {addProgress !== null && (
          <div className={classes["add-progress-overlay"]}>
            <p>Adding files…</p>
            <progress className={shared.progress} value={addProgress} max={100} />
          </div>
        )}
        {preview && (
          <VaultPreviewPanel preview={preview} onClose={revokePreview} />
        )}
      </div>
    );
  }

  return null;
};
