import { ArrowLeftIcon } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { favorites, online } from "@/api";
import PlaylistGrid, { playlistKey } from "@/components/PlaylistGrid";
import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useSettings } from "@/settings-context";
import type { PlatformPlaylist, Track } from "@/types";

/// 歌单页：收藏的歌单 + 「爱听的」四个文件夹。
export default function PlaylistsPage() {
  const { settings, replace } = useSettings();
  const [folders, setFolders] = useState<string[]>([]);
  const [folder, setFolder] = useState("我的收藏");
  const [opened, setOpened] = useState<{ name: string; tracks: Track[] } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    favorites
      .folders()
      .then((next) => {
        setFolders(next);
        if (next.length > 1) {
          setFolder(next[1]);
        }
      })
      .catch(() => setFolders([]));
  }, []);

  const openPlaylist = async (playlist: PlatformPlaylist) => {
    setLoading(true);
    setError(null);
    try {
      setOpened({ name: playlist.name, tracks: await online.playlistTracks(playlist) });
    } catch (problem) {
      setError(String(problem));
    } finally {
      setLoading(false);
    }
  };

  const savedPlaylists = settings?.saved_playlists ?? [];
  const savedTracks = settings?.saved_tracks ?? [];
  const isSaved = (playlist: PlatformPlaylist) =>
    savedPlaylists.some((item) => playlistKey(item.playlist) === playlistKey(playlist));
  const isTrackSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === folder &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );
  const folderTracks = savedTracks
    .filter((item) => item.folder === folder)
    .map((item) => item.track);

  return (
    <div className="h-full overflow-y-auto px-7 py-6">
      <h1 className="mb-5 text-[22px] font-semibold">歌单</h1>

      {error ? (
        <div className="mb-4 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <section className="mb-8 flex flex-col gap-3">
        <h2 className="text-[13px] font-semibold tracking-wide text-muted-foreground">
          收藏的歌单
        </h2>
        <PlaylistGrid
          playlists={savedPlaylists.map((item) => item.playlist)}
          onOpen={(playlist) => void openPlaylist(playlist)}
          onToggleSave={(playlist) => {
            void favorites.togglePlaylist(playlist).then(replace);
          }}
          isSaved={isSaved}
          empty="还没有收藏的歌单。"
        />
      </section>

      {opened ? (
        <section className="mb-8 flex flex-col gap-3">
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="icon-sm" onClick={() => setOpened(null)}>
              <ArrowLeftIcon />
            </Button>
            <h2 className="text-sm font-semibold">{opened.name}</h2>
            <span className="text-xs text-muted-foreground">{opened.tracks.length} 首</span>
          </div>
          <TrackList
            tracks={opened.tracks}
            onToggleSave={(track) => {
              void favorites.toggleTrack(track, folder).then(replace);
            }}
            saved={isTrackSaved}
          />
        </section>
      ) : loading ? (
        <Skeleton className="mb-8 h-40 rounded-xl" />
      ) : null}

      <section className="flex flex-col gap-3">
        <h2 className="text-[13px] font-semibold tracking-wide text-muted-foreground">
          爱听的
        </h2>
        <div className="flex flex-wrap gap-1.5">
          {folders.map((name) => (
            <Button
              key={name}
              size="sm"
              variant={name === folder ? "default" : "outline"}
              onClick={() => setFolder(name)}
            >
              {name}
            </Button>
          ))}
        </div>
        <TrackList
          tracks={folderTracks}
          onToggleSave={(track) => {
            void favorites.toggleTrack(track, folder).then(replace);
          }}
          saved={isTrackSaved}
          empty={`「${folder}」还没有歌曲，在榜单或歌单里点行尾的星标加进来。`}
        />
      </section>
    </div>
  );
}
