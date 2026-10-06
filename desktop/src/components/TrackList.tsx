import { formatTime, usePlayer } from "../player-context";
import type { Track } from "../types";

interface Props {
  tracks: Track[];
  /// 传入时在行尾显示收藏按钮（`saved` 决定按钮文案）。
  onToggleSave?: (track: Track) => void;
  saved?: (track: Track) => boolean;
  empty?: string;
}

/// 曲目列表：点行播放，可选收藏按钮。搜索页、榜单页、歌单页共用。
export default function TrackList({ tracks, onToggleSave, saved, empty }: Props) {
  const { play, snapshot } = usePlayer();
  const playingId = snapshot?.current?.id ?? null;

  if (tracks.length === 0) {
    return <p className="hint">{empty ?? "这里还没有歌曲。"}</p>;
  }

  return (
    <ul className={onToggleSave ? "track-list with-save" : "track-list"}>
      {tracks.map((track) => (
        <li
          key={`${track.source}-${track.id}`}
          className={track.id === playingId ? "track playing" : "track"}
          onClick={() => void play(track)}
        >
          <span className="track-title">{track.title}</span>
          <span className="track-artist">{track.artist}</span>
          <span className="track-album">{track.album}</span>
          <span className="track-time">{formatTime(track.durationMs)}</span>
          {onToggleSave ? (
            <button
              className="save"
              onClick={(event) => {
                event.stopPropagation();
                onToggleSave(track);
              }}
            >
              {saved?.(track) ? "已收藏" : "收藏"}
            </button>
          ) : null}
        </li>
      ))}
    </ul>
  );
}
