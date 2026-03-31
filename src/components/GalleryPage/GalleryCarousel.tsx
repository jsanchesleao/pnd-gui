import { useEffect, useState } from "react";
import type { GalleryImage } from "./GalleryPage.types";
import classes from "./GalleryPage.module.css";
import shared from "../shared.module.css";

interface Props {
  images: GalleryImage[];
  onClose: () => void;
  onChooseAnother: () => void;
}

export const GalleryCarousel: React.FC<Props> = ({ images, onClose, onChooseAnother }) => {
  const [index, setIndex] = useState(0);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "ArrowLeft") setIndex((i) => Math.max(0, i - 1));
      else if (e.key === "ArrowRight") setIndex((i) => Math.min(images.length - 1, i + 1));
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [images.length]);

  const current = images[index];

  return (
    <div className={shared.container}>
      <div className={classes.carousel}>
        <div className={classes.imageWrapper}>
          <img className={classes.image} src={current.objectUrl} alt={current.name} />
        </div>
        <p className={classes.filename}>{current.name}</p>
        <div className={classes.carouselControls}>
          <button onClick={() => setIndex((i) => i - 1)} disabled={index === 0}>
            &#8249;
          </button>
          <span className={classes.counter}>
            {index + 1} / {images.length}
          </span>
          <button onClick={() => setIndex((i) => i + 1)} disabled={index === images.length - 1}>
            &#8250;
          </button>
        </div>
      </div>
      <div className={shared["button-group"]}>
        <button onClick={onClose}>Close</button>
        <button onClick={onChooseAnother}>Choose another file</button>
      </div>
    </div>
  );
};
