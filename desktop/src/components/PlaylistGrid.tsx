import { StarIcon } from "@phosphor-icons/react";
import { motion } from "motion/react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { PlatformPlaylist } from "@/types";

interface Props {
  playlists: PlatformPlaylist[];
  activeKey?: string | null;
  onOpen: (playlist: PlatformPlaylist) => void;
  onToggleSave?: (playlist: PlatformPlaylist) => void;
  isSaved?: (playlist: PlatformPlaylist) => boolean;
  empty?: string;
}

export function playlistKey(playlist: PlatformPlaylist): string {
  return `${playlist.channel}-${playlist.id}`;
}

/// 歌单卡片网格：热门歌单与我的收藏共用。
export default function PlaylistGrid({
  playlists,
  activeKey,
  onOpen,
  onToggleSave,
  isSaved,
  empty,
}: Props) {
  if (playlists.length === 0) {
    return (
      <div className="flex h-36 items-center justify-center rounded-xl border border-dashed text-sm text-muted-foreground">
        {empty ?? "还没有歌单。"}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(142px,1fr))] gap-3">
      {playlists.map((playlist, index) => {
        const key = playlistKey(playlist);
        const saved = isSaved?.(playlist) ?? false;
        return (
          <motion.button
            key={key}
            type="button"
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{
              duration: 0.24,
              delay: Math.min(index, 10) * 0.02,
              ease: [0.22, 1, 0.36, 1],
            }}
            whileHover={{ y: -2 }}
            whileTap={{ scale: 0.985 }}
            onClick={() => onOpen(playlist)}
            className={cn(
              "group relative flex flex-col gap-2 rounded-xl border bg-card p-2 text-left transition-colors",
              key === activeKey ? "border-primary/60" : "hover:border-primary/40",
            )}
          >
            <div className="aspect-square w-full overflow-hidden rounded-[10px] bg-muted">
              {playlist.artwork_uri ? (
                <img
                  src={playlist.artwork_uri}
                  alt=""
                  loading="lazy"
                  className="size-full object-cover transition-transform duration-300 group-hover:scale-[1.04]"
                />
              ) : (
                <div className="size-full bg-gradient-to-br from-muted to-accent" />
              )}
            </div>
            <div className="min-w-0">
              <div className="truncate text-[13px] font-medium" title={playlist.name}>
                {playlist.name}
              </div>
              <div className="truncate text-[11px] text-muted-foreground">
                {playlist.author || "未知作者"}
                {playlist.track_count ? ` · ${playlist.track_count} 首` : ""}
              </div>
            </div>
            {onToggleSave ? (
              <Button
                variant="secondary"
                size="icon-sm"
                className={cn(
                  "absolute top-3 right-3 size-7 rounded-full shadow-sm transition-opacity",
                  saved ? "text-primary opacity-100" : "opacity-0 group-hover:opacity-100",
                )}
                title={saved ? "取消收藏" : "收藏歌单"}
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleSave(playlist);
                }}
              >
                <StarIcon weight={saved ? "fill" : "regular"} />
              </Button>
            ) : null}
          </motion.button>
        );
      })}
    </div>
  );
}
