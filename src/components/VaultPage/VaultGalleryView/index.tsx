import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getFileCategory } from "../../../utils/mediaTypes";
import { FileIcon } from "../VaultFileList/FileIcon";
import type { FileEntry } from "../VaultFileList/VaultFileList.types";
import classes from "./VaultGalleryView.module.css";

type ItemState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; objectUrl: string; type: "image" | "video" }
  | { status: "no-thumbnail" }
  | { status: "badge" };

interface Props {
  entries: FileEntry[];
  thumbnailGenerating: Set<string>;
  onGetThumbnail: (uuid: string) => Promise<string | null>;
  onEnqueueThumbnail: (uuid: string) => void;
  onPreview: (uuid: string) => void;
  onClose: () => void;
}

export const VaultGalleryView: React.FC<Props> = ({
  entries,
  thumbnailGenerating,
  onGetThumbnail,
  onEnqueueThumbnail,
  onPreview,
  onClose,
}) => {
  const [index, setIndex] = useState(0);
  const [itemStates, setItemStates] = useState<Map<string, ItemState>>(new Map());
  const loadedRef = useRef<Set<string>>(new Set());
  // Tracks uuids currently in "no-thumbnail" status so the re-check effect
  // doesn't need itemStates as a dependency.
  const noThumbnailRef = useRef<Set<string>>(new Set());

  const entriesByUuid = useMemo(
    () => new Map(entries.map((e) => [e.uuid, e])),
    [entries],
  );

  function setItemState(uuid: string, state: ItemState) {
    if (state.status === "no-thumbnail") {
      noThumbnailRef.current.add(uuid);
    } else {
      noThumbnailRef.current.delete(uuid);
    }
    setItemStates((prev) => new Map(prev).set(uuid, state));
  }

  const loadItem = useCallback(
    async (uuid: string, filename: string) => {
      if (loadedRef.current.has(uuid)) return;
      loadedRef.current.add(uuid);

      const category = getFileCategory(filename);

      if (category !== "image" && category !== "video") {
        setItemState(uuid, { status: "badge" });
        return;
      }

      setItemState(uuid, { status: "loading" });
      const url = await onGetThumbnail(uuid);

      if (url) {
        setItemState(uuid, { status: "ready", objectUrl: url, type: category });
      } else if (category === "video") {
        setItemState(uuid, { status: "no-thumbnail" });
        onEnqueueThumbnail(uuid); // VaultPage deduplicates enqueues internally
      } else {
        setItemState(uuid, { status: "badge" });
      }
    },
    [onGetThumbnail, onEnqueueThumbnail],
  );

  // Load current item + neighbours on navigation
  useEffect(() => {
    const targets = [entries[index], entries[index - 1], entries[index + 1]];
    for (const e of targets) {
      if (e) loadItem(e.uuid, e.entry.name);
    }
  }, [index, entries, loadItem]);

  // Re-check no-thumbnail videos when thumbnail generation finishes
  useEffect(() => {
    for (const uuid of noThumbnailRef.current) {
      if (thumbnailGenerating.has(uuid)) continue;
      loadedRef.current.delete(uuid);
      noThumbnailRef.current.delete(uuid);
      const entry = entriesByUuid.get(uuid);
      if (entry) loadItem(entry.uuid, entry.entry.name);
    }
  }, [thumbnailGenerating, entriesByUuid, loadItem]);

  // Keyboard navigation
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "ArrowLeft") setIndex((i) => Math.max(0, i - 1));
      else if (e.key === "ArrowRight")
        setIndex((i) => Math.min(entries.length - 1, i + 1));
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [entries.length]);

  const current = entries[index];
  const state: ItemState = current
    ? (itemStates.get(current.uuid) ?? { status: "idle" })
    : { status: "idle" };
  const category = current ? getFileCategory(current.entry.name) : "other";

  function renderViewer() {
    if (!current) {
      return (
        <div className={classes["badge-slot"]}>
          <FileIcon category="other" />
        </div>
      );
    }

    switch (state.status) {
      case "idle":
      case "loading":
        return <div className={classes.placeholder} />;

      case "ready":
        if (state.type === "video") {
          return (
            <div className={classes["thumb-wrapper"]}>
              <img src={state.objectUrl} alt={current.entry.name} />
              <button
                className={classes["play-btn"]}
                onClick={() => onPreview(current.uuid)}
                title="Play video"
              >
                &#9654;
              </button>
            </div>
          );
        }
        return <img src={state.objectUrl} alt={current.entry.name} />;

      case "no-thumbnail":
        return (
          <div className={classes["thumb-wrapper"]}>
            <div className={classes["badge-slot"]}>
              <FileIcon
                category="video"
                generating={thumbnailGenerating.has(current.uuid)}
              />
            </div>
            <button
              className={classes["play-btn"]}
              onClick={() => onPreview(current.uuid)}
              title="Play video"
            >
              &#9654;
            </button>
          </div>
        );

      case "badge":
        return (
          <div className={classes["badge-slot"]}>
            <FileIcon category={category} />
          </div>
        );
    }
  }

  return (
    <div className={classes.gallery}>
      <div className={classes.viewer}>{renderViewer()}</div>
      <div className={classes.info}>
        <span className={classes.filename} title={current?.entry.name ?? ""}>
          {current?.entry.name ?? ""}
        </span>
        <span className={classes.counter}>
          {entries.length === 0 ? "0 / 0" : `${index + 1} / ${entries.length}`}
        </span>
      </div>
      <div className={classes.nav}>
        <button
          onClick={() => setIndex((i) => i - 1)}
          disabled={index === 0 || entries.length === 0}
        >
          &#8249;
        </button>
        <button onClick={onClose}>Close gallery</button>
        {category !== "image" && category !== "video" && (
          <button
            onClick={() => onPreview(current?.uuid ?? "")}
            disabled={!current}
          >
            Open preview
          </button>
        )}
        <button
          onClick={() => setIndex((i) => i + 1)}
          disabled={index >= entries.length - 1 || entries.length === 0}
        >
          &#8250;
        </button>
      </div>
    </div>
  );
};
