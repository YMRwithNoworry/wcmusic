import { useEffect, useState } from "react";

import { api } from "./api";
import PlayerBar from "./components/PlayerBar";
import HomePage from "./pages/HomePage";
import PlaylistsPage from "./pages/PlaylistsPage";
import RankingsPage from "./pages/RankingsPage";
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

function page(tab: Tab) {
  switch (tab) {
    case "主页":
      return <HomePage />;
    case "搜索":
      return <SearchPage />;
    case "排行榜":
      return <RankingsPage />;
    case "歌单":
      return <PlaylistsPage />;
    case "设置":
      return <SettingsPage />;
    default:
      return <Placeholder tab={tab} />;
  }
}

function Shell() {
  const [tab, setTab] = useState<Tab>("主页");
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
      <main className="content">{page(tab)}</main>
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
