import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./app/App";
import { IslandApp } from "./features/island/IslandApp";
import { PresencePanel } from "./features/island/PresencePanel";
import { QuickApp } from "./features/quick/QuickApp";
import "./app/global.css";

// Four windows load this page: the main one, the quick box (M8 §3), the island pill and the wake
// panel (M14).
const label = (() => {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
})();

const app =
  label === "quick"
    ? QuickApp
    : label === "island"
      ? IslandApp
      : label === "presence-panel"
        ? PresencePanel
        : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{React.createElement(app)}</React.StrictMode>,
);
