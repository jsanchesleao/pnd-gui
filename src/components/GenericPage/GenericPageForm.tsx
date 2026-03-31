import shared from "../shared.module.css";
import { BLOCK_SIZE_OPTIONS } from "./GenericPage.constants";

interface Props {
  file: File;
  isEncrypt: boolean;
  password: string;
  chunkSize: number;
  onPasswordChange: (value: string) => void;
  onChunkSizeChange: (value: number) => void;
  onProcess: () => void;
  onChooseFile: () => void;
}

export const GenericPageForm: React.FC<Props> = ({
  file,
  isEncrypt,
  password,
  chunkSize,
  onPasswordChange,
  onChunkSizeChange,
  onProcess,
  onChooseFile,
}) => (
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
        onChange={(e) => onPasswordChange(e.target.value)}
      />
      {isEncrypt && (
        <select
          value={chunkSize}
          onChange={(e) => onChunkSizeChange(Number(e.target.value))}
        >
          {BLOCK_SIZE_OPTIONS.map(({ label, value }) => (
            <option key={value} value={value}>
              Block size: {label}
            </option>
          ))}
        </select>
      )}
      <div className={shared["button-group"]}>
        <button onClick={onProcess}>
          {isEncrypt ? "Encrypt" : "Decrypt"}
        </button>
        <button onClick={onChooseFile}>Change File</button>
      </div>
    </div>
  </div>
);
