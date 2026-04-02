import { useEffect, useRef, useState } from "react";
import {
  buildFolderTree,
  getEntriesInPath,
  type VaultState,
} from "../../utils/vault";
import { fsaSupported } from "../../utils/platform";
import { ClipboardPaste, FilePlus2, FolderPlus, PanelLeft, Save, Scissors, Trash2, X } from "lucide-react";
import { VaultFileList } from "./VaultFileList";
import { VaultFolderTree } from "./VaultFolderTree";
import classes from "./VaultPage.module.css";

interface Props {
  vault: VaultState;
  currentPath: string;
  onNavigate: (path: string) => void;
  onAddFiles: () => void;
  onDropFiles: (files: File[]) => void;
  onNewFolder: () => void;
  onSave: () => void;
  onClose: () => void;
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  thumbnailGenerating: Set<string>;
  onEnqueueThumbnail: (uuid: string) => void;
  clipboard: string[];
  onCut: (uuids: string[]) => void;
  onPaste: () => void;
  onDeleteSelected: (uuids: string[]) => void;
}

export const VaultBrowser: React.FC<Props> = ({
  vault,
  currentPath,
  onNavigate,
  onAddFiles,
  onDropFiles,
  onNewFolder,
  onSave,
  onClose,
  onPreview,
  onExport,
  onRename,
  onGetThumbnail,
  thumbnailGenerating,
  onEnqueueThumbnail,
  clipboard,
  onCut,
  onPaste,
  onDeleteSelected,
}) => {
  const [selectedUuids, setSelectedUuids] = useState<Set<string>>(new Set());
  const [treeOpen, setTreeOpen] = useState(false);
  const dragCountRef = useRef(0);
  const [isDragOver, setIsDragOver] = useState(false);

  useEffect(() => {
    setSelectedUuids(new Set());
  }, [currentPath]);

  function handleSelect(uuid: string) {
    setSelectedUuids((prev) => {
      const next = new Set(prev);
      next.has(uuid) ? next.delete(uuid) : next.add(uuid);
      return next;
    });
  }

  function handleCut() {
    onCut(Array.from(selectedUuids));
    setSelectedUuids(new Set());
  }

  function handleDeleteSelected() {
    const uuids = Array.from(selectedUuids);
    const n = uuids.length;
    if (!confirm(`Delete ${n} item${n !== 1 ? "s" : ""}?`)) return;
    onDeleteSelected(uuids);
    setSelectedUuids(new Set());
  }

  const tree = buildFolderTree(vault.index);
  const entries = getEntriesInPath(vault.index, currentPath);
  const breadcrumb = currentPath === "" ? "(root)" : currentPath;

  return (
    <div className={classes.browser}>
      <div className={classes.toolbar}>
        <div className={classes["toolbar-actions"]}>
          <button
            className={classes["tree-toggle"]}
            onClick={() => setTreeOpen((o) => !o)}
            title={treeOpen ? "Hide folders" : "Show folders"}
          >
            <PanelLeft size={16} />
          </button>
          <button onClick={onAddFiles} title="Add files"><FilePlus2 size={16} /></button>
          <button onClick={onNewFolder} title="New folder"><FolderPlus size={16} /></button>
          <button
            onClick={handleCut}
            disabled={selectedUuids.size === 0}
            title={selectedUuids.size > 0 ? `Cut (${selectedUuids.size})` : "Cut"}
          >
            <Scissors size={16} />
            {selectedUuids.size > 0 && <span className={classes["btn-badge"]}>{selectedUuids.size}</span>}
          </button>
          <button
            onClick={onPaste}
            disabled={clipboard.length === 0}
            title={clipboard.length > 0 ? `Paste (${clipboard.length})` : "Paste"}
          >
            <ClipboardPaste size={16} />
            {clipboard.length > 0 && <span className={classes["btn-badge"]}>{clipboard.length}</span>}
          </button>
          <button
            onClick={handleDeleteSelected}
            disabled={selectedUuids.size === 0}
            title={selectedUuids.size > 0 ? `Delete (${selectedUuids.size})` : "Delete"}
          >
            <Trash2 size={16} />
            {selectedUuids.size > 0 && <span className={classes["btn-badge"]}>{selectedUuids.size}</span>}
          </button>
        </div>
        <span className={classes["toolbar-spacer"]} />
        <span className={classes["toolbar-breadcrumb"]}>{breadcrumb}</span>
        <span className={classes["toolbar-spacer"]} />
        <div className={classes["toolbar-vault"]}>
          <button onClick={onSave} disabled={!vault.modified} title="Save vault">
            {vault.modified && <span className={classes["modified-dot"]} />}
            <Save size={16} />
          </button>
          <button onClick={onClose} title="Close vault"><X size={16} /></button>
        </div>
      </div>
      <div
        className={classes.panels}
        data-tree-open={String(treeOpen)}
        {...(fsaSupported
          ? {
              onDragEnter: (e) => {
                e.preventDefault();
                dragCountRef.current++;
                setIsDragOver(true);
              },
              onDragOver: (e) => {
                e.preventDefault();
              },
              onDragLeave: () => {
                dragCountRef.current--;
                if (dragCountRef.current === 0) setIsDragOver(false);
              },
              onDrop: (e) => {
                e.preventDefault();
                dragCountRef.current = 0;
                setIsDragOver(false);
                const files = Array.from(e.dataTransfer.files);
                if (files.length > 0) onDropFiles(files);
              },
            }
          : {})}
        style={{ position: "relative" }}
      >
        <div
          className={classes["folder-tree-wrapper"]}
          data-hidden={String(!treeOpen)}
        >
          <VaultFolderTree
            tree={tree}
            currentPath={currentPath}
            onNavigate={onNavigate}
          />
        </div>
        <VaultFileList
          entries={entries}
          onPreview={onPreview}
          onExport={onExport}
          onRename={onRename}
          onGetThumbnail={onGetThumbnail}
          thumbnailGenerating={thumbnailGenerating}
          onEnqueueThumbnail={onEnqueueThumbnail}
          selectedUuids={selectedUuids}
          onSelect={handleSelect}
        />
        {fsaSupported && isDragOver && (
          <div className={classes["drop-overlay"]}>
            <span>Drop files to add to vault</span>
          </div>
        )}
      </div>
    </div>
  );
};
