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
import { PlayerProvider, usePlayer } from "@/player-context";
import { SettingsProvider } from "@/settings-context";
import type { AppInfo } from "@/types";

// 侧栏图标直接用仓库根 assets/ 里的应用图标，Vite 打包时会带进来。
import appIcon from "../../assets/icons/app_icon.png";

/// 侧栏导航：对应当前 Tauri 客户端的页面。
const NAV = [
  { key: "home", label: "发现音乐", icon: HouseIcon },
  { key: "search", label: "搜索音乐", icon: MagnifyingGlassIcon },
  { key: "rankings", label: "排行榜", icon: ChartLineUpIcon },
  { key: "playlists", label: "我的收藏", icon: MusicNotesIcon },
  { key: "sources", label: "音源管理", icon: PlugsConnectedIcon },
  { key: "settings", label: "设置", icon: GearSixIcon },
] as const;

type NavKey = (typeof NAV)[number]["key"];

function NowPlayingNav({ active, onOpen }: { active: boolean; onOpen: () => void }) {
  const { snapshot } = usePlayer();
  return (
    <button
      type="button"
      disabled={!snapshot?.current}
      onClick={onOpen}
      className={`mt-3 flex min-h-11 items-center gap-3 rounded-xl px-3 text-left text-[13px] transition-colors focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40 ${active ? "bg-accent text-primary" : "text-muted-foreground hover:bg-card hover:text-foreground"}`}
    >
      <MusicNotesIcon size={18} />
      正在播放
    </button>
  );
}

function page(key: NavKey, navigate: (key: NavKey) => void) {
  switch (key) {
    case "home":
      return <HomePage onSearch={() => navigate("search")} />;
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
    <div className="grid h-full grid-rows-[36px_minmax(0,1fr)_100px]">
      <TitleBar />

      <div className="grid min-h-0 grid-cols-[184px_minmax(0,1fr)] gap-2 pr-3 pb-3">
        <aside className="flex min-h-0 flex-col px-3 pt-4 pb-2 [@media(max-height:680px)]:pt-2">
          <div className="mb-8 flex items-center gap-2.5 px-2 [@media(max-height:680px)]:mb-4">
            <img
              src={appIcon}
              alt=""
              className="size-8 rounded-[10px] object-cover ring-1 ring-border"
            />
            <div className="leading-tight">
              <div className="text-[15px] font-semibold tracking-tight">WCMusic</div>
              <div className="mt-1 text-[11px] text-muted-foreground">随时听见喜欢</div>
            </div>
          </div>

          <nav aria-label="主导航" className="flex flex-1 flex-col gap-1 overflow-y-auto">
            {NAV.map((item, index) => {
              const active = item.key === tab && !nowPlaying;
              return (
                <button
                  key={item.key}
                  type="button"
                  onClick={() => {
                    setNowPlaying(false);
                    setTab(item.key);
                  }}
                  aria-current={active ? "page" : undefined}
                  className={`relative flex min-h-11 shrink-0 items-center gap-3 rounded-xl px-3 py-2.5 text-[13px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring [@media(max-height:680px)]:min-h-10 [@media(max-height:680px)]:py-2 ${index === 3 || index === 4 ? "mt-5 [@media(max-height:680px)]:mt-3" : ""} ${
                    active
                      ? "font-semibold text-primary"
                      : "text-muted-foreground hover:bg-card hover:text-foreground"
                  }`}
                >
                  {active ? (
                    <motion.span
                      layoutId="nav-pill"
                      className="absolute inset-0 rounded-xl bg-accent"
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
          <NowPlayingNav active={nowPlaying} onOpen={() => setNowPlaying(true)} />
          <div className="mt-4 px-3 text-[10px] text-muted-foreground/75 [@media(max-height:680px)]:mt-2">{info ? `WCMusic ${info.version}` : "正在连接播放器…"}</div>
        </aside>

        <main className="relative min-h-0 min-w-0 overflow-hidden rounded-[20px] border border-border/60 bg-card">
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
                page(tab, setTab)
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
