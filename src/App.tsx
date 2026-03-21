import { useState } from "react";
import "./App.css";
import { GenericPage } from "./components/GenericPage";
import { VideoPlayerPage } from "./components/VideoPlayerPage";
import { ImageViewerPage } from "./components/ImageViewerPage";

type Page = "main" | "video" | "image";

function App() {
  const [page, setPage] = useState<Page>("main");

  return (
    <div className="main-wrapper">
      <nav className="nav">
        <button
          className={page === "main" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => setPage("main")}
        >
          Encrypt / Decrypt
        </button>
        <button
          className={page === "video" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => setPage("video")}
        >
          Video Player
        </button>
        <button
          className={page === "image" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => setPage("image")}
        >
          Image Viewer
        </button>
      </nav>
      <main>
        {page === "main" && <GenericPage />}
        {page === "video" && <VideoPlayerPage />}
        {page === "image" && <ImageViewerPage />}
      </main>
    </div>
  );
}

export default App;
