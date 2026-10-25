import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./app/App";
import { QuickApp } from "./features/quick/QuickApp";
import "./app/global.css";

// Two windows load this page: the main one, and the quick box (M8 §3).
const isQuick = (() => {
  try {
    return getCurrentWindow().label === "quick";
  } catch {
    return false;
  }
})();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isQuick ? <QuickApp /> : <App />}</React.StrictMode>,
);
