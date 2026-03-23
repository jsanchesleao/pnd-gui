import { useState } from "react";
import "./App.css";
import { GenericPage } from "./components/GenericPage";
import { PreviewPage } from "./components/PreviewPage";

type Page = "main" | "preview";

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
          className={page === "preview" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => setPage("preview")}
        >
          Preview
        </button>
      </nav>
      <main>
        {page === "main" && <GenericPage />}
        {page === "preview" && <PreviewPage />}
      </main>
    </div>
  );
}

export default App;
