import { getCurrentWindow } from "@tauri-apps/api/window";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import Overlay from "./overlay/Overlay";
import "./index.css";

const isOverlay = getCurrentWindow().label === "overlay";
document.documentElement.classList.toggle("overlay", isOverlay);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isOverlay ? <Overlay /> : <App />}</React.StrictMode>,
);
