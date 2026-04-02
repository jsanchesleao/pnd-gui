import { useState } from "react";
import { VideoPlayerPage } from "../VideoPlayerPage";
import { ImageViewerPage } from "../ImageViewerPage";
import { GalleryPage } from "../GalleryPage";
import shared from "../shared.module.css";
import type { State } from "./PreviewPage.types";
import { detectViewer } from "./PreviewPage.helpers";
import { pickFile } from "../../utils/platform";

export const PreviewPage: React.FC = () => {
  const [state, setState] = useState<State>({ type: "idle" });

  async function handleChooseFile() {
    const file = await pickFile();
    if (!file) return;
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
