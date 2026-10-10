import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { api } from "./api";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

api.bootstrap().then(
  (bootstrap) =>
    createRoot(root).render(
      <StrictMode>
        <App bootstrap={bootstrap} />
      </StrictMode>,
    ),
  (error: unknown) => {
    root.textContent = `navgator could not load its config: ${String(error)}`;
    root.className = "fatal";
  },
);
