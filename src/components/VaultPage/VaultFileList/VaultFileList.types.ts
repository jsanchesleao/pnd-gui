import type { VaultIndexEntry } from "../../../utils/vault";

export type SortMode = "name" | "type" | "size" | "date";
export type SortOrder = "asc" | "desc";
export type ViewMode = "list" | "grid";

export interface FileEntry {
  uuid: string;
  entry: VaultIndexEntry;
}

export interface Props {
  entries: FileEntry[];
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  thumbnailGenerating: Set<string>;
  onEnqueueThumbnail: (uuid: string) => void;
  selectedUuids: Set<string>;
  onSelect: (uuid: string) => void;
}

export interface ItemProps {
  uuid: string;
  entry: VaultIndexEntry;
  onPreview: (uuid: string) => void;
  onExport: (uuid: string) => void;
  onRename: (uuid: string, newName: string) => string | null;
  isSelected: boolean;
  onSelect: (uuid: string) => void;
}

export interface GridItemProps extends ItemProps {
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  isGenerating: boolean;
  onEnqueueThumbnail: (uuid: string) => void;
}
