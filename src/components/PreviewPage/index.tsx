import { useState } from "react";
import { VideoPlayerPage } from "../VideoPlayerPage";
import { ImageViewerPage } from "../ImageViewerPage";
import { GalleryPage } from "../GalleryPage";
import shared from "../shared.module.css";

type Viewer = "video" | "image" | "gallery";

type State =
  | { type: "idle" }
  | { type: "viewing"; file: File; viewer: Viewer }
  | { type: "unknown"; file: File };

const VIDEO_EXTS = new Set(["mp4", "webm", "mkv", "mov", "avi"]);
const IMAGE_EXTS = new Set(["jpg", "jpeg", "png", "gif", "webp", "avif", "bmp", "svg"]);

function detectViewer(filename: string): Viewer | null {
  const base = filename.endsWith(".lock") ? filename.slice(0, -5) : filename;
  const ext = base.split(".").pop()?.toLowerCase() ?? "";
  if (ext === "zip") return "gallery";
  if (VIDEO_EXTS.has(ext)) return "video";
  if (IMAGE_EXTS.has(ext)) return "image";
  return null;
}

export const PreviewPage: React.FC = () => {
  const [state, setState] = useState<State>({ type: "idle" });

  async function handleChooseFile() {
    const [handle] = await window.showOpenFilePicker();
    const file = await handle.getFile();
    const viewer = detectViewer(file.name);
    if (viewer === null) {
      setState({ type: "unknown", file });
    } else {
      setState({ type: "viewing", file, viewer });
    }
  }

  function handleReset() {
    setState({ type: "idle" });
  }

  if (state.type === "idle") {
    return (
      <div className={shared.container}>
        <div className={shared.controls}>
          <button onClick={handleChooseFile}>Choose encrypted file</button>
        </div>
      </div>
    );
  }

  if (state.type === "unknown") {
    return (
      <div className={shared.container}>
        <p className={shared.text} data-text-type="failure">
          Unsupported file type: {state.file.name}
        </p>
        <button onClick={handleChooseFile}>Choose another file</button>
      </div>
    );
  }

  if (state.type === "viewing") {
    const { file, viewer } = state;
    if (viewer === "video") {
      return <VideoPlayerPage initialFile={file} onReset={handleReset} />;
    }
    if (viewer === "image") {
      return <ImageViewerPage initialFile={file} onReset={handleReset} />;
    }
    return <GalleryPage initialFile={file} onReset={handleReset} />;
  }

  return null;
};
