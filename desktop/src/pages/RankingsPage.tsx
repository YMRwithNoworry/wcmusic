import { useCallback, useEffect, useState } from "react";

import { favorites, online } from "../api";
import TrackList from "../components/TrackList";
import { CHANNELS } from "./HomePage";
import { useSettings } from "../settings-context";
import type { OnlineSearchChannel, PlatformRanking, Track } from "../types";

/// 排行榜页：先选平台，再选榜单，最后出曲目。
export default function RankingsPage() {
  const { settings, replace } = useSettings();
  const [channel, setChannel] = useState<OnlineSearchChannel>("Kuwo");
  const [rankings, setRankings] = useState<PlatformRanking[]>([]);
  const [active, setActive] = useState<PlatformRanking | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const all = await online.rankings();
      setRankings(all);
      const first = all.find((item) => item.channel === channel) ?? null;
      setActive(first);
    } catch (problem) {
      setError(String(problem));
      setRankings([]);
      setActive(null);
    } finally {
      setLoading(false);
    }
  }, [channel]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!active) {
      setTracks([]);
      return;
    }
    let alive = true;
    setLoading(true);
    online
      .rankingTracks(active)
      .then((next) => {
        if (alive) {
          setTracks(next);
        }
      })
      .catch((problem) => {
        if (alive) {
          setError(String(problem));
          setTracks([]);
        }
      })
      .finally(() => {
        if (alive) {
          setLoading(false);
        }
      });
    return () => {
      alive = false;
    };
  }, [active]);

  const savedTracks = settings?.saved_tracks ?? [];
  const isSaved = (track: Track) =>
    savedTracks.some(
      (item) =>
        item.folder === "我的收藏" &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );

  const visible = rankings.filter((item) => item.channel === channel);

  return (
    <div className="rankings">
      <div className="section-head">
        <h1>排行榜</h1>
        <select
          value={channel}
          onChange={(event) => {
            setActive(null);
            setChannel(event.target.value as OnlineSearchChannel);
          }}
        >
          {CHANNELS.map((item) => (
            <option key={item.value} value={item.value}>
              {item.label}
            </option>
          ))}
        </select>
      </div>

      {error ? <p className="error">{error}</p> : null}

      <div className="chip-row">
        {visible.map((ranking) => (
          <button
            key={ranking.id}
            className={ranking.id === active?.id ? "chip active" : "chip"}
            onClick={() => setActive(ranking)}
          >
            {ranking.name}
          </button>
        ))}
        {!loading && visible.length === 0 ? <span className="hint">没有取到榜单。</span> : null}
      </div>

      {loading ? (
        <p className="hint">正在加载…</p>
      ) : (
        <TrackList
          tracks={tracks}
          onToggleSave={(track) => {
            void favorites.toggleTrack(track, "我的收藏").then(replace);
          }}
          saved={isSaved}
          empty="选一个榜单看看。"
        />
      )}
    </div>
  );
}
