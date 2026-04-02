import { useState } from "react";
import { LayoutGrid, List } from "lucide-react";
import type { Props, SortMode, SortOrder, ViewMode } from "./VaultFileList.types";
import { sortEntries } from "./VaultFileList.helpers";
import { VaultFileItem } from "./VaultFileItem";
import { VaultGridItem } from "./VaultGridItem";
import classes from "./VaultFileList.module.css";

export type { FileEntry } from "./VaultFileList.types";

export const VaultFileList: React.FC<Props> = ({
  entries,
  onPreview,
  onExport,
  onRename,
  onGetThumbnail,
  thumbnailGenerating,
  onEnqueueThumbnail,
  selectedUuids,
  onSelect,
}) => {
  const [sortBy, setSortBy] = useState<SortMode>("name");
  const [sortOrder, setSortOrder] = useState<SortOrder>("asc");
  const [viewMode, setViewMode] = useState<ViewMode>("list");

  if (entries.length === 0) {
    return (
      <div className={classes["file-list"]}>
        <p className={classes["file-list-empty"]}>This folder is empty.</p>
      </div>
    );
  }

  const sortedEntries = sortEntries(entries, sortBy, sortOrder);

  return (
    <div className={classes["file-list"]}>
      <div className={classes["file-list-sort-bar"]}>
        <label htmlFor="vault-sort">Sort by:</label>
        <select
          id="vault-sort"
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value as SortMode)}
        >
          <option value="name">Name</option>
          <option value="type">Type</option>
          <option value="size">Size</option>
          <option value="date">Date added</option>
        </select>
        <button
          className={classes["sort-order-btn"]}
          onClick={() => setSortOrder(o => o === "asc" ? "desc" : "asc")}
          title={sortOrder === "asc" ? "Ascending" : "Descending"}
        >
          {sortOrder === "asc" ? "↑" : "↓"}
        </button>
        <div className={classes["view-toggle"]}>
          <button
            data-active={viewMode === "list"}
            onClick={() => setViewMode("list")}
            title="List view"
          >
            <List size={14} />
          </button>
          <button
            data-active={viewMode === "grid"}
            onClick={() => setViewMode("grid")}
            title="Grid view"
          >
            <LayoutGrid size={14} />
          </button>
        </div>
      </div>
      {viewMode === "list" ? (
        sortedEntries.map(({ uuid, entry }) => (
          <VaultFileItem
            key={uuid}
            uuid={uuid}
            entry={entry}
            onPreview={onPreview}
            onExport={onExport}
            onRename={onRename}
            isSelected={selectedUuids.has(uuid)}
            onSelect={onSelect}
          />
        ))
      ) : (
        <div className={classes["file-grid"]}>
          {sortedEntries.map(({ uuid, entry }) => (
            <VaultGridItem
              key={uuid}
              uuid={uuid}
              entry={entry}
              onPreview={onPreview}
              onExport={onExport}
              onRename={onRename}
              onGetThumbnail={onGetThumbnail}
              isGenerating={thumbnailGenerating.has(uuid)}
              onEnqueueThumbnail={onEnqueueThumbnail}
              isSelected={selectedUuids.has(uuid)}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
};
