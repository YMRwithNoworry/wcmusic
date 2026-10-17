import { getCurrentWindow } from "@tauri-apps/api/window";
import "@fontsource-variable/outfit";
import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import LyricsWindow from "./LyricsWindow";
import { PlayerProvider } from "./player-context";
import { SettingsProvider } from "./settings-context";
import "./index.css";

// 播放器使用应用内操作，关闭 WebView 自带的浏览器右键菜单。
document.addEventListener("contextmenu", (event) => event.preventDefault());

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
