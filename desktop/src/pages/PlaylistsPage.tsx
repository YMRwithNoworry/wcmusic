import { useEffect, useState } from "react";

import { favorites, online } from "@/api";
import PlaylistDetail from "@/components/PlaylistDetail";
import PlaylistGrid, { playlistKey } from "@/components/PlaylistGrid";
import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
import { useSettings } from "@/settings-context";
import type { PlatformPlaylist, Track } from "@/types";

/// 歌单页：收藏的歌单 + 「爱听的」四个文件夹。
export default function PlaylistsPage() {
  const { settings, replace } = useSettings();
  const [folders, setFolders] = useState<string[]>([]);
  const [folder, setFolder] = useState("我的收藏");
  /// 点开的歌单：`tracks` 为 `null` 表示还在载入。
  const [detail, setDetail] = useState<{ name: string; tracks: Track[] | null } | null>(null);
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
    setDetail({ name: playlist.name, tracks: null });
    setError(null);
    try {
      setDetail({ name: playlist.name, tracks: await online.playlistTracks(playlist) });
    } catch (problem) {
      setError(String(problem));
      setDetail(null);
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
  const toggleTrack = (track: Track) => {
    void favorites.toggleTrack(track, folder).then(replace);
  };
  const folderTracks = savedTracks
    .filter((item) => item.folder === folder)
    .map((item) => item.track);

  if (detail) {
    return (
      <PlaylistDetail
        name={detail.name}
        tracks={detail.tracks}
        onBack={() => setDetail(null)}
        onToggleSave={toggleTrack}
        saved={isTrackSaved}
      />
    );
  }

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
          onToggleSave={toggleTrack}
          saved={isTrackSaved}
          empty={`「${folder}」还没有歌曲，在榜单或歌单里点行尾的星标加进来。`}
        />
      </section>
    </div>
  );
}
