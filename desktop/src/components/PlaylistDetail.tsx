import { ArrowLeftIcon, PlayIcon } from "@phosphor-icons/react";

import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { usePlayer } from "@/player-context";
import type { Track } from "@/types";

interface Props {
  name: string;
  tracks: Track[] | null;
  onBack: () => void;
  onToggleSave?: (track: Track, folder: string) => void;
  saved?: (track: Track, folder?: string) => boolean;
  folders?: string[];
  defaultFolder?: string;
  onCreateFolder?: () => void;
}

/// 歌单详情：点开卡片后接管整块内容区，带返回按钮。
///
/// 之前是把详情塞在长列表最底部，点卡片后画面没变化，看起来像「点不动」。
export default function PlaylistDetail({
  name,
  tracks,
  onBack,
  onToggleSave,
  saved,
  folders,
  defaultFolder,
  onCreateFolder,
}: Props) {
  const { playAt } = usePlayer();
  return (
    <div className="flex h-full min-h-0 flex-col px-7 py-6">
      <header className="mb-6 flex items-center gap-3">
        <Button variant="ghost" size="icon-sm" onClick={onBack} title="返回">
          <ArrowLeftIcon />
        </Button>
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-[24px] font-semibold tracking-tight">{name}</h1>
          <p className="text-xs text-muted-foreground">
            {tracks ? `${tracks.length} 首` : "正在载入…"}
          </p>
        </div>
        <Button disabled={!tracks?.length} onClick={() => { if (tracks?.length) void playAt(tracks, 0); }}><PlayIcon weight="fill" />播放全部</Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {tracks ? (
          <TrackList
            tracks={tracks}
            onToggleSave={onToggleSave}
            saved={saved}
            folders={folders}
            defaultFolder={defaultFolder}
            onCreateFolder={onCreateFolder}
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
