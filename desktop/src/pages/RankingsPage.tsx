import { PlayIcon } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { favorites, online } from "@/api";
import PageHeader from "@/components/PageHeader";
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
import { usePlayer } from "@/player-context";
import { useSettings } from "@/settings-context";
import type { OnlineSearchChannel, PlatformRanking, Track } from "@/types";

const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

/// 排行榜页：先选平台，再选榜单，最后出曲目。
export default function RankingsPage() {
  const { playAt } = usePlayer();
  const { settings, replace } = useSettings();
  const requestIdRef = useRef(0);
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [rankings, setRankings] = useState<PlatformRanking[]>([]);
  const [active, setActive] = useState<PlatformRanking | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [directoryLoading, setDirectoryLoading] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setDirectoryLoading(true);
    setError(null);
    try {
      const all = await online.rankings();
      if (requestId !== requestIdRef.current) return;
      setRankings(all);
    } catch (problem) {
      if (requestId !== requestIdRef.current) return;
      setError(String(problem));
      setRankings([]);
      setActive(null);
    } finally {
      if (requestId === requestIdRef.current) {
        setDirectoryLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    void load();
    return () => { ++requestIdRef.current; };
  }, [load, settings?.use_network_proxy]);

  useEffect(() => {
    setActive(rankings.find((item) => item.channel === channel) ?? null);
  }, [channel, rankings]);

  useEffect(() => {
    if (!active) {
      setTracks([]);
      setLoading(false);
      return;
    }
    let alive = true;
    setTracks([]);
    setError(null);
    setLoading(true);
    online
      .rankingTracks(active)
      .then((next) => {
        if (alive) {
          setTracks(next);
        }
      })
      .catch((problem) => {
        if (alive) {
          setError(String(problem));
          setTracks([]);
        }
      })
      .finally(() => {
        if (alive) {
          setLoading(false);
        }
      });
    return () => {
      alive = false;
    };
  }, [active]);

  const savedTracks = settings?.saved_tracks ?? [];
  const isTrackSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === "我的收藏" &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );

  const visible = rankings.filter((item) => item.channel === channel);

  return (
    <div className="flex h-full min-h-0 flex-col px-7 py-6">
      <PageHeader eyebrow="榜单" title="排行榜" description="先选音乐平台，再挑一个榜单，听听大家最近在循环什么。">
        <Select
          value={channel}
          onValueChange={(value) => {
            setActive(null);
            setChannel(value as OnlineSearchChannel);
          }}
        >
          <SelectTrigger aria-label="排行榜平台" className="w-[140px]">
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
      </PageHeader>

      {error ? (
        <div className="mb-4 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
          <Button className="ml-3" variant="secondary" size="sm" onClick={() => void load()}>重试</Button>
        </div>
      ) : null}

      <div aria-label="选择榜单" className="mb-5 flex shrink-0 gap-2 overflow-x-auto pb-3">
        {visible.map((ranking) => (
          <Button
            key={`${ranking.channel}-${ranking.id}`}
            size="sm"
            variant={ranking.id === active?.id ? "default" : "outline"}
            className="shrink-0 rounded-full px-4"
            aria-pressed={ranking.id === active?.id}
            onClick={() => setActive(ranking)}
          >
            {ranking.name}
          </Button>
        ))}
        {!directoryLoading && visible.length === 0 ? (
          <span className="text-sm text-muted-foreground">没有取到榜单。</span>
        ) : null}
      </div>

      {active ? <div className="mb-5 flex shrink-0 items-center justify-between gap-3 rounded-xl border border-border/60 bg-muted/40 px-4 py-3">
        <div><h2 className="text-base font-semibold">{active.name}</h2><p className="mt-1 text-xs text-muted-foreground">{loading ? "正在加载歌曲…" : `${tracks.length} 首歌曲`}</p></div>
        <Button disabled={loading || !tracks.length} onClick={() => void playAt(tracks, 0)}><PlayIcon weight="fill" />播放全部</Button>
      </div> : null}
      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {directoryLoading || loading ? (
          <div className="flex flex-col gap-1.5">
            {Array.from({ length: 10 }).map((_, index) => (
              <Skeleton key={index} className="h-9 rounded-[10px]" />
            ))}
          </div>
        ) : (
          <TrackList
            tracks={tracks}
            onToggleSave={(track) => {
              void favorites.toggleTrack(track, "我的收藏").then(replace);
            }}
            saved={isTrackSaved}
            empty="选一个榜单看看。"
          />
        )}
      </div>
    </div>
  );
}
