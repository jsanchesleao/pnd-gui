import { buildFolderTree, getEntriesInPath, type VaultState } from "../../utils/vault";
import { VaultFileList } from "./VaultFileList";
import { VaultFolderTree } from "./VaultFolderTree";
import classes from "./VaultPage.module.css";

interface Props {
  vault: VaultState;
  currentPath: string;
  onNavigate: (path: string) => void;
  onAddFiles: () => void;
  onNewFolder: () => void;
  onSave: () => void;
  onClose: () => void;
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onDelete: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  onMove: (uuid: string, newPath: string) => void;
}

export const VaultBrowser: React.FC<Props> = ({
  vault,
  currentPath,
  onNavigate,
  onAddFiles,
  onNewFolder,
  onSave,
  onClose,
  onPreview,
  onExport,
  onDelete,
  onRename,
  onMove,
}) => {
  const tree = buildFolderTree(vault.index);
  const entries = getEntriesInPath(vault.index, currentPath);
  const breadcrumb = currentPath === "" ? "(root)" : currentPath;

  return (
    <div className={classes.browser}>
      <div className={classes.toolbar}>
        <button onClick={onAddFiles}>+ Add Files</button>
        <button onClick={onNewFolder}>+ New Folder</button>
        <span className={classes["toolbar-spacer"]} />
        <span style={{ fontSize: "0.85rem", opacity: 0.7 }}>{breadcrumb}</span>
        <span className={classes["toolbar-spacer"]} />
        <button onClick={onSave} disabled={!vault.modified}>
          {vault.modified && <span className={classes["modified-dot"]} />}
          Save
        </button>
        <button onClick={onClose}>Close</button>
      </div>
      <div className={classes.panels}>
        <VaultFolderTree
          tree={tree}
          currentPath={currentPath}
          onNavigate={onNavigate}
        />
        <VaultFileList
          entries={entries}
          onPreview={onPreview}
          onExport={onExport}
          onDelete={onDelete}
          onRename={onRename}
          onMove={onMove}
        />
      </div>
    </div>
  );
};
