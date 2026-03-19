import { useRef, useState } from "react";
import { createDecryptedStream, createEncryptedStream } from "../utils/crypto";
import classes from "./GenericPage.module.css";

export type GenericPageProps = {};

export const GenericPage: React.FC<GenericPageProps> = () => {
  const [file, setFile] = useState<File | null>(null);
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState<
    "idle" | "processing" | "done" | "error"
  >("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const abortControllerRef = useRef<AbortController | null>(null);

  const mode = file && file.name.endsWith(".lock") ? "decrypt" : "encrypt";
  const isEncrypt = mode === "encrypt";

  async function handleChooseFile() {
    const [handle] = await window.showOpenFilePicker();
    setFile(await handle.getFile());
    setStatus("idle");
    setErrorMessage(null);
    setProgress(0);
  }

  async function handleProcess() {
    if (!file || !password) return;
    setStatus("processing");
    setErrorMessage(null);
    setProgress(0);
    const abortController = new AbortController();
    abortControllerRef.current = abortController;
    try {
      const suggestedName =
        mode === "encrypt"
          ? file.name + ".lock"
          : file.name.endsWith(".lock")
            ? file.name.slice(0, -5)
            : file.name + ".lock";
      const saveHandle = await window.showSaveFilePicker({ suggestedName });
      const writable = await saveHandle.createWritable();
      const totalBytes = file.size;
      let processedBytes = 0;
      const progressStream = new TransformStream<Uint8Array, Uint8Array>({
        transform(chunk, controller) {
          processedBytes += chunk.byteLength;
          setProgress(Math.round((processedBytes / totalBytes) * 100));
          controller.enqueue(chunk);
        },
      });
      const processed =
        mode === "encrypt"
          ? createEncryptedStream(
              file.stream().pipeThrough(progressStream),
              password,
            )
          : createDecryptedStream(
              file.stream().pipeThrough(progressStream),
              password,
            );
      await processed.pipeTo(writable, { signal: abortController.signal });
      setStatus("done");
    } catch (e: unknown) {
      if (e instanceof DOMException && e.name === "AbortError") {
        setStatus("idle");
      } else {
        setStatus("error");
        setErrorMessage(e instanceof Error ? e.message : String(e));
      }
    }
  }

  function handleCancel() {
    abortControllerRef.current?.abort();
  }

  if (!file) {
    return (
      <div className={classes.container}>
        <button onClick={handleChooseFile}>Choose File</button>
      </div>
    );
  }

  if (status === "idle") {
    return (
      <div className={classes.container}>
        <p>
          Enter password to <strong>{isEncrypt ? "encrypt" : "decrypt"}</strong>{" "}
          {file.name}
        </p>
        <input
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <button onClick={handleProcess}>
          {isEncrypt ? "Encrypt" : "Decrypt"}
        </button>
        <button onClick={handleChooseFile}>Change File</button>
      </div>
    );
  }

  if (status === "processing") {
    return (
      <div className={classes.container}>
        <p>
          {isEncrypt ? "Encrypting" : "Decrypting"} {file.name}{" "}
        </p>
        <progress className={classes.progress} value={progress} max={100} />
        <button onClick={handleCancel}>Cancel</button>
      </div>
    );
  }

  if (status === "done") {
    return (
      <div className={classes.container}>
        <button onClick={handleChooseFile}>Choose File</button>
        <p className={classes.text} data-text-type="success">
          File {isEncrypt ? "encrypted" : "decrypted"} and saved successfully.
        </p>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className={classes.container}>
        <button onClick={handleChooseFile}>Choose File</button>
        <p className={classes.text} data-text-type="failure">
          Error: {errorMessage}
        </p>
      </div>
    );
  }

  return null;
};
