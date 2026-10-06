import { useEffect, useState } from "react";

import { api } from "./api";
import PlayerBar from "./components/PlayerBar";
import SearchPage from "./pages/SearchPage";
import SettingsPage from "./pages/SettingsPage";
import { PlayerProvider } from "./player-context";
import { SettingsProvider } from "./settings-context";
import type { AppInfo } from "./types";

/// 左侧导航的六个页面，与旧 GPUI 客户端的 Tab 一一对应。
const TABS = ["主页", "搜索", "排行榜", "歌单", "音源", "设置"] as const;
type Tab = (typeof TABS)[number];

function Placeholder({ tab }: { tab: Tab }) {
  return (
    <>
      <h1>{tab}</h1>
      <p className="hint">该页面正在迁移中。</p>
    </>
  );
}

function Shell() {
  const [tab, setTab] = useState<Tab>("搜索");
  const [info, setInfo] = useState<AppInfo | null>(null);

  // 启动时拉一次应用信息，同时验证前后端 IPC 是否打通。
  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => setInfo(null));
  }, []);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">WCMusic</div>
        <nav>
          {TABS.map((name) => (
            <button
              key={name}
              className={name === tab ? "tab active" : "tab"}
              onClick={() => setTab(name)}
            >
              {name}
            </button>
          ))}
        </nav>
        <div className="version">
          {info ? `${info.name} ${info.version}` : "连接后端中…"}
        </div>
      </aside>
      <main className="content">
        {tab === "搜索" ? (
          <SearchPage />
        ) : tab === "设置" ? (
          <SettingsPage />
        ) : (
          <Placeholder tab={tab} />
        )}
      </main>
      <PlayerBar />
    </div>
  );
}

export default function App() {
  return (
    <SettingsProvider>
      <PlayerProvider>
        <Shell />
      </PlayerProvider>
    </SettingsProvider>
  );
}
