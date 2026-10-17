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

/// 侧栏导航：按用途分组，比一排等距按钮更好扫读。
const NAV = [
  { key: "home", label: "发现音乐", icon: HouseIcon },
  { key: "search", label: "搜索音乐", icon: MagnifyingGlassIcon },
  { key: "rankings", label: "排行榜", icon: ChartLineUpIcon },
  { key: "playlists", label: "我的收藏", icon: MusicNotesIcon },
  { key: "sources", label: "音源管理", icon: PlugsConnectedIcon },
  { key: "settings", label: "设置", icon: GearSixIcon },
] as const;

const NAV_GROUPS = [
  { label: "浏览", items: ["home", "search", "rankings"] },
  { label: "资料库", items: ["playlists", "sources"] },
  { label: "系统", items: ["settings"] },
] as const satisfies readonly { label: string; items: readonly NavKey[] }[];

type NavKey = (typeof NAV)[number]["key"];

function NowPlayingNav({ active, onOpen }: { active: boolean; onOpen: () => void }) {
  const { snapshot, artwork } = usePlayer();
  const track = snapshot?.current;
  return (
    <button
      type="button"
      disabled={!track}
      onClick={onOpen}
      aria-label={track ? `正在播放：${track.title}` : "正在播放"}
      className={`mt-2 flex items-center gap-2.5 rounded-xl p-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-sidebar-ring disabled:opacity-45 ${active ? "bg-sidebar-accent" : "hover:bg-sidebar-accent/70"}`}
    >
      <span className="relative size-9 shrink-0 overflow-hidden rounded-[9px] bg-muted ring-1 ring-black/10">
        {artwork ? (
          <img src={artwork} alt="" className="size-full object-cover" />
        ) : (
          <span className="flex size-full items-center justify-center text-muted-foreground">
            <MusicNotesIcon size={16} weight="duotone" />
          </span>
        )}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-[12px] text-sidebar-foreground/60">
          正在播放
        </span>
        <span className="mt-0.5 block truncate text-[13px] font-medium">
          {track?.title ?? "尚未播放"}
        </span>
      </span>
      {snapshot?.playing ? (
        <span className="mr-1 flex h-3 items-end gap-[2px]" aria-hidden="true">
          {[0, 1, 2].map((index) => (
            <span
              key={index}
              className="player-bar-pulse w-[2px] rounded-full bg-primary"
              style={{ animationDelay: `${index * 0.14}s` }}
            />
          ))}
        </span>
      ) : null}
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
    <div className="grid h-full grid-rows-[38px_minmax(0,1fr)_92px] bg-background">
      <TitleBar />

      <div className="grid min-h-0 grid-cols-[228px_minmax(0,1fr)] gap-0">
        <aside className="flex min-h-0 flex-col bg-sidebar px-5 pt-4 pb-3 text-sidebar-foreground">
          <div className="mb-8 flex items-center gap-3 px-1 [@media(max-height:680px)]:mb-4">
            <span className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-[12px]">
              <img
                src={appIcon}
                alt=""
                className="size-full object-cover"
              />
            </span>
            <div className="min-w-0 leading-tight">
              <div className="text-[17px] font-semibold tracking-[0.01em]">WCMusic</div>
              <div className="mt-1 text-[12px] text-sidebar-foreground/60">随时听见喜欢</div>
            </div>
          </div>

          <nav aria-label="主导航" className="no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto">
            {NAV_GROUPS.map((group, groupIndex) => (
              <div key={group.label} className={groupIndex > 0 ? "mt-3 [@media(max-height:680px)]:mt-2" : undefined}>
                <div className="px-3 pb-1.5 text-[12px] font-medium text-sidebar-foreground/45 [@media(max-height:680px)]:hidden">
                  {group.label}
                </div>
                <div className="flex flex-col gap-0.5">
                  {group.items.map((key) => {
                    const item = NAV.find((entry) => entry.key === key)!;
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
                        className={`relative flex min-h-10 shrink-0 items-center gap-3 rounded-xl px-3 py-2 text-[13px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-sidebar-ring [@media(max-height:680px)]:min-h-9 [@media(max-height:680px)]:py-1.5 ${
                          active
                            ? "font-semibold text-sidebar-primary"
                            : "text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-foreground"
                        }`}
                      >
                        {active ? (
                          <motion.span
                            layoutId="nav-pill"
                            className="nav-pill absolute inset-0 rounded-xl"
                            transition={{ type: "spring", stiffness: 520, damping: 38 }}
                          />
                        ) : null}
                        <item.icon
                          weight={active ? "fill" : "regular"}
                          className="relative z-10 size-[17px]"
                        />
                        <span className="relative z-10">{item.label}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </nav>
          <NowPlayingNav active={nowPlaying} onOpen={() => setNowPlaying(true)} />
          <div className="mt-3 px-2 text-[12px] text-sidebar-foreground/40">{info ? `WCMusic ${info.version}` : "正在连接播放器…"}</div>
        </aside>

        <main className="relative min-h-0 min-w-0 max-w-full overflow-x-hidden bg-background">
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
