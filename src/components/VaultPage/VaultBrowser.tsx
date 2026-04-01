import { useEffect, useRef, useState } from "react";
import {
  buildFolderTree,
  getEntriesInPath,
  type VaultState,
} from "../../utils/vault";
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
        <button onClick={onAddFiles}>+ Add Files</button>
        <button onClick={onNewFolder}>+ New Folder</button>
        <button onClick={handleCut} disabled={selectedUuids.size === 0}>
          Cut{selectedUuids.size > 0 ? ` (${selectedUuids.size})` : ""}
        </button>
        <button onClick={onPaste} disabled={clipboard.length === 0}>
          Paste{clipboard.length > 0 ? ` (${clipboard.length})` : ""}
        </button>
        <button
          onClick={handleDeleteSelected}
          disabled={selectedUuids.size === 0}
        >
          Delete{selectedUuids.size > 0 ? ` (${selectedUuids.size})` : ""}
        </button>
        <span className={classes["toolbar-spacer"]} />
        <span style={{ fontSize: "0.85rem", opacity: 0.7 }}>{breadcrumb}</span>
        <span className={classes["toolbar-spacer"]} />
        <button onClick={onSave} disabled={!vault.modified}>
          {vault.modified && <span className={classes["modified-dot"]} />}
          Save
        </button>
        <button onClick={onClose}>Close</button>
      </div>
      <div
        className={classes.panels}
        onDragEnter={(e) => {
          e.preventDefault();
          console.log("DRAG ENTER");
          dragCountRef.current++;
          setIsDragOver(true);
        }}
        onDragOver={(e) => {
          e.preventDefault();
          console.log("DRAG OVER");
        }}
        onDragLeave={() => {
          console.log("DRAG LEAVE");
          dragCountRef.current--;
          if (dragCountRef.current === 0) setIsDragOver(false);
        }}
        onDrop={(e) => {
          e.preventDefault();
          console.log("DROPPED");
          dragCountRef.current = 0;
          setIsDragOver(false);
          const files = Array.from(e.dataTransfer.files);
          if (files.length > 0) onDropFiles(files);
        }}
        style={{ position: "relative" }}
      >
        <VaultFolderTree
          tree={tree}
          currentPath={currentPath}
          onNavigate={onNavigate}
        />
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
        {isDragOver && (
          <div className={classes["drop-overlay"]}>
            <span>Drop files to add to vault</span>
          </div>
        )}
      </div>
    </div>
  );
};
