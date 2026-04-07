import { useState } from "react";
import { decryptFileToBytes } from "../../utils/crypto";
import { pickFile } from "../../utils/platform";
import shared from "../shared.module.css";
import classes from "./TextViewerPage.module.css";
import type { Props, State } from "./TextViewerPage.types";
import { ImageViewerForm } from "../ImageViewerPage/ImageViewerForm";
import { DecryptingProgress } from "../DecryptingProgress";
import { DecryptError } from "../DecryptError";
import { MarkdownView } from "../MarkdownView";

export const TextViewerPage: React.FC<Props> = ({ initialFile, onReset }) => {
  const [state, setState] = useState<State>(
    initialFile ? { type: "password", file: initialFile } : { type: "idle" },
  );
  const [password, setPassword] = useState("");
  const [formatted, setFormatted] = useState(false);

  async function handleChooseFile() {
    const file = await pickFile();
    if (!file) return;
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

      const textContent = new TextDecoder().decode(combined);
      setState({ type: "viewing", file, textContent });
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
          <button onClick={handleChooseFile}>Choose encrypted text file</button>
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
    const isMarkdown = state.file.name.toLowerCase().endsWith(".md");
    return (
      <div className={shared.container}>
        <div className={shared.controls}>
          <span>{state.file.name}</span>
          <div className={shared["button-group"]}>
            {isMarkdown && (
              <button onClick={() => setFormatted((f) => !f)}>
                {formatted ? "View Raw" : "View Formatted"}
              </button>
            )}
            <button onClick={onReset ?? handleChooseFile}>Close</button>
          </div>
        </div>
        {formatted && isMarkdown
          ? <MarkdownView text={state.textContent} />
          : <pre className={classes["text-content"]}>{state.textContent}</pre>
        }
      </div>
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
