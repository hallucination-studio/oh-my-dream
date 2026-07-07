import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App.tsx";
import "@xyflow/react/dist/style.css";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  // Fail loud: a missing mount point is a build/HTML error, not something to
  // silently swallow.
  throw new Error("Root element #root not found in index.html");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
