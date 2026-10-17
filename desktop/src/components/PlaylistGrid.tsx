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
  layout?: "grid" | "bento";
}

export function playlistKey(playlist: PlatformPlaylist): string {
  return `${playlist.channel}-${playlist.id}`;
}

export default function PlaylistGrid({ playlists, activeKey, onOpen, onToggleSave, isSaved, empty, layout = "grid" }: Props) {
  if (!playlists.length) return <EmptyState title="这里还没有歌单" description={empty ?? "点击歌单封面上的星标，就能在这里找到它。"} />;
  const bento = layout === "bento" && playlists.length >= 5;
  return (
    <div className={cn(
      bento
        ? "grid grid-flow-dense grid-cols-12 auto-rows-[minmax(156px,1fr)] gap-3"
        : "grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-x-5 gap-y-7",
    )}>
      {playlists.map((playlist, index) => {
        const key = playlistKey(playlist);
        const saved = isSaved?.(playlist) ?? false;
        return (
          <article
            key={key}
            data-bento-card={bento ? "true" : undefined}
            className={cn(
              "group relative min-w-0",
              bento && (index === 0 ? "col-span-6 row-span-2" : "col-span-3 row-span-1"),
            )}
          >
            <button
              type="button"
              onClick={() => onOpen(playlist)}
              aria-label={`打开歌单：${playlist.name}`}
              className={cn(
                "w-full rounded-[16px] text-left outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-4 focus-visible:ring-offset-background",
                bento && "h-full",
                key === activeKey && "ring-2 ring-primary",
              )}
            >
              <div className={cn(
                "relative overflow-hidden bg-muted transition-transform duration-300 group-hover:scale-[1.015]",
                bento ? "h-full min-h-[156px] rounded-[16px]" : "aspect-square rounded-[12px]",
              )}>
                {playlist.artwork_uri ? (
                  <img src={playlist.artwork_uri} alt={playlist.name} loading="lazy" className="size-full object-cover transition-transform duration-700 group-hover:scale-[1.05]" />
                ) : (
                  <div className="flex size-full items-center justify-center text-muted-foreground"><MusicNotesIcon size={40} weight="duotone" /></div>
                )}
                <span className="cover-fade pointer-events-none absolute inset-0 opacity-80 transition-opacity duration-300 group-hover:opacity-100" />
                {bento ? (
                  <span className="absolute inset-x-4 bottom-4 block text-white">
                    <span className="block line-clamp-2 text-[14px] leading-5 font-semibold" title={playlist.name}>{playlist.name}</span>
                    <span className="mt-1 block truncate text-[12px] text-white/70">{playlist.author || "精选歌单"}{playlist.track_count ? ` · ${playlist.track_count} 首` : ""}</span>
                  </span>
                ) : (
                  <>
                    <span className="absolute right-3 bottom-3 flex size-9 translate-y-1 items-center justify-center rounded-full bg-primary text-primary-foreground opacity-0 transition-[transform,opacity] duration-200 group-hover:translate-y-0 group-hover:opacity-100 group-focus-within:translate-y-0 group-focus-within:opacity-100"><ArrowUpRightIcon size={17} weight="bold" /></span>
                    {playlist.track_count ? <span className="absolute bottom-3 left-3 rounded-md bg-black/50 px-2 py-1 text-[12px] font-medium text-white">{playlist.track_count} 首</span> : null}
                  </>
                )}
              </div>
              {!bento ? (
                <>
                  <div className="mt-3 line-clamp-2 min-h-10 text-[13px] leading-5 font-medium transition-colors group-hover:text-primary" title={playlist.name}>{playlist.name}</div>
                  <p className="mt-1 truncate text-[13px] text-muted-foreground">{playlist.author || "精选歌单"}</p>
                </>
              ) : null}
            </button>
            {onToggleSave ? (
              <Button
                variant="secondary"
                size="icon-sm"
                className={cn(
                  "absolute top-3 right-3 size-8 rounded-full transition-colors",
                  bento ? "bg-black/35 text-white hover:bg-black/55" : "bg-card/90",
                  saved ? "text-primary" : "text-muted-foreground hover:text-primary",
                )}
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
