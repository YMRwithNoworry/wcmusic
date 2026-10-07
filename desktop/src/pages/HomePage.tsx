import { ArrowLeftIcon } from "@phosphor-icons/react";
import { useCallback, useEffect, useState } from "react";

import { favorites, online } from "@/api";
import PlaylistGrid, { playlistKey } from "@/components/PlaylistGrid";
import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
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
    <h2 className="text-[13px] font-semibold tracking-wide text-muted-foreground">
      {children}
    </h2>
  );
}

/// 「此刻」页：平台热门歌单 + 我的收藏。
export default function HomePage() {
  const { settings, replace } = useSettings();
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [hot, setHot] = useState<PlatformPlaylist[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState<{ name: string; tracks: Track[] } | null>(null);
  const [opening, setOpening] = useState(false);

  const loadHot = useCallback(async (next: OnlineSearchChannel) => {
    setLoading(true);
    setError(null);
    try {
      setHot(await online.playlists(next));
    } catch (problem) {
      setError(String(problem));
      setHot([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadHot(channel);
  }, [channel, loadHot]);

  const openPlaylist = async (playlist: PlatformPlaylist) => {
    setOpening(true);
    setError(null);
    try {
      setOpened({ name: playlist.name, tracks: await online.playlistTracks(playlist) });
    } catch (problem) {
      setError(String(problem));
    } finally {
      setOpening(false);
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

  return (
    <div className="h-full overflow-y-auto px-7 py-6">
      <header className="mb-6 flex items-end justify-between gap-6">
        <div>
          <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
            WCMusic / 每日发现
          </div>
          <h1 className="mt-1.5 text-[26px] leading-tight font-semibold">
            此刻，听见喜欢
          </h1>
        </div>
        <Select
          value={channel}
          onValueChange={(value) => setChannel(value as OnlineSearchChannel)}
        >
          <SelectTrigger className="w-[140px]">
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
      </header>

      {error ? (
        <div className="mb-5 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <section className="mb-8 flex flex-col gap-3">
        <SectionTitle>平台热门歌单</SectionTitle>
        {loading ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(142px,1fr))] gap-3">
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

      {opened ? (
        <section className="flex flex-col gap-3">
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="icon-sm" onClick={() => setOpened(null)}>
              <ArrowLeftIcon />
            </Button>
            <h2 className="text-sm font-semibold">{opened.name}</h2>
            <span className="text-xs text-muted-foreground">
              {opened.tracks.length} 首
            </span>
          </div>
          <TrackList
            tracks={opened.tracks}
            onToggleSave={(track) => {
              void favorites.toggleTrack(track, "我的收藏").then(replace);
            }}
            saved={isTrackSaved}
          />
        </section>
      ) : opening ? (
        <p className="text-sm text-muted-foreground">正在打开歌单…</p>
      ) : null}
    </div>
  );
}
