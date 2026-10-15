import { getCurrentWindow } from "@tauri-apps/api/window";
import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import LyricsWindow from "./LyricsWindow";
import { PlayerProvider } from "./player-context";
import { SettingsProvider } from "./settings-context";
import "./index.css";

const isLyricsWindow = getCurrentWindow().label === "lyrics";
if (isLyricsWindow) document.documentElement.classList.add("lyrics-mode");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isLyricsWindow ? (
      <SettingsProvider>
        <PlayerProvider controls={false}>
          <LyricsWindow />
        </PlayerProvider>
      </SettingsProvider>
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
