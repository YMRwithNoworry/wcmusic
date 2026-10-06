import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import LyricsWindow from "./LyricsWindow";
import { PlayerProvider } from "./player-context";
import { SettingsProvider } from "./settings-context";
import "./styles.css";

/// 桌面歌词是独立的置顶窗口：同一个页面，URL 带 `#lyrics`。
const isLyricsWindow = window.location.hash === "#lyrics";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isLyricsWindow ? (
      <SettingsProvider>
        <PlayerProvider>
          <LyricsWindow />
        </PlayerProvider>
      </SettingsProvider>
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
