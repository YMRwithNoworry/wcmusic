import { ArrowLeftIcon } from "@phosphor-icons/react";

import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { Track } from "@/types";

interface Props {
  name: string;
  tracks: Track[] | null;
  onBack: () => void;
  onToggleSave?: (track: Track) => void;
  saved?: (track: Track) => boolean;
}

/// 歌单详情：点开卡片后接管整块内容区，带返回按钮。
///
/// 之前是把详情塞在长列表最底部，点卡片后画面没变化，看起来像「点不动」。
export default function PlaylistDetail({ name, tracks, onBack, onToggleSave, saved }: Props) {
  return (
    <div className="flex h-full min-h-0 flex-col px-7 py-6">
      <header className="mb-4 flex items-center gap-3">
        <Button variant="ghost" size="icon-sm" onClick={onBack} title="返回">
          <ArrowLeftIcon />
        </Button>
        <div className="min-w-0">
          <h1 className="truncate text-[20px] font-semibold">{name}</h1>
          <p className="text-xs text-muted-foreground">
            {tracks ? `${tracks.length} 首` : "正在载入…"}
          </p>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {tracks ? (
          <TrackList
            tracks={tracks}
            onToggleSave={onToggleSave}
            saved={saved}
            empty="这个歌单里还没有歌曲。"
          />
        ) : (
          <div className="flex flex-col gap-1.5">
            {Array.from({ length: 10 }).map((_, index) => (
              <Skeleton key={index} className="h-9 rounded-[10px]" />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
