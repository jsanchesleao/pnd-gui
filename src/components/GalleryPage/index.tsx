import { useEffect, useRef, useState } from "react";
import { unzipSync } from "fflate";
import { createDecryptedStream } from "../../utils/crypto";
import shared from "../shared.module.css";
import type { GalleryImage, State } from "./GalleryPage.types";
import { getMimeType, isImageFile } from "./GalleryPage.helpers";
import { GalleryPasswordForm } from "./GalleryPasswordForm";
import { GalleryCarousel } from "./GalleryCarousel";
import { DecryptingProgress } from "../DecryptingProgress";
import { DecryptError } from "../DecryptError";

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

      setState({ type: "viewing", file, images });
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
      <GalleryPasswordForm
        file={state.file}
        password={password}
        onPasswordChange={setPassword}
        onDecrypt={handleDecrypt}
        onChangeFile={onReset ?? handleChooseFile}
      />
    );
  }

  if (state.type === "loading") {
    return <DecryptingProgress filename={state.file.name} progress={state.progress} />;
  }

  if (state.type === "viewing") {
    return (
      <GalleryCarousel
        images={state.images}
        onClose={() => {
          revokeAllUrls();
          if (onReset) onReset();
          else setState({ type: "idle" });
        }}
        onChooseAnother={onReset ?? handleChooseFile}
      />
    );
  }

  if (state.type === "error") {
    return (
      <DecryptError
        message={state.message}
        onTryAgain={() => setState({ type: "password", file: state.file })}
        onChangeFile={onReset ?? handleChooseFile}
      />
    );
  }

  return null;
};
