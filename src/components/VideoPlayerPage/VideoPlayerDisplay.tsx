import classes from "./VideoPlayerPage.module.css";
import shared from "../shared.module.css";

interface Props {
  objectUrl: string;
  onClose: () => void;
  onChooseAnother: () => void;
}

export const VideoPlayerDisplay: React.FC<Props> = ({
  objectUrl,
  onClose,
  onChooseAnother,
}) => (
  <div className={shared.container}>
    <div className={classes["video-wrapper"]}>
      <video className={classes.video} src={objectUrl} controls autoPlay />
    </div>
    <div className={shared["button-group"]}>
      <button onClick={onClose}>Close Video</button>
      <button onClick={onChooseAnother}>Choose another file</button>
    </div>
  </div>
);
