import { useState } from "react";

import { favorites, online } from "../api";
import TrackList from "../components/TrackList";
import { useSettings } from "../settings-context";
import type { OnlineSearchChannel, Track } from "../types";

const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

export default function SearchPage() {
  const { settings, replace } = useSettings();
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    const keyword = query.trim();
    if (!keyword || loading) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      setResults(await online.search(keyword, channel));
    } catch (problem) {
      setError(String(problem));
      setResults([]);
    } finally {
      setLoading(false);
    }
  };

  const savedTracks = settings?.saved_tracks ?? [];
  const isTrackSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === "我的收藏" &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );

  return (
    <div className="search">
      <div className="search-bar">
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
        <input
          className="query"
          value={query}
          placeholder="搜索歌曲 / 歌手"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              void run();
            }
          }}
        />
        <button onClick={() => void run()} disabled={loading}>
          {loading ? "搜索中…" : "搜索"}
        </button>
      </div>

      {error ? <p className="error">{error}</p> : null}

      {loading ? (
        <p className="hint">搜索中…</p>
      ) : (
        <TrackList
          tracks={results}
          onToggleSave={(track) => {
            void favorites.toggleTrack(track, "我的收藏").then(replace);
          }}
          saved={isTrackSaved}
          empty="输入关键字后回车即可搜索，点结果行开始播放。"
        />
      )}
    </div>
  );
}
