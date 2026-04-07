import { useState } from "react";
import classes from "./VaultPreviewPanel.module.css";
import { MarkdownView } from "../../MarkdownView";

interface Props {
  uuid: string;
  name: string;
  text: string;
  onClose: () => void;
  onSave?: (uuid: string, text: string) => Promise<void>;
}

export const VaultTextEditor: React.FC<Props> = ({ uuid, name, text, onClose, onSave }) => {
  const [value, setValue] = useState(text);
  const [saving, setSaving] = useState(false);
  const [formatted, setFormatted] = useState(false);
  const isMarkdown = name.toLowerCase().endsWith(".md");

  async function handleSave() {
    if (!onSave) return;
    setSaving(true);
    try {
      await onSave(uuid, value);
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <p className={classes["text-editor-name"]}>{name}</p>
      {formatted && isMarkdown
        ? <MarkdownView text={value} maxHeight="60vh" />
        : (
          <textarea
            className={classes["text-editor"]}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            disabled={saving}
          />
        )
      }
      <div style={{ display: "flex", gap: "0.5rem" }}>
        {isMarkdown && (
          <button onClick={() => setFormatted((f) => !f)} disabled={saving}>
            {formatted ? "View Raw" : "View Formatted"}
          </button>
        )}
        {onSave && !formatted && (
          <button onClick={handleSave} disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </button>
        )}
        <button onClick={onClose} disabled={saving}>Close</button>
      </div>
    </>
  );
};
