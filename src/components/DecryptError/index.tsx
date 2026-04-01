import shared from "../shared.module.css";

interface Props {
  message: string;
  onTryAgain: () => void;
  onChangeFile: () => void;
}

export const DecryptError: React.FC<Props> = ({
  message,
  onTryAgain,
  onChangeFile,
}) => {
  return (
    <div className={shared.container}>
      <p className={shared.text} data-text-type="failure">
        Error: {message}
      </p>
      <div className={shared.controls}>
        <button onClick={onTryAgain}>Try again</button>
        <button onClick={onChangeFile}>Change File</button>
      </div>
    </div>
  );
};
