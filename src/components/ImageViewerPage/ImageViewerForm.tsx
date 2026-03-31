import shared from "../shared.module.css";

interface Props {
  file: File;
  password: string;
  onPasswordChange: (value: string) => void;
  onDecrypt: () => void;
  onChangeFile: () => void;
}

export const ImageViewerForm: React.FC<Props> = ({
  file,
  password,
  onPasswordChange,
  onDecrypt,
  onChangeFile,
}) => (
  <div className={shared.container}>
    <p>{file.name}</p>
    <input
      type="password"
      placeholder="Password"
      value={password}
      onChange={(e) => onPasswordChange(e.target.value)}
      onKeyDown={(e) => e.key === "Enter" && onDecrypt()}
    />
    <div className={shared["button-group"]}>
      <button onClick={onDecrypt}>View</button>
      <button onClick={onChangeFile}>Change File</button>
    </div>
  </div>
);
