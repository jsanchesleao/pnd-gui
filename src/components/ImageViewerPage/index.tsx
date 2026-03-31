import { useEffect, useRef, useState } from "react";
import { decryptFileToBytes } from "../../utils/crypto";
import { getMimeType } from "../../utils/mediaTypes";
import shared from "../shared.module.css";
import type { Props, State } from "./ImageViewerPage.types";
import { ImageViewerForm } from "./ImageViewerForm";
import { ImageViewerDisplay } from "./ImageViewerDisplay";
import { DecryptingProgress } from "../DecryptingProgress";
import { DecryptError } from "../DecryptError";

export const ImageViewerPage: React.FC<Props> = ({ initialFile, onReset }) => {
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

  async function handleDecrypt() {
    if (state.type !== "password" || !password) return;
    const { file } = state;
    setState({ type: "loading", file, progress: 0 });

    try {
      const combined = await decryptFileToBytes(file, password, (percent) => {
        setState((prev) =>
          prev.type === "loading" ? { ...prev, progress: percent } : prev,
        );
      });

      revokeCurrentUrl();
      const mimeType = getMimeType(file.name);
      const blob = new Blob([combined], { type: mimeType });
      const objectUrl = URL.createObjectURL(blob);
      objectUrlRef.current = objectUrl;
      setState({ type: "viewing", file, objectUrl });
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
          <button onClick={handleChooseFile}>
            Choose encrypted image file
          </button>
        </div>
      </div>
    );
  }

  if (state.type === "password") {
    return (
      <ImageViewerForm
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
      <ImageViewerDisplay
        objectUrl={state.objectUrl}
        filename={state.file.name}
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
      <DecryptError
        message={state.message}
        onTryAgain={() => setState({ type: "password", file: state.file })}
        onChangeFile={handleChooseFile}
      />
    );
  }

  return null;
};
