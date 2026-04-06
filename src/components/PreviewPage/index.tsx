import { useEffect, useState } from "react";
import { VideoPlayerPage } from "../VideoPlayerPage";
import { ImageViewerPage } from "../ImageViewerPage";
import { GalleryPage } from "../GalleryPage";
import { RecentPreviewList } from "./RecentPreviewList";
import shared from "../shared.module.css";
import classes from "./PreviewPage.module.css";
import type { State } from "./PreviewPage.types";
import {
  detectViewer,
  fetchFileFromUrl,
  isValidHttpUrl,
} from "./PreviewPage.helpers";
import { pickFileWithHandle, tauriSupported } from "../../utils/platform";
import {
  type RecentPreviewEntry,
  addLocalRecentPreview,
  addRemoteRecentPreview,
  getRecentPreviews,
  removeRecentPreview,
  renameRecentPreview,
} from "../../utils/recentPreviews";

interface Props {
  onActiveChange?: (active: boolean) => void;
}

export const PreviewPage: React.FC<Props> = ({ onActiveChange }) => {
  const [state, setState] = useState<State>({ type: "idle" });
  const [urlInput, setUrlInput] = useState("");
  const [entries, setEntries] = useState<RecentPreviewEntry[]>([]);

  useEffect(() => {
    getRecentPreviews()
      .then(setEntries)
      .catch(() => {});
  }, []);

  useEffect(() => {
    onActiveChange?.(state.type !== "idle");
  }, [state.type]);

  async function refreshEntries() {
    try {
      setEntries(await getRecentPreviews());
    } catch {
      // non-fatal
    }
  }

  function openFile(file: File) {
    const viewer = detectViewer(file.name);
    if (viewer === null) {
      setState({ type: "unknown", file });
    } else {
      setState({ type: "viewing", file, viewer });
    }
  }

  async function handleChooseFile() {
    const picked = await pickFileWithHandle();
    if (!picked) return;
    const { file, handle } = picked;
    if (handle) {
      await addLocalRecentPreview(handle).catch(() => {});
      await refreshEntries();
    }
    openFile(file);
  }

  async function handleLoadUrl() {
    const url = urlInput.trim();
    if (!isValidHttpUrl(url)) {
      setState({
        type: "idle",
        error: "Please enter a valid HTTP or HTTPS URL",
      });
      return;
    }
    setState({ type: "fetching-url", url });
    try {
      const file = await fetchFileFromUrl(url);
      await addRemoteRecentPreview(url).catch(() => {});
      await refreshEntries();
      openFile(file);
    } catch (err) {
      setState({
        type: "idle",
        error: err instanceof Error ? err.message : "Failed to download file",
      });
    }
  }

  async function handleOpenRecent(entry: RecentPreviewEntry) {
    if (entry.type === "local" && entry.handle) {
      try {
        const permission = await entry.handle.requestPermission({
          mode: "read",
        });
        if (permission !== "granted") {
          setState({
            type: "idle",
            error: "File access permission was denied",
          });
          return;
        }
        const file = await entry.handle.getFile();
        await addLocalRecentPreview(entry.handle).catch(() => {});
        await refreshEntries();
        openFile(file);
      } catch (err) {
        setState({
          type: "idle",
          error: err instanceof Error ? err.message : "Failed to open file",
        });
      }
    } else if (entry.type === "remote" && entry.url) {
      setState({ type: "fetching-url", url: entry.url });
      try {
        const file = await fetchFileFromUrl(entry.url);
        await addRemoteRecentPreview(entry.url).catch(() => {});
        await refreshEntries();
        openFile(file);
      } catch (err) {
        setState({
          type: "idle",
          error: err instanceof Error ? err.message : "Failed to download file",
        });
      }
    }
  }

  async function handleRemoveRecent(id: number) {
    await removeRecentPreview(id).catch(() => {});
    await refreshEntries();
  }

  async function handleRenameRecent(id: number, alias: string) {
    await renameRecentPreview(id, alias).catch(() => {});
    await refreshEntries();
  }

  function handleReset() {
    setState({ type: "idle" });
  }

  if (state.type === "idle") {
    return (
      <div className={shared.container}>
        <RecentPreviewList
          entries={entries}
          onOpen={handleOpenRecent}
          onRemove={handleRemoveRecent}
          onRename={handleRenameRecent}
        />
        <div className={classes.idleContent}>
          <div className={shared.controls}>
            <button onClick={handleChooseFile}>Choose encrypted file</button>
          </div>
          {tauriSupported && (
            <>
              <div className={classes.divider}>
                <span>or paste a URL</span>
              </div>
              <div className={classes.urlRow}>
                <input
                  type="text"
                  placeholder="https://example.com/file.jpg.lock"
                  value={urlInput}
                  onChange={(e) => setUrlInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleLoadUrl();
                  }}
                />
                <button onClick={handleLoadUrl} disabled={!urlInput.trim()}>
                  Load
                </button>
              </div>
            </>
          )}
          {state.error && (
            <p className={shared.text} data-text-type="failure">
              {state.error}
            </p>
          )}
        </div>
      </div>
    );
  }

  if (state.type === "fetching-url") {
    return (
      <div className={shared.container}>
        <p className={shared.text}>Downloading file…</p>
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
