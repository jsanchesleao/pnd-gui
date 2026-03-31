import type { FileCategory } from "../../../utils/mediaTypes";
import { CATEGORY_COLORS, CATEGORY_LABELS } from "./VaultFileList.constants";
import classes from "./VaultFileList.module.css";

export const FileIcon: React.FC<{ category: FileCategory; generating?: boolean }> = ({ category, generating }) => (
  <div
    className={`${classes["file-icon"]}${generating ? ` ${classes["file-icon-generating"]}` : ""}`}
    style={{ backgroundColor: CATEGORY_COLORS[category] }}
  >
    {CATEGORY_LABELS[category]}
  </div>
);
