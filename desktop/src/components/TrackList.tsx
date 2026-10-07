import { StarIcon } from "@phosphor-icons/react";
import { motion } from "motion/react";
import { memo, useCallback, useRef } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { formatTime, usePlayer } from "@/player-context";
import type { Track } from "@/types";

interface Props {
  tracks: Track[];
  /// 传入时行尾显示收藏按钮（`saved` 决定是否已收藏）。
  onToggleSave?: (track: Track) => void;
  saved?: (track: Track) => boolean;
  empty?: string;
}

/// 正在播放的三条跳动竖条。
function PlayingBars() {
  return (
    <span className="flex h-3 items-end gap-[2px]">
      {[0, 1, 2].map((index) => (
        <motion.span
          key={index}
          className="w-[2px] rounded-full bg-primary"
          animate={{ height: ["35%", "100%", "50%"] }}
          transition={{
            duration: 1,
            repeat: Infinity,
            ease: "easeInOut",
            delay: index * 0.14,
          }}
        />
      ))}
    </span>
  );
}

interface RowProps {
  track: Track;
  index: number;
  playing: boolean;
  isSaved: boolean;
  showSave: boolean;
  onPlay: (index: number) => void;
  onToggleSave?: (track: Track) => void;
}

/// 单行曲目：用 memo 隔离，播放进度轮询不会让整张列表重渲染。
const TrackRow = memo(function TrackRow({
  track,
  index,
  playing,
  isSaved,
  showSave,
  onPlay,
  onToggleSave,
}: RowProps) {
  return (
    <motion.li
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        duration: 0.22,
        delay: Math.min(index, 12) * 0.015,
        ease: [0.22, 1, 0.36, 1],
      }}
      onClick={() => onPlay(index)}
      className={cn(
        "group grid cursor-pointer grid-cols-[24px_minmax(0,2.2fr)_minmax(0,1.1fr)_minmax(0,1fr)_64px_28px] items-center gap-3 rounded-[10px] px-2.5 py-2 text-sm transition-colors",
        playing ? "bg-accent/70" : "hover:bg-accent/45",
      )}
    >
      <span className="flex size-6 items-center justify-center">
        {playing ? (
          <PlayingBars />
        ) : (
          <span className="tabular text-[11px] text-muted-foreground opacity-70">
            {index + 1}
          </span>
        )}
      </span>
      <span className={cn("truncate font-medium", playing && "text-primary")}>
        {track.title}
      </span>
      <span className="truncate text-xs text-muted-foreground">{track.artist}</span>
      <span className="truncate text-xs text-muted-foreground">{track.album}</span>
      <span className="tabular text-right text-[11px] text-muted-foreground">
        {formatTime(track.durationMs)}
      </span>
      {showSave ? (
        <Button
          variant="ghost"
          size="icon-sm"
          className={cn(
            "size-7 transition-opacity",
            isSaved
              ? "text-primary opacity-100"
              : "text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
          )}
          title={isSaved ? "取消收藏" : "收藏到我的收藏"}
          onClick={(event) => {
            event.stopPropagation();
            onToggleSave?.(track);
          }}
        >
          <StarIcon weight={isSaved ? "fill" : "regular"} />
        </Button>
      ) : (
        <span />
      )}
    </motion.li>
  );
});

/// 曲目列表：点行播放，可选收藏按钮。搜索 / 榜单 / 歌单页共用。
export default function TrackList({ tracks, onToggleSave, saved, empty }: Props) {
  const { playAt, snapshot } = usePlayer();
  const playingId = snapshot?.current?.id ?? null;

  // 用 ref 拿到最新列表，`onPlay` 才能保持稳定，memo 的行才不会被无谓重渲染。
  const tracksRef = useRef(tracks);
  tracksRef.current = tracks;
  const onPlay = useCallback((index: number) => void playAt(tracksRef.current, index), [playAt]);

  if (tracks.length === 0) {
    return (
      <div className="flex h-40 items-center justify-center rounded-xl border border-dashed text-sm text-muted-foreground">
        {empty ?? "这里还没有歌曲。"}
      </div>
    );
  }

  return (
    <ul className="flex flex-col gap-0.5">
      {tracks.map((track, index) => (
        <TrackRow
          key={`${track.source}-${track.id}`}
          track={track}
          index={index}
          playing={track.id === playingId}
          isSaved={saved?.(track) ?? false}
          showSave={!!onToggleSave}
          onPlay={onPlay}
          onToggleSave={onToggleSave}
        />
      ))}
    </ul>
  );
}
