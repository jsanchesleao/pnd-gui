import { useEffect, useRef, useState } from "react";
import { unzipSync } from "fflate";
import { createDecryptedStream } from "../../utils/crypto";
import classes from "./GalleryPage.module.css";
import shared from "../shared.module.css";
import type { GalleryImage, State } from "./GalleryPage.types";
import { getMimeType, isImageFile } from "./GalleryPage.helpers";

interface Props {
  initialFile?: File;
  onReset?: () => void;
}

export const GalleryPage: React.FC<Props> = ({ initialFile, onReset }) => {
  const [state, setState] = useState<State>(
    initialFile ? { type: "password", file: initialFile } : { type: "idle" },
  );
  const [password, setPassword] = useState("");
  const objectUrlsRef = useRef<string[]>([]);

  useEffect(() => {
    return () => {
      revokeAllUrls();
    };
  }, []);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (state.type !== "viewing") return;
      if (e.key === "ArrowLeft" && state.index > 0) {
        setState({ ...state, index: state.index - 1 });
      } else if (e.key === "ArrowRight" && state.index < state.images.length - 1) {
        setState({ ...state, index: state.index + 1 });
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state]);

  function revokeAllUrls() {
    for (const url of objectUrlsRef.current) {
      URL.revokeObjectURL(url);
    }
    objectUrlsRef.current = [];
  }

  async function handleChooseFile() {
    const [handle] = await window.showOpenFilePicker();
    const file = await handle.getFile();
    revokeAllUrls();
    setPassword("");
    setState({ type: "password", file });
  }

  async function handleDecrypt() {
    if (state.type !== "password" || !password) return;
    const { file } = state;
    setState({ type: "loading", file, progress: 0 });

    try {
      const totalBytes = file.size;
      let processedBytes = 0;

      const progressStream = new TransformStream<Uint8Array, Uint8Array>({
        transform(chunk, controller) {
          processedBytes += chunk.byteLength;
          setState((prev) =>
            prev.type === "loading"
              ? { ...prev, progress: Math.round((processedBytes / totalBytes) * 100) }
              : prev,
          );
          controller.enqueue(chunk);
        },
      });

      const decryptedStream = createDecryptedStream(
        file.stream().pipeThrough(progressStream),
        password,
      );

      const chunks: Uint8Array[] = [];
      const reader = decryptedStream.getReader();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
      }

      const totalLength = chunks.reduce((acc, c) => acc + c.length, 0);
      const combined = new Uint8Array(totalLength);
      let offset = 0;
      for (const chunk of chunks) {
        combined.set(chunk, offset);
        offset += chunk.length;
      }

      const entries = unzipSync(combined);
      const imageEntries = Object.entries(entries)
        .filter(([name]) => isImageFile(name))
        .sort(([a], [b]) => a.localeCompare(b));

      if (imageEntries.length === 0) {
        setState({ type: "error", file, message: "No images found in archive" });
        return;
      }

      revokeAllUrls();
      const images: GalleryImage[] = imageEntries.map(([name, bytes]) => {
        const objectUrl = URL.createObjectURL(
          new Blob([bytes], { type: getMimeType(name) }),
        );
        objectUrlsRef.current.push(objectUrl);
        return { name, objectUrl };
      });

      setState({ type: "viewing", file, images, index: 0 });
    } catch (e: unknown) {
      setState({
        type: "error",
        file,
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }

  if (state.type === "idle") {
    return (
      <div className={shared.container}>
        <div className={shared.controls}>
          <button onClick={handleChooseFile}>Choose encrypted zip file</button>
        </div>
      </div>
    );
  }

  if (state.type === "password") {
    return (
      <div className={shared.container}>
        <p>{state.file.name}</p>
        <input
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleDecrypt()}
        />
        <div className={shared["button-group"]}>
          <button onClick={handleDecrypt}>View</button>
          <button onClick={onReset ?? handleChooseFile}>Change File</button>
        </div>
      </div>
    );
  }

  if (state.type === "loading") {
    return (
      <div className={shared.container}>
        <p>Decrypting {state.file.name}…</p>
        <progress className={shared.progress} value={state.progress} max={100} />
      </div>
    );
  }

  if (state.type === "viewing") {
    const { images, index } = state;
    const current = images[index];
    return (
      <div className={shared.container}>
        <div className={classes.carousel}>
          <div className={classes.imageWrapper}>
            <img
              className={classes.image}
              src={current.objectUrl}
              alt={current.name}
            />
          </div>
          <p className={classes.filename}>{current.name}</p>
          <div className={classes.carouselControls}>
            <button
              onClick={() => setState({ ...state, index: index - 1 })}
              disabled={index === 0}
            >
              &#8249;
            </button>
            <span className={classes.counter}>
              {index + 1} / {images.length}
            </span>
            <button
              onClick={() => setState({ ...state, index: index + 1 })}
              disabled={index === images.length - 1}
            >
              &#8250;
            </button>
          </div>
        </div>
        <div className={shared["button-group"]}>
          <button
            onClick={() => {
              revokeAllUrls();
              if (onReset) onReset();
              else setState({ type: "idle" });
            }}
          >
            Close
          </button>
          <button onClick={onReset ?? handleChooseFile}>Choose another file</button>
        </div>
      </div>
    );
  }

  if (state.type === "error") {
    return (
      <div className={shared.container}>
        <p className={shared.text} data-text-type="failure">
          Error: {state.message}
        </p>
        <button onClick={() => setState({ type: "password", file: state.file })}>
          Try again
        </button>
        <button onClick={onReset ?? handleChooseFile}>Change File</button>
      </div>
    );
  }

  return null;
};
