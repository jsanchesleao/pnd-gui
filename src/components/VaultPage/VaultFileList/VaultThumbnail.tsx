import { useEffect, useState } from "react";
import { getFileCategory, isTextFile } from "../../../utils/mediaTypes";
import { FileIcon } from "./FileIcon";
import classes from "./VaultFileList.module.css";

export const VaultThumbnail: React.FC<{
  uuid: string;
  filename: string;
  isGenerating: boolean;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  onEnqueueThumbnail: (uuid: string) => void;
}> = ({ uuid, filename, isGenerating, onGetThumbnail, onEnqueueThumbnail }) => {
  const category = getFileCategory(filename);
  const hasThumb = category === "image" || category === "video" || isTextFile(filename);
  const [imgUrl, setImgUrl] = useState<string | null | "loading">("loading");

  useEffect(() => {
    if (!hasThumb) return;
    let active = true;
    setImgUrl("loading");
    onGetThumbnail(uuid).then((url) => {
      if (!active) return;
      if (url) {
        setImgUrl(url);
      } else {
        setImgUrl(null);
        // Request generation only when idle — the effect will re-run when
        // isGenerating transitions back to false after the thumbnail is saved.
        if ((category === "video" || isTextFile(filename)) && !isGenerating) {
          onEnqueueThumbnail(uuid);
        }
      }
    });
    return () => { active = false; };
  }, [uuid, filename, category, hasThumb, onGetThumbnail, isGenerating, onEnqueueThumbnail]);

  // Non-thumbnail files always show a static badge
  if (!hasThumb) {
    return <FileIcon category={category} />;
  }
  // While fetching / decrypting, show a gray pulsing placeholder
  if (imgUrl === "loading") {
    return <div className={classes["file-icon-placeholder"]} />;
  }
  // Thumbnail available — show it
  if (imgUrl) {
    return <img className={classes["file-grid-thumb-img"]} src={imgUrl} alt={filename} />;
  }
  // No thumbnail yet — show the category badge (pulse while generating)
  return <FileIcon category={category} generating={isGenerating} />;
};
