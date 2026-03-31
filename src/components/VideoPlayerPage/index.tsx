import { useEffect, useRef, useState } from "react";
import { createDecryptedStream } from "../../utils/crypto";
import { getMimeType } from "../../utils/mediaTypes";
import shared from "../shared.module.css";
import type { Props, State } from "./VideoPlayerPage.types";
import { VideoPlayerForm } from "./VideoPlayerForm";
import { VideoPlayerDisplay } from "./VideoPlayerDisplay";

export const VideoPlayerPage: React.FC<Props> = ({ initialFile, onReset }) => {
  const [state, setState] = useState<State>(
    initialFile ? { type: "password", file: initialFile } : { type: "idle" },
  );
  const [password, setPassword] = useState("");
  const objectUrlRef = useRef<string | null>(null);

  useEffect(() => {
    return () => {
      if (objectUrlRef.current) {
        URL.revokeObjectURL(objectUrlRef.current);
      }
    };
  }, []);

  function revokeCurrentUrl() {
    if (objectUrlRef.current) {
      URL.revokeObjectURL(objectUrlRef.current);
      objectUrlRef.current = null;
    }
  }

  async function handleChooseFile() {
    const [handle] = await window.showOpenFilePicker();
    const file = await handle.getFile();
    revokeCurrentUrl();
    setPassword("");
    setState({ type: "password", file });
  }

  async function handlePlay() {
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
              ? {
                  ...prev,
                  progress: Math.round((processedBytes / totalBytes) * 100),
                }
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

      revokeCurrentUrl();
      const mimeType = getMimeType(file.name);
      const blob = new Blob([combined], { type: mimeType });
      const objectUrl = URL.createObjectURL(blob);
      objectUrlRef.current = objectUrl;
      setState({ type: "playing", file, objectUrl });
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
          <div className={shared["button-group"]}>
            <button onClick={handleChooseFile}>
              Choose encrypted video file
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (state.type === "password") {
    return (
      <VideoPlayerForm
        file={state.file}
        password={password}
        onPasswordChange={setPassword}
        onPlay={handlePlay}
        onChangeFile={onReset ?? handleChooseFile}
      />
    );
  }

  if (state.type === "loading") {
    return (
      <div className={shared.container}>
        <p>Decrypting {state.file.name}…</p>
        <progress
          className={shared.progress}
          value={state.progress}
          max={100}
        />
      </div>
    );
  }

  if (state.type === "playing") {
    return (
      <VideoPlayerDisplay
        objectUrl={state.objectUrl}
        onClose={() => {
          revokeCurrentUrl();
          if (onReset) onReset();
          else setState({ type: "idle" });
        }}
        onChooseAnother={onReset ?? handleChooseFile}
      />
    );
  }

  if (state.type === "error") {
    return (
      <div className={shared.container}>
        <p className={shared.text} data-text-type="failure">
          Error: {state.message}
        </p>
        <button
          onClick={() => setState({ type: "password", file: state.file })}
        >
          Try again
        </button>
        <button onClick={onReset ?? handleChooseFile}>Change File</button>
      </div>
    );
  }

  return null;
};
