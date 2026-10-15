import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useSearchSession } from "@/lib/search-session";

import { favorites } from "@/api";
import PageHeader from "@/components/PageHeader";
import EmptyState from "@/components/EmptyState";
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
  const { channel, setChannel, query, setQuery, results, loading, error, run } = useSearchSession();

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
      <PageHeader title="搜索音乐" description="输入歌曲或歌手，选择平台开始搜索。切换页面后，输入和结果会保留。" />
      <div className="mb-5 flex items-center gap-3 rounded-2xl border bg-background/70 p-3">
        <Select
          value={channel}
          onValueChange={(value) => setChannel(value as OnlineSearchChannel)}
        >
          <SelectTrigger aria-label="搜索平台" className="h-10 w-[132px] border-0 bg-transparent shadow-none">
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
            aria-label="搜索歌曲或歌手"
            className="h-10 border-0 bg-transparent pl-9 text-sm shadow-none focus-visible:ring-1"
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void run();
              }
            }}
          />
        </div>
        <Button className="h-10 px-5" onClick={() => void run()} disabled={loading || !query.trim()}>
          {loading ? "搜索中" : "搜索"}
        </Button>
      </div>

      {error ? (
        <div className="mb-4 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      {!loading && results.length > 0 ? (
        <div className="mb-4 flex items-center justify-between text-xs text-muted-foreground">
          <span>找到 {results.length} 首歌曲</span><span>点击歌曲播放 · 星标收藏</span>
        </div>
      ) : null}
      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {loading ? (
          <div className="flex flex-col gap-1.5">
            {Array.from({ length: 8 }).map((_, index) => (
              <Skeleton key={index} className="h-9 rounded-[10px]" />
            ))}
          </div>
        ) : results.length === 0 && !error ? (
          <EmptyState title="想听什么，搜一下" description="试试歌名、歌手，或换一个音乐平台。搜索后可直接播放，也能收藏喜欢的歌曲。" />
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
