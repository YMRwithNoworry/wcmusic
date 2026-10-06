import { useState } from "react";

import { online } from "../api";
import { formatTime, usePlayer } from "../player-context";
import type { OnlineSearchChannel, Track } from "../types";

const CHANNELS: { value: OnlineSearchChannel; label: string }[] = [
  { value: "Kuwo", label: "酷我音乐" },
  { value: "Kugou", label: "酷狗音乐" },
  { value: "QqMusic", label: "QQ 音乐" },
  { value: "Netease", label: "网易云音乐" },
];

export default function SearchPage() {
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { play, snapshot } = usePlayer();
  const playingId = snapshot?.current?.id ?? null;

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
      {!loading && !error && results.length === 0 ? (
        <p className="hint">输入关键字后回车即可搜索，点结果行开始播放。</p>
      ) : null}

      <ul className="track-list">
        {results.map((track) => (
          <li
            key={`${track.source}-${track.id}`}
            className={track.id === playingId ? "track playing" : "track"}
            onClick={() => void play(track)}
          >
            <span className="track-title">{track.title}</span>
            <span className="track-artist">{track.artist}</span>
            <span className="track-album">{track.album}</span>
            <span className="track-time">{formatTime(track.durationMs)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
