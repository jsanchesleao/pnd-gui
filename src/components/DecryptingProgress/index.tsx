import shared from "../shared.module.css";

interface Props {
  filename: string;
  progress: number;
}

export const DecryptingProgress: React.FC<Props> = ({ filename, progress }) => {
  return (
    <div className={shared.container}>
      <p>Decrypting {filename}…</p>
      <progress className={shared.progress} value={progress} max={100} />
    </div>
  );
};
