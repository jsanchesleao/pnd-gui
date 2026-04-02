import { useRef, useState } from "react";
import "./App.css";
import { GenericPage } from "./components/GenericPage";
import { PreviewPage } from "./components/PreviewPage";
import { VaultPage } from "./components/VaultPage";

type Page = "main" | "preview" | "vault";

function App() {
  const [page, setPage] = useState<Page>("main");
  const [previewActive, setPreviewActive] = useState(false);
  const [vaultActive, setVaultActive] = useState(false);
  const vaultModifiedRef = useRef(false);

  const hideNav = (page === "preview" && previewActive) || (page === "vault" && vaultActive);

  function handleNavClick(target: Page) {
    if (page === "vault" && target !== "vault" && vaultModifiedRef.current) {
      if (!confirm("The vault has unsaved changes. Leave anyway?")) return;
    }
    setPage(target);
  }

  return (
    <div className="main-wrapper">
      <nav className={hideNav ? "nav nav--hidden-mobile" : "nav"}>
        <button
          className={page === "main" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => handleNavClick("main")}
        >
          Encrypt / Decrypt
        </button>
        <button
          className={page === "preview" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => handleNavClick("preview")}
        >
          Preview
        </button>
        <button
          className={page === "vault" ? "nav-btn nav-btn--active" : "nav-btn"}
          onClick={() => handleNavClick("vault")}
        >
          Vault
        </button>
      </nav>
      <main>
        {page === "main" && <GenericPage />}
        {page === "preview" && <PreviewPage onActiveChange={setPreviewActive} />}
        {page === "vault" && (
          <VaultPage
            onModifiedChange={(m) => { vaultModifiedRef.current = m; }}
            onActiveChange={setVaultActive}
          />
        )}
      </main>
    </div>
  );
}

export default App;
