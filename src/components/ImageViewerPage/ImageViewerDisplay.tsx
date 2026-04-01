import classes from "./ImageViewerPage.module.css";
import shared from "../shared.module.css";

interface Props {
  objectUrl: string;
  filename: string;
  onClose: () => void;
  onChooseAnother: () => void;
}

export const ImageViewerDisplay: React.FC<Props> = ({
  objectUrl,
  filename,
  onClose,
  onChooseAnother,
}) => (
  <div className={shared.container}>
    <img className={classes.image} src={objectUrl} alt={filename} />
    <div className={shared.controls}>
      <div className={shared["button-group"]}>
        <button onClick={onClose}>Close Image</button>
        <button onClick={onChooseAnother}>Choose another file</button>
      </div>
    </div>
  </div>
);
