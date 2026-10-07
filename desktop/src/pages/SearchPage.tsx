import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { favorites, online } from "@/api";
import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { useSettings } from "@/settings-context";
import type { OnlineSearchChannel, Track } from "@/types";

const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

export default function SearchPage() {
  const { settings, replace } = useSettings();
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    const keyword = query.trim();
    if (!keyword || loading) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      setResults(await online.search(keyword, channel));
    } catch (problem) {
      setError(String(problem));
      setResults([]);
    } finally {
      setLoading(false);
    }
  };

  const savedTracks = settings?.saved_tracks ?? [];
  const isTrackSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === "我的收藏" &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );

  return (
    <div className="flex h-full min-h-0 flex-col px-7 py-6">
      <div className="mb-5 flex items-center gap-2">
        <Select
          value={channel}
          onValueChange={(value) => setChannel(value as OnlineSearchChannel)}
        >
          <SelectTrigger className="w-[132px]">
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
        <div className="relative flex-1">
          <MagnifyingGlassIcon className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            placeholder="搜索歌曲、歌手"
            className="pl-9"
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void run();
              }
            }}
          />
        </div>
        <Button onClick={() => void run()} disabled={loading}>
          {loading ? "搜索中" : "搜索"}
        </Button>
      </div>

      {error ? (
        <div className="mb-4 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {loading ? (
          <div className="flex flex-col gap-1.5">
            {Array.from({ length: 8 }).map((_, index) => (
              <Skeleton key={index} className="h-9 rounded-[10px]" />
            ))}
          </div>
        ) : (
          <TrackList
            tracks={results}
            onToggleSave={(track) => {
              void favorites.toggleTrack(track, "我的收藏").then(replace);
            }}
            saved={isTrackSaved}
            empty="输入关键字回车即可搜索，点结果行开始播放。"
          />
        )}
      </div>
    </div>
  );
}
