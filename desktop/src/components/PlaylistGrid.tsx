import { ArrowUpRightIcon, MusicNotesIcon, StarIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import EmptyState from "@/components/EmptyState";
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

export default function PlaylistGrid({ playlists, activeKey, onOpen, onToggleSave, isSaved, empty }: Props) {
  if (!playlists.length) return <EmptyState title="这里还没有歌单" description={empty ?? "点击歌单封面上的星标，就能在这里找到它。"} />;
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-5 gap-y-7">
      {playlists.map((playlist) => {
        const key = playlistKey(playlist);
        const saved = isSaved?.(playlist) ?? false;
        return (
          <article key={key} className="group relative min-w-0">
            <button
              type="button"
              onClick={() => onOpen(playlist)}
              aria-label={`打开歌单：${playlist.name}`}
              className={cn("w-full rounded-xl text-left outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-4 focus-visible:ring-offset-card", key === activeKey && "ring-2 ring-primary")}
            >
              <div className="relative aspect-square overflow-hidden rounded-[14px] bg-muted ring-1 ring-black/5 transition-all duration-300 group-hover:shadow-[0_18px_36px_-18px_rgb(16_24_40/45)] group-hover:ring-black/10">
                {playlist.artwork_uri ? <img src={playlist.artwork_uri} alt="" loading="lazy" className="size-full object-cover transition-transform duration-500 group-hover:scale-[1.05]" /> : (
                  <div className="flex size-full items-center justify-center text-muted-foreground"><MusicNotesIcon size={40} weight="duotone" /></div>
                )}
                <span className="cover-fade pointer-events-none absolute inset-0 opacity-70 transition-opacity duration-300 group-hover:opacity-100" />
                <span className="absolute right-3 bottom-3 flex size-9 translate-y-1 items-center justify-center rounded-full bg-primary text-primary-foreground opacity-0 shadow-lg transition-all duration-300 group-hover:translate-y-0 group-hover:opacity-100 group-focus-within:translate-y-0 group-focus-within:opacity-100"><ArrowUpRightIcon size={17} weight="bold" /></span>
                {playlist.track_count ? <span className="absolute bottom-3 left-3 rounded-md bg-black/25 px-1.5 py-0.5 text-[10px] font-medium text-white/95 backdrop-blur-sm">{playlist.track_count} 首</span> : null}
              </div>
              <div className="mt-3 line-clamp-2 min-h-10 text-[13px] leading-5 font-medium transition-colors group-hover:text-primary" title={playlist.name}>{playlist.name}</div>
              <p className="mt-1 truncate text-[11px] text-muted-foreground">{playlist.author || "精选歌单"}</p>
            </button>
            {onToggleSave ? (
              <Button
                variant="secondary"
                size="icon-sm"
                className={cn("absolute top-2.5 right-2.5 size-8 rounded-full bg-card/80 shadow-sm ring-1 ring-black/5 backdrop-blur-md transition-colors", saved ? "text-primary" : "text-muted-foreground hover:text-primary")}
                aria-pressed={saved}
                aria-label={saved ? `取消收藏：${playlist.name}` : `收藏歌单：${playlist.name}`}
                title={saved ? "取消收藏" : "收藏歌单"}
                onClick={() => onToggleSave(playlist)}
              ><StarIcon weight={saved ? "fill" : "regular"} /></Button>
            ) : null}
          </article>
        );
      })}
    </div>
  );
}
