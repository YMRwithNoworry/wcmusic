import { useEffect, useState } from "react";

import { favorites, online } from "../api";
import PlaylistGrid, { playlistKey } from "../components/PlaylistGrid";
import TrackList from "../components/TrackList";
import { useSettings } from "../settings-context";
import type { PlatformPlaylist, Track } from "../types";

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
        if (next.length > 0 && !next.includes(folder)) {
          setFolder(next[1] ?? next[0]);
        }
      })
      .catch(() => setFolders([]));
    // 只在挂载时取一次文件夹列表。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const openPlaylist = async (playlist: PlatformPlaylist) => {
    setLoading(true);
    setError(null);
    try {
      setOpened({
        name: playlist.name,
        tracks: await online.playlistTracks(playlist),
      });
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
    <div className="playlists">
      <h1>歌单</h1>
      {error ? <p className="error">{error}</p> : null}

      <section>
        <div className="section-head">
          <h2>收藏的歌单</h2>
        </div>
        <PlaylistGrid
          playlists={savedPlaylists.map((item) => item.playlist)}
          activeKey={null}
          onOpen={(playlist) => void openPlaylist(playlist)}
          onToggleSave={(playlist) => {
            void favorites.togglePlaylist(playlist).then(replace);
          }}
          isSaved={isSaved}
          empty="还没有收藏的歌单。"
        />
      </section>

      {loading ? <p className="hint">正在打开歌单…</p> : null}
      {opened ? (
        <section>
          <div className="section-head">
            <h2>{opened.name}</h2>
            <button className="link" onClick={() => setOpened(null)}>
              收起
            </button>
          </div>
          <TrackList
            tracks={opened.tracks}
            onToggleSave={(track) => {
              void favorites.toggleTrack(track, folder).then(replace);
            }}
            saved={isTrackSaved}
          />
        </section>
      ) : null}

      <section>
        <div className="section-head">
          <h2>爱听的</h2>
        </div>
        <div className="chip-row">
          {folders.map((name) => (
            <button
              key={name}
              className={name === folder ? "chip active" : "chip"}
              onClick={() => setFolder(name)}
            >
              {name}
            </button>
          ))}
        </div>
        <TrackList
          tracks={folderTracks}
          onToggleSave={(track) => {
            void favorites.toggleTrack(track, folder).then(replace);
          }}
          saved={isTrackSaved}
          empty={`「${folder}」还没有歌曲，在榜单/歌单里点行尾的「收藏」加进来。`}
        />
      </section>
    </div>
  );
}
