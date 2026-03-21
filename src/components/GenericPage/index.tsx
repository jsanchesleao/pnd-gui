import { useRef, useState } from "react";
import {
  createDecryptedStream,
  createEncryptedStream,
} from "../../utils/crypto";
import shared from "../shared.module.css";

export type GenericPageProps = {};

const BLOCK_SIZE_OPTIONS = [
  { label: "256 KB", value: 262144 },
  { label: "512 KB", value: 524288 },
  { label: "1 MB (default)", value: 1048576 },
  { label: "2 MB", value: 2097152 },
  { label: "4 MB", value: 4194304 },
  { label: "8 MB", value: 8388608 },
];

const DEFAULT_BLOCK_SIZE = BLOCK_SIZE_OPTIONS[2];

export const GenericPage: React.FC<GenericPageProps> = () => {
  const [file, setFile] = useState<File | null>(null);
  const [password, setPassword] = useState("");
  const [chunkSize, setChunkSize] = useState(DEFAULT_BLOCK_SIZE.value);
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
              chunkSize,
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
      <div className={shared.container}>
        <div className={shared.controls}>
          <button onClick={handleChooseFile}>Choose File</button>
        </div>
      </div>
    );
  }

  if (status === "idle") {
    return (
      <div className={shared.container}>
        <p>
          Enter password to <strong>{isEncrypt ? "encrypt" : "decrypt"}</strong>{" "}
          {file.name}
        </p>
        <div className={shared.controls}>
          <input
            type="password"
            placeholder="Password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          {isEncrypt && (
            <select
              value={chunkSize}
              onChange={(e) => setChunkSize(Number(e.target.value))}
            >
              {BLOCK_SIZE_OPTIONS.map(({ label, value }) => (
                <option key={value} value={value}>
                  Block size: {label}
                </option>
              ))}
            </select>
          )}
          <div className={shared["button-group"]}>
            <button onClick={handleProcess}>
              {isEncrypt ? "Encrypt" : "Decrypt"}
            </button>
            <button onClick={handleChooseFile}>Change File</button>
          </div>
        </div>
      </div>
    );
  }

  if (status === "processing") {
    return (
      <div className={shared.container}>
        <p>
          {isEncrypt ? "Encrypting" : "Decrypting"} {file.name}{" "}
        </p>
        <progress className={shared.progress} value={progress} max={100} />
        <button onClick={handleCancel}>Cancel</button>
      </div>
    );
  }

  if (status === "done") {
    return (
      <div className={shared.container}>
        <p className={shared.text} data-text-type="success">
          File {isEncrypt ? "encrypted" : "decrypted"} and saved successfully.
        </p>
        <div className={shared.controls}>
          <button onClick={handleChooseFile}>Choose File</button>
        </div>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className={shared.container}>
        <p className={shared.text} data-text-type="failure">
          Error: {errorMessage}
        </p>
        <div className={shared.controls}>
          <button onClick={handleChooseFile}>Choose File</button>
        </div>
      </div>
    );
  }

  return null;
};
