import { formatTime, usePlayer } from "../player-context";

export default function PlayerBar({ onOpenNowPlaying }: { onOpenNowPlaying: () => void }) {
  const {
    snapshot,
    artwork,
    toggle,
    next,
    previous,
    seek,
    setVolume,
    setSpatial,
    queueLength,
  } = usePlayer();
  const track = snapshot?.current ?? null;
  const duration = track?.durationMs ?? 0;
  const position = snapshot?.position_ms ?? 0;

  return (
    <footer className="player-bar">
      <div className="now" onClick={onOpenNowPlaying} title="打开歌曲详情页">
        <div className="cover">
          {artwork ? <img src={artwork} alt="" /> : null}
        </div>
        <div className="meta">
          <div className="title">{track?.title ?? "未在播放"}</div>
          <div className="sub">
            {snapshot?.error
              ? snapshot.error
              : snapshot?.loading
                ? "正在解析整曲地址…"
                : (track?.artist ?? "点搜索结果里的歌曲开始播放")}
          </div>
        </div>
      </div>

      <div className="controls">
        <button
          className="step"
          onClick={() => void previous()}
          disabled={queueLength === 0}
          title="上一首"
        >
          ⏮
        </button>
        <button className="play" onClick={() => void toggle()} disabled={!track}>
          {snapshot?.playing ? "暂停" : "播放"}
        </button>
        <button
          className="step"
          onClick={() => void next()}
          disabled={queueLength === 0}
          title="下一首"
        >
          ⏭
        </button>
        <span className="time">
          {formatTime(position)} / {formatTime(duration)}
        </span>
        <input
          className="progress"
          type="range"
          min={0}
          max={Math.max(duration, position, 1)}
          value={position}
          onChange={(event) => void seek(Number(event.target.value))}
          disabled={!track}
        />
      </div>

      <div className="right">
        <label className="spatial">
          <input
            type="checkbox"
            checked={snapshot?.spatial ?? false}
            onChange={(event) => void setSpatial(event.target.checked)}
          />
          空间音频
        </label>
        <input
          className="volume"
          type="range"
          min={0}
          max={100}
          value={Math.round((snapshot?.volume ?? 1) * 100)}
          onChange={(event) => void setVolume(Number(event.target.value) / 100)}
        />
      </div>
    </footer>
  );
}
