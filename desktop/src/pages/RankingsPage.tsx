import { useCallback, useEffect, useState } from "react";

import { favorites, online } from "@/api";
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
import type { OnlineSearchChannel, PlatformRanking, Track } from "@/types";

const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

/// 排行榜页：先选平台，再选榜单，最后出曲目。
export default function RankingsPage() {
  const { settings, replace } = useSettings();
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [rankings, setRankings] = useState<PlatformRanking[]>([]);
  const [active, setActive] = useState<PlatformRanking | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const all = await online.rankings();
      setRankings(all);
      setActive(all.find((item) => item.channel === channel) ?? null);
    } catch (problem) {
      setError(String(problem));
      setRankings([]);
      setActive(null);
    } finally {
      setLoading(false);
    }
  }, [channel]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!active) {
      setTracks([]);
      return;
    }
    let alive = true;
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
      <header className="mb-4 flex items-center justify-between gap-4">
        <h1 className="text-[22px] font-semibold">排行榜</h1>
        <Select
          value={channel}
          onValueChange={(value) => {
            setActive(null);
            setChannel(value as OnlineSearchChannel);
          }}
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
        <div className="mb-4 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <div className="mb-4 flex flex-wrap gap-1.5">
        {visible.map((ranking) => (
          <Button
            key={ranking.id}
            size="sm"
            variant={ranking.id === active?.id ? "default" : "outline"}
            onClick={() => setActive(ranking)}
          >
            {ranking.name}
          </Button>
        ))}
        {!loading && visible.length === 0 ? (
          <span className="text-sm text-muted-foreground">没有取到榜单。</span>
        ) : null}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {loading ? (
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
