import { useCallback, useEffect, useState } from "react";

import { favorites, online } from "../api";
import PlaylistGrid, { playlistKey } from "../components/PlaylistGrid";
import TrackList from "../components/TrackList";
import { useSettings } from "../settings-context";
import type { OnlineSearchChannel, PlatformPlaylist, Track } from "../types";

export const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

/// 「此刻」页：平台热门歌单 + 我的收藏。
export default function HomePage() {
  const { settings, replace } = useSettings();
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [hot, setHot] = useState<PlatformPlaylist[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState<{ name: string; tracks: Track[] } | null>(null);
  const [opening, setOpening] = useState(false);

  const loadHot = useCallback(async (next: OnlineSearchChannel) => {
    setLoading(true);
    setError(null);
    try {
      setHot(await online.playlists(next));
    } catch (problem) {
      setError(String(problem));
      setHot([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadHot(channel);
  }, [channel, loadHot]);

  const openPlaylist = async (playlist: PlatformPlaylist) => {
    setOpening(true);
    setError(null);
    try {
      setOpened({
        name: playlist.name,
        tracks: await online.playlistTracks(playlist),
      });
    } catch (problem) {
      setError(String(problem));
    } finally {
      setOpening(false);
    }
  };

  const savedPlaylists = settings?.saved_playlists ?? [];
  const savedTracks = settings?.saved_tracks ?? [];
  const isSaved = (playlist: PlatformPlaylist) =>
    savedPlaylists.some((item) => playlistKey(item.playlist) === playlistKey(playlist));
  const isTrackSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === "我的收藏" &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );

  return (
    <div className="home">
      <header className="hero">
        <div className="hero-kicker">WCMusic / 每日发现</div>
        <h1>此刻，听见喜欢</h1>
      </header>

      {error ? <p className="error">{error}</p> : null}

      <section>
        <div className="section-head">
          <h2>平台热门歌单</h2>
          <select
            value={channel}
            onChange={(event) => setChannel(event.target.value as OnlineSearchChannel)}
          >
            {CHANNELS.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </div>
        {loading ? (
          <p className="hint">正在加载{CHANNELS.find((c) => c.value === channel)?.label}热门歌单…</p>
        ) : (
          <PlaylistGrid
            playlists={hot}
            activeKey={null}
            onOpen={(playlist) => void openPlaylist(playlist)}
            onToggleSave={(playlist) => {
              void favorites.togglePlaylist(playlist).then(replace);
            }}
            isSaved={isSaved}
          />
        )}
      </section>

      <section>
        <div className="section-head">
          <h2>我的收藏</h2>
        </div>
        <PlaylistGrid
          playlists={savedPlaylists.map((item) => item.playlist)}
          activeKey={null}
          onOpen={(playlist) => void openPlaylist(playlist)}
          onToggleSave={(playlist) => {
            void favorites.togglePlaylist(playlist).then(replace);
          }}
          isSaved={isSaved}
          empty="还没有收藏的歌单，点热门歌单卡片上的「收藏」。"
        />
      </section>

      {opening ? <p className="hint">正在打开歌单…</p> : null}
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
              void favorites.toggleTrack(track, "我的收藏").then(replace);
            }}
            saved={isTrackSaved}
          />
        </section>
      ) : null}
    </div>
  );
}
