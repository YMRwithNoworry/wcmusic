import { ArrowClockwiseIcon, MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { favorites, online } from "@/api";
import PageHeader from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import PlaylistDetail from "@/components/PlaylistDetail";
import PlaylistGrid, { playlistKey } from "@/components/PlaylistGrid";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { useSettings } from "@/settings-context";
import type { OnlineSearchChannel, PlatformPlaylist, Track } from "@/types";

export const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="flex items-center gap-2 text-[16px] font-semibold tracking-tight">
      <span aria-hidden="true" className="h-4 w-[3px] rounded-full bg-gradient-to-b from-primary to-primary/25" />
      {children}
    </h2>
  );
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
  const isSaved = (playlist: PlatformPlaylist) =>
    savedPlaylists.some((item) => playlistKey(item.playlist) === playlistKey(playlist));
  const isTrackSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === "我的收藏" &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );
  const toggleTrack = (track: Track) => {
    void favorites.toggleTrack(track, "我的收藏").then(replace);
  };

  if (detail) {
    return (
      <PlaylistDetail
        name={detail.name}
        tracks={detail.tracks}
        onBack={() => setDetail(null)}
        onToggleSave={toggleTrack}
        saved={isTrackSaved}
      />
    );
  }

  return (
    <div className="h-full overflow-y-auto px-7 py-6">
      <PageHeader eyebrow="发现" title="今天，听点不一样的" description="从喜欢的平台发现歌单，也可以直接搜索歌曲或歌手。">
        <Button variant="outline" onClick={onSearch}><MagnifyingGlassIcon />搜索音乐</Button>
      </PageHeader>
      <div className="mb-7 flex flex-wrap items-center justify-between gap-3 border-b pb-5">
        <p className="text-[13px] text-muted-foreground">选择平台，发现此刻值得听的音乐</p>
        <Select
          value={channel}
          onValueChange={(value) => setChannel(value as OnlineSearchChannel)}
        >
          <SelectTrigger aria-label="歌单平台" className="w-[140px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {CHANNELS.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {item.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {error ? (
        <div className="mb-5 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
          <Button className="ml-3" size="sm" variant="secondary" onClick={() => void loadHot(channel)}>重试</Button>
        </div>
      ) : null}

      <section className="mb-8 flex flex-col gap-3">
        <div className="flex items-center justify-between gap-3">
          <SectionTitle>平台热门歌单</SectionTitle>
          <div className="flex items-center gap-3">
            <span className="text-[11px] text-muted-foreground" role="status">
              {loading ? "正在获取最新歌单…" : updatedAt
                ? `更新于 ${new Date(updatedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
                : hot.length ? "暂显示上次歌单" : ""}
            </span>
            <Button size="sm" variant="secondary" disabled={loading} onClick={() => void loadHot(channel)}>
              <ArrowClockwiseIcon />换一批
            </Button>
          </div>
        </div>
        {loading && hot.length === 0 ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-5 gap-y-7">
            {Array.from({ length: 6 }).map((_, index) => (
              <Skeleton key={index} className="h-[212px] rounded-xl" />
            ))}
          </div>
        ) : (
          <PlaylistGrid
            playlists={hot}
            onOpen={(playlist) => void openPlaylist(playlist)}
            onToggleSave={(playlist) => {
              void favorites.togglePlaylist(playlist).then(replace);
            }}
            isSaved={isSaved}
          />
        )}
      </section>

      <section className="mb-8 flex flex-col gap-3">
        <SectionTitle>我的收藏</SectionTitle>
        <PlaylistGrid
          playlists={savedPlaylists.map((item) => item.playlist)}
          onOpen={(playlist) => void openPlaylist(playlist)}
          onToggleSave={(playlist) => {
            void favorites.togglePlaylist(playlist).then(replace);
          }}
          isSaved={isSaved}
          empty="还没有收藏的歌单，把鼠标移到热门歌单卡片上点星标。"
        />
      </section>
    </div>
  );
}
