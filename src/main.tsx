import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { DockWindow } from "./components/DockWindow";
import { PreviewWindow } from "./components/PreviewWindow";
import "./styles.css";

const params = new URLSearchParams(window.location.search);
const winType = params.get("window");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {winType === "preview" ? (
      <PreviewWindow />
    ) : winType === "dock" ? (
      <DockWindow />
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
