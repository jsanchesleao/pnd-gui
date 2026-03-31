import { useState } from "react";
import type { FolderNode } from "../../../utils/vault";
import classes from "./VaultFolderTree.module.css";

interface Props {
  tree: FolderNode;
  currentPath: string;
  onNavigate: (path: string) => void;
}

export const VaultFolderTree: React.FC<Props> = ({ tree, currentPath, onNavigate }) => {
  return (
    <nav className={classes["folder-tree"]}>
      <FolderNodeView
        node={tree}
        currentPath={currentPath}
        onNavigate={onNavigate}
        isRoot
      />
    </nav>
  );
};

interface NodeProps {
  node: FolderNode;
  currentPath: string;
  onNavigate: (path: string) => void;
  isRoot?: boolean;
}

const FolderNodeView: React.FC<NodeProps> = ({
  node,
  currentPath,
  onNavigate,
  isRoot,
}) => {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;
  const isActive = node.fullPath === currentPath;

  function handleClick() {
    onNavigate(node.fullPath);
    if (hasChildren) setExpanded((v) => !v);
  }

  return (
    <div className={classes["folder-node"]}>
      <div
        className={classes["folder-node-label"]}
        data-active={String(isActive)}
        onClick={handleClick}
        title={node.fullPath || "(root)"}
      >
        <span>{hasChildren ? (expanded ? "▾" : "▸") : "·"}</span>
        <span>{isRoot ? "(root)" : node.name}</span>
      </div>
      {hasChildren && expanded && (
        <div className={classes["folder-node-children"]}>
          {node.children.map((child) => (
            <FolderNodeView
              key={child.fullPath}
              node={child}
              currentPath={currentPath}
              onNavigate={onNavigate}
            />
          ))}
        </div>
      )}
    </div>
  );
};
