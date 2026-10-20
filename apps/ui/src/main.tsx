/**
 * The MPCraft frontend entry point.
 *
 * The frontend knows nothing about MAVLink: it renders typed payloads that the Rust backend
 * pushes over Tauri IPC. See `src/lib/ipc/` for the contract.
 */

import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "./app/App";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("missing #root element");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
