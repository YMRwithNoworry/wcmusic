import { ArrowClockwiseIcon, MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";

import { favorites, online } from "@/api";
import { Button } from "@/components/ui/button";
import PlaylistDetail from "@/components/PlaylistDetail";
import PlaylistGrid, { playlistKey } from "@/components/PlaylistGrid";
import { Skeleton } from "@/components/ui/skeleton";
import { favoriteFolderNames } from "@/lib/favorite-folders";
import { useSettings } from "@/settings-context";
import type { OnlineSearchChannel, PlatformPlaylist, Track } from "@/types";

export const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

gsap.registerPlugin(ScrollTrigger);

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="text-[18px] font-semibold leading-tight">{children}</h2>
  );
}

function channelShortName(label: string): string {
  return label.replace(/音乐$/, "");
}

function cachedPlaylists(channel: OnlineSearchChannel): PlatformPlaylist[] {
  try {
    const cached: unknown = JSON.parse(localStorage.getItem(`wcmusic-hot-${channel}`) ?? "[]");
    if (!Array.isArray(cached)) return [];
    return cached.filter((item): item is PlatformPlaylist =>
      item && item.channel === channel && typeof item.id === "string" && typeof item.name === "string",
    );
  } catch {
    return [];
  }
}

function nextPlaylistPage(channel: OnlineSearchChannel): number {
  try {
    const previous = Number(localStorage.getItem(`wcmusic-hot-page-${channel}`));
    return Number.isInteger(previous) && previous >= 1 && previous <= 5 ? previous % 5 + 1 : 1;
  } catch { return 1; }
}

/// 「此刻」页：平台热门歌单 + 我的收藏；点开歌单后切到详情视图。
export default function HomePage({ onSearch }: { onSearch?: () => void }) {
  const { settings, replace } = useSettings();
  const requestIdRef = useRef(0);
  const refreshingRef = useRef(false);
  const lastUpdateRef = useRef(0);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [hot, setHot] = useState<PlatformPlaylist[]>(() => cachedPlaylists("Kuwo"));
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /// 点开的歌单：`tracks` 为 `null` 表示还在载入。
  const [detail, setDetail] = useState<{ name: string; tracks: Track[] | null } | null>(null);
  const homeScrollRef = useRef<HTMLDivElement>(null);
  const shelfRef = useRef<HTMLElement>(null);
  const shelfTitleRef = useRef<HTMLDivElement>(null);

  const loadHot = useCallback(async (next: OnlineSearchChannel) => {
    const requestId = ++requestIdRef.current;
    refreshingRef.current = true;
    setUpdatedAt(null);
    setHot(cachedPlaylists(next));
    setLoading(true);
    setError(null);
    try {
      const page = nextPlaylistPage(next);
      const playlists = await online.playlists(next, page);
      if (requestId !== requestIdRef.current) return;
      if (playlists.length === 0) throw new Error("平台暂未返回歌单，请稍后重试");
      setHot(playlists);
      lastUpdateRef.current = Date.now();
      setUpdatedAt(lastUpdateRef.current);
      try {
        localStorage.setItem(`wcmusic-hot-page-${next}`, String(page));
        localStorage.setItem(`wcmusic-hot-${next}`, JSON.stringify(playlists));
      } catch {
        // 本地缓存不可用不影响实时歌单。
      }
    } catch (problem) {
      if (requestId !== requestIdRef.current) return;
      setError(String(problem));
      setHot((current) => current.length ? current : cachedPlaylists(next));
    } finally {
      if (requestId === requestIdRef.current) {
        refreshingRef.current = false;
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    void loadHot(channel);
    return () => { ++requestIdRef.current; };
  }, [channel, loadHot, settings?.use_network_proxy]);

  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "visible" && !refreshingRef.current
        && Date.now() - lastUpdateRef.current >= 5 * 60_000) {
        void loadHot(channel);
      }
    };
    const timer = window.setInterval(refresh, 60_000);
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [channel, loadHot]);

  useLayoutEffect(() => {
    const scroller = homeScrollRef.current;
    const shelf = shelfRef.current;
    if (!scroller || !shelf || hot.length === 0) return;

    let mediaQuery: ReturnType<typeof gsap.matchMedia> | null = null;
    const context = gsap.context(() => {
      mediaQuery = gsap.matchMedia();
      mediaQuery.add("(prefers-reduced-motion: no-preference)", () => {
        gsap.utils.toArray<HTMLElement>('[data-bento-card="true"]', shelf).forEach((card) => {
          gsap.fromTo(
            card,
            { opacity: 0.68, scale: 0.965 },
            {
              opacity: 1,
              scale: 1,
              ease: "none",
              scrollTrigger: {
                trigger: card,
                scroller,
                start: "top 92%",
                end: "center 62%",
                scrub: true,
              },
            },
          );
        });

        if (shelfTitleRef.current) {
          ScrollTrigger.create({
            trigger: shelf,
            scroller,
            pin: shelfTitleRef.current,
            start: "top 22px",
            end: "bottom bottom",
            pinSpacing: false,
          });
        }
      });
    }, shelf);

    return () => {
      mediaQuery?.revert();
      context.revert();
    };
  }, [channel, hot.length]);

  const openPlaylist = async (playlist: PlatformPlaylist) => {
    setDetail({ name: playlist.name, tracks: null });
    setError(null);
    try {
      setDetail({ name: playlist.name, tracks: await online.playlistTracks(playlist) });
    } catch (problem) {
      setError(String(problem));
      setDetail(null);
    }
  };

  const savedPlaylists = settings?.saved_playlists ?? [];
  const savedTracks = settings?.saved_tracks ?? [];
  const folders = favoriteFolderNames(settings);
  const isSaved = (playlist: PlatformPlaylist) =>
    savedPlaylists.some((item) => playlistKey(item.playlist) === playlistKey(playlist));
  const isTrackSaved = (track: Track, folder = "我的收藏") =>
    savedTracks.some(
      (item) =>
        item.folder === folder &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );
  const toggleTrack = (track: Track, folder: string) => {
    void favorites.toggleTrack(track, folder).then(replace);
  };

  if (detail) {
    return (
      <PlaylistDetail
        name={detail.name}
        tracks={detail.tracks}
        onBack={() => setDetail(null)}
        onToggleSave={toggleTrack}
        saved={isTrackSaved}
        folders={folders}
      />
    );
  }

  const featured = hot[0];
  const displayHot = hot.slice(0, 5);
  const remainingHot = hot.slice(5);

  return (
    <div ref={homeScrollRef} className="h-full overflow-y-auto px-8 py-7 [@media(max-width:1100px)]:px-6">
      <section className="mb-10 grid min-h-[330px] grid-cols-[minmax(0,1.12fr)_minmax(280px,.88fr)] gap-8 [@media(max-width:900px)]:grid-cols-1">
        <div className="flex min-h-[330px] flex-col justify-between border-b border-border pb-5">
          <div>
            <h1 className="max-w-[13ch] text-[clamp(2.6rem,5vw,5rem)] font-semibold leading-[0.98]">
              今天，听点
              <span className="mx-2 inline-flex size-[0.78em] translate-y-[0.07em] overflow-hidden rounded-[0.16em] bg-primary align-baseline text-primary-foreground">
                {featured?.artwork_uri ? <img src={featured.artwork_uri} alt="" className="size-full object-cover" /> : <MagnifyingGlassIcon className="m-auto size-1/2" />}
              </span>
              不一样的音乐
            </h1>
            <p className="mt-6 max-w-[38rem] text-[15px] leading-7 text-muted-foreground">
              从喜欢的平台发现歌单，也可以直接搜索歌曲或歌手。把今天的第一首歌交给正在更新的推荐。
            </p>
            <div className="mt-7 flex flex-wrap items-center gap-3">
              <Button size="lg" onClick={onSearch}><MagnifyingGlassIcon />搜索音乐</Button>
              <Button size="lg" variant="outline" disabled={loading} onClick={() => void loadHot(channel)}>
                <ArrowClockwiseIcon />换一批
              </Button>
            </div>
          </div>

          <div className="mt-8 overflow-hidden border-y border-border/70 py-3" aria-label="可用音乐平台">
            <div className="source-marquee-track flex w-max gap-8 whitespace-nowrap text-[12px] text-muted-foreground">
              {[...CHANNELS, ...CHANNELS].map((item, index) => (
                <span key={`${item.value}-${index}`} className="inline-flex items-center gap-2">
                  <span className="size-1.5 rounded-full bg-primary" aria-hidden="true" />
                  {item.label}
                </span>
              ))}
            </div>
          </div>
        </div>

        <button
          type="button"
          disabled={!featured}
          onClick={() => featured && void openPlaylist(featured)}
          className="group relative min-h-[330px] overflow-hidden rounded-[20px] bg-sidebar text-left text-sidebar-foreground outline-none transition-transform duration-500 hover:scale-[1.01] focus-visible:ring-2 focus-visible:ring-sidebar-ring disabled:cursor-default"
          aria-label={featured ? `打开歌单：${featured.name}` : "正在加载歌单"}
        >
          {featured?.artwork_uri ? (
            <img src={featured.artwork_uri} alt="" className="absolute inset-0 size-full object-cover opacity-85 transition-transform duration-700 group-hover:scale-105" />
          ) : null}
          <span className="absolute inset-0 bg-black/35" aria-hidden="true" />
          <span className="cover-fade absolute inset-0" aria-hidden="true" />
          <span className="absolute inset-x-6 bottom-6 block">
            <span className="mb-3 block text-[12px] font-medium text-white/70">正在发现</span>
            <span className="block max-w-[15ch] text-[clamp(1.5rem,3vw,2.5rem)] font-semibold leading-tight text-white">
              {featured?.name ? `${featured.name} · 正在发现` : (loading ? "正在加载歌单" : "暂时没有歌单")}
            </span>
            <span className="mt-2 block truncate text-[13px] text-white/70">{featured?.author || "选择一个平台开始发现"}</span>
          </span>
        </button>
      </section>

      <div className="mb-9">
        <div className="mb-3 flex items-end justify-between gap-4">
          <div>
            <SectionTitle>从哪个平台开始</SectionTitle>
            <p className="mt-1 text-[13px] text-muted-foreground">切换后会保留各平台的热门歌单页码。</p>
          </div>
          <span className="hidden text-[12px] text-muted-foreground [@media(min-width:760px)]:block">{CHANNELS.find((item) => item.value === channel)?.label}</span>
        </div>
        <div
          className="grid min-h-[66px] overflow-hidden rounded-[16px] border border-border bg-muted/55"
          style={{ gridTemplateColumns: CHANNELS.map((item) => channel === item.value ? "1.55fr" : "1fr").join(" ") }}
          role="group"
          aria-label="歌单平台"
        >
          {CHANNELS.map((item, index) => {
            const active = item.value === channel;
            return (
              <button
                key={item.value}
                type="button"
                aria-pressed={active}
                onClick={() => setChannel(item.value)}
                className={`relative min-w-0 border-r border-border px-4 py-3 text-left transition-colors last:border-r-0 focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-ring ${active ? "bg-card text-foreground" : "text-muted-foreground hover:bg-card/70 hover:text-foreground"}`}
              >
                <span className="block text-[11px] tabular text-muted-foreground/70">0{index + 1}</span>
                <span className="mt-1 block truncate text-[14px] font-medium">{active ? item.label : channelShortName(item.label)}</span>
                {active ? <span className="mt-1 block text-[11px] text-primary">热门歌单</span> : null}
              </button>
            );
          })}
        </div>
      </div>

      {error ? (
        <div className="mb-7 flex items-center justify-between gap-3 rounded-[12px] border border-destructive/30 bg-destructive/10 px-4 py-3 text-[13px] text-destructive">
          <span>{error}</span>
          <Button size="sm" variant="secondary" onClick={() => void loadHot(channel)}>重试</Button>
        </div>
      ) : null}

      <section ref={shelfRef} className="mb-12 grid grid-cols-[minmax(130px,.28fr)_minmax(0,1fr)] gap-8 [@media(max-width:900px)]:grid-cols-1">
        <div ref={shelfTitleRef} className="self-start pt-1">
          <SectionTitle>平台热门歌单</SectionTitle>
          <p className="mt-2 max-w-[15rem] text-[13px] leading-6 text-muted-foreground">把平台此刻最常被打开的歌单，整理成一张可以直接播放的唱片内页。</p>
          <span className="mt-5 block text-[12px] text-muted-foreground" role="status">
            {loading ? "正在获取最新歌单…" : updatedAt
              ? `更新于 ${new Date(updatedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
              : hot.length ? "暂显示上次歌单" : ""}
          </span>
        </div>
        <div className="min-w-0">
          {loading && hot.length === 0 ? (
            <div className="grid grid-cols-2 gap-3 [@media(min-width:760px)]:grid-cols-4">
              {Array.from({ length: 5 }).map((_, index) => (
                <Skeleton key={index} className="aspect-square rounded-[16px]" />
              ))}
            </div>
          ) : (
            <PlaylistGrid
              playlists={displayHot}
              layout="bento"
              onOpen={(playlist) => void openPlaylist(playlist)}
              onToggleSave={(playlist) => {
                void favorites.togglePlaylist(playlist).then(replace);
              }}
              isSaved={isSaved}
            />
          )}
          {remainingHot.length > 0 ? (
            <div className="mt-8">
              <SectionTitle>更多歌单</SectionTitle>
              <div className="mt-4">
                <PlaylistGrid
                  playlists={remainingHot}
                  onOpen={(playlist) => void openPlaylist(playlist)}
                  onToggleSave={(playlist) => {
                    void favorites.togglePlaylist(playlist).then(replace);
                  }}
                  isSaved={isSaved}
                />
              </div>
            </div>
          ) : null}
        </div>
      </section>

      <section className="mb-8">
        <SectionTitle>我的收藏</SectionTitle>
        <div className="mt-4">
          <PlaylistGrid
            playlists={savedPlaylists.map((item) => item.playlist)}
            onOpen={(playlist) => void openPlaylist(playlist)}
            onToggleSave={(playlist) => {
              void favorites.togglePlaylist(playlist).then(replace);
            }}
            isSaved={isSaved}
            empty="还没有收藏的歌单，把鼠标移到热门歌单卡片上点星标。"
          />
        </div>
      </section>
    </div>
  );
}
