import {
  ChartLineUpIcon,
  GearSixIcon,
  HouseIcon,
  MagnifyingGlassIcon,
  MusicNotesIcon,
  PlugsConnectedIcon,
} from "@phosphor-icons/react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { useEffect, useState } from "react";

import { api } from "@/api";
import PlayerBar from "@/components/PlayerBar";
import TitleBar from "@/components/TitleBar";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import HomePage from "@/pages/HomePage";
import NowPlayingPage from "@/pages/NowPlayingPage";
import PlaylistsPage from "@/pages/PlaylistsPage";
import RankingsPage from "@/pages/RankingsPage";
import SearchPage from "@/pages/SearchPage";
import SettingsPage from "@/pages/SettingsPage";
import SourcesPage from "@/pages/SourcesPage";
import { PlayerProvider } from "@/player-context";
import { SettingsProvider } from "@/settings-context";
import type { AppInfo } from "@/types";

// 侧栏图标直接用仓库根 assets/ 里的应用图标，Vite 打包时会带进来。
import appIcon from "../../assets/icons/app_icon.png";

/// 侧栏导航：与旧 GPUI 客户端的 Tab 一一对应。
const NAV = [
  { key: "home", label: "主页", icon: HouseIcon },
  { key: "search", label: "搜索", icon: MagnifyingGlassIcon },
  { key: "rankings", label: "排行榜", icon: ChartLineUpIcon },
  { key: "playlists", label: "歌单", icon: MusicNotesIcon },
  { key: "sources", label: "音源", icon: PlugsConnectedIcon },
  { key: "settings", label: "设置", icon: GearSixIcon },
] as const;

type NavKey = (typeof NAV)[number]["key"];

function page(key: NavKey) {
  switch (key) {
    case "home":
      return <HomePage />;
    case "search":
      return <SearchPage />;
    case "rankings":
      return <RankingsPage />;
    case "playlists":
      return <PlaylistsPage />;
    case "sources":
      return <SourcesPage />;
    case "settings":
      return <SettingsPage />;
  }
}

function Shell() {
  const [tab, setTab] = useState<NavKey>("home");
  const [nowPlaying, setNowPlaying] = useState(false);
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => setInfo(null));
  }, []);

  const viewKey = nowPlaying ? "now-playing" : tab;

  return (
    <div className="grid h-full grid-rows-[36px_minmax(0,1fr)_84px]">
      <TitleBar />

      <div className="grid min-h-0 grid-cols-[212px_minmax(0,1fr)]">
        <aside className="flex min-h-0 flex-col gap-4 border-r bg-sidebar px-3 py-4">
          <div className="flex items-center gap-2.5 px-2">
            <img
              src={appIcon}
              alt=""
              className="size-8 rounded-[10px] object-cover ring-1 ring-border"
            />
            <div className="leading-tight">
              <div className="text-sm font-semibold">WCMusic</div>
              <div className="text-[11px] text-muted-foreground">
                {info ? `v${info.version}` : "连接中"}
              </div>
            </div>
          </div>

          <nav className="flex flex-col gap-0.5">
            {NAV.map((item) => {
              const active = item.key === tab && !nowPlaying;
              return (
                <button
                  key={item.key}
                  type="button"
                  onClick={() => {
                    setNowPlaying(false);
                    setTab(item.key);
                  }}
                  className={`relative flex items-center gap-3 rounded-[10px] px-3 py-2 text-sm transition-colors ${
                    active
                      ? "text-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {active ? (
                    <motion.span
                      layoutId="nav-pill"
                      className="absolute inset-0 rounded-[10px] bg-accent"
                      transition={{ type: "spring", stiffness: 520, damping: 38 }}
                    />
                  ) : null}
                  <item.icon
                    weight={active ? "fill" : "regular"}
                    className="relative z-10 size-[18px]"
                  />
                  <span className="relative z-10">{item.label}</span>
                </button>
              );
            })}
          </nav>
        </aside>

        <main className="relative min-h-0 overflow-hidden">
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={viewKey}
              className="h-full min-h-0"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
            >
              {nowPlaying ? (
                <NowPlayingPage onBack={() => setNowPlaying(false)} />
              ) : (
                page(tab)
              )}
            </motion.div>
          </AnimatePresence>
        </main>
      </div>

      <PlayerBar onOpenNowPlaying={() => setNowPlaying(true)} />
    </div>
  );
}

export default function App() {
  return (
    <MotionConfig reducedMotion="user">
      <SettingsProvider>
        <PlayerProvider>
          <TooltipProvider delayDuration={350}>
            <Shell />
            <Toaster position="bottom-right" />
          </TooltipProvider>
        </PlayerProvider>
      </SettingsProvider>
    </MotionConfig>
  );
}
