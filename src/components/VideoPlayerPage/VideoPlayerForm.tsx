import shared from "../shared.module.css";

interface Props {
  file: File;
  password: string;
  onPasswordChange: (value: string) => void;
  onPlay: () => void;
  onChangeFile: () => void;
}

export const VideoPlayerForm: React.FC<Props> = ({
  file,
  password,
  onPasswordChange,
  onPlay,
  onChangeFile,
}) => (
  <div className={shared.container}>
    <p>{file.name}</p>
    <div className={shared.controls}>
      <input
        type="password"
        placeholder="Password"
        value={password}
        onChange={(e) => onPasswordChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onPlay()}
      />
      <div className={shared["button-group"]}>
        <button onClick={onPlay}>Play</button>
        <button onClick={onChangeFile}>Change File</button>
      </div>
    </div>
  </div>
);
