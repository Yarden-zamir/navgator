import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api } from "./api";
import type { Bootstrap } from "./api/Bootstrap";
import { App } from "./App";
import { SettingsPage, openSettings } from "./SettingsPage";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

function onSettingsPage(): boolean {
  return window.location.hash.startsWith("#settings");
}

/// The navigator. It loads the config on every mount, so it shows settings
/// changes after the settings page closes.
function Navigator() {
  const [bootstrap, setBootstrap] = useState<Bootstrap | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.bootstrap().then(setBootstrap, (failure: unknown) => setError(String(failure)));
  }, []);
  if (error !== null) {
    return (
      <div className="fatal">
        <p>navgator could not load its config: {error}</p>
        <button type="button" className="small-button" onClick={openSettings}>
          Open settings
        </button>
      </div>
    );
  }
  return bootstrap && <App bootstrap={bootstrap} />;
}

function Root() {
  const [settings, setSettings] = useState(onSettingsPage);
  useEffect(() => {
    const listener = () => setSettings(onSettingsPage());
    window.addEventListener("hashchange", listener);
    return () => window.removeEventListener("hashchange", listener);
  }, []);
  return settings ? <SettingsPage onClose={() => (window.location.hash = "")} /> : <Navigator />;
}

createRoot(root).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
