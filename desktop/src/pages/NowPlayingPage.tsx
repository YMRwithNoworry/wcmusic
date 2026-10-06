import { useEffect, useMemo, useRef, useState } from "react";

import { lyrics as lyricsApi } from "../api";
import { usePlayer } from "../player-context";
import { useSettings } from "../settings-context";
import type { LyricLine } from "../types";

/// `0xRRGGBB` + 透明度 → CSS 颜色。
function toCss(rgb: number, alpha: number): string {
  const r = (rgb >> 16) & 0xff;
  const g = (rgb >> 8) & 0xff;
  const b = rgb & 0xff;
  return alpha >= 1 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/// 当前行下标：最后一条 `time_ms <= position` 的歌词。
function currentIndex(lines: LyricLine[], position: number): number {
  let index = -1;
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i].time_ms > position) {
      break;
    }
    index = i;
  }
  return index;
}

/// 歌曲详情页：大封面 + 歌曲信息 + 逐字歌词（点歌词跳转）。
export default function NowPlayingPage({ onBack }: { onBack: () => void }) {
  const { settings } = useSettings();
  const { snapshot, artwork, seek } = usePlayer();
  const track = snapshot?.current ?? null;
  const [lines, setLines] = useState<LyricLine[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);

  const trackKey = track ? `${track.source}-${track.id}` : null;

  useEffect(() => {
    if (!track) {
      setLines([]);
      setError(null);
      return;
    }
    let alive = true;
    setLoading(true);
    setError(null);
    lyricsApi
      .load(track)
      .then((next) => {
        if (alive) {
          setLines(next);
        }
      })
      .catch((problem) => {
        if (alive) {
          setError(String(problem));
          setLines([]);
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
    // 只在换歌时重新抓歌词。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trackKey]);

  const style = settings?.lyrics;
  const offset = style?.offset_ms ?? 0;
  const position = Math.max(0, (snapshot?.position_ms ?? 0) + offset);
  const index = useMemo(() => currentIndex(lines, position), [lines, position]);
  const current = index >= 0 ? lines[index] : null;
  const next = index >= 0 ? lines[index + 1] : null;
  const progress =
    current && next && next.time_ms > current.time_ms
      ? Math.min(1, Math.max(0, (position - current.time_ms) / (next.time_ms - current.time_ms)))
      : 0;

  // 当前行居中：换行时滚一次。
  useEffect(() => {
    const element = listRef.current?.querySelector(".lyric-line.current");
    element?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [index]);

  const textColor = toCss(style?.text_color ?? 0xffffff, style?.text_alpha ?? 1);
  const highlight = toCss(style?.highlight_color ?? 0xffffff, style?.highlight_alpha ?? 1);
  const karaoke = style?.karaoke ?? false;

  return (
    <div className="now-playing">
      <div className="np-left">
        <button className="link" onClick={onBack}>
          ← 返回
        </button>
        <div className="np-cover">
          {artwork ? <img src={artwork} alt="" /> : null}
        </div>
        <div className="np-meta">
          <div className="np-title">{track?.title ?? "未在播放"}</div>
          <div className="np-sub">{track?.artist}</div>
          <div className="np-sub">{track?.album}</div>
        </div>
      </div>

      <div
        className="np-lyrics"
        ref={listRef}
        style={{ textAlign: (style?.alignment ?? "center") as "left" | "center" | "right" }}
      >
        {loading ? <p className="hint">正在获取歌词…</p> : null}
        {error ? <p className="error">{error}</p> : null}
        {!loading && !error && lines.length === 0 ? (
          <p className="hint">{track ? "这首歌没有找到歌词。" : "先播放一首歌。"}</p>
        ) : null}
        {lines.map((line, lineIndex) => {
          const isCurrent = lineIndex === index;
          const karaokeStyle =
            isCurrent && karaoke
              ? {
                  backgroundImage: `linear-gradient(90deg, ${highlight} ${progress * 100}%, ${textColor} ${progress * 100}%)`,
                  WebkitBackgroundClip: "text" as const,
                  backgroundClip: "text" as const,
                  color: "transparent",
                }
              : undefined;
          return (
            <div
              key={`${line.time_ms}-${lineIndex}`}
              className={isCurrent ? "lyric-line current" : "lyric-line"}
              style={{
                fontSize: `${style?.font_size ?? 28}px`,
                fontWeight: style?.font_weight ?? 400,
                color: isCurrent && !karaoke ? highlight : textColor,
              }}
              onClick={() => void seek(Math.max(0, line.time_ms - offset))}
            >
              <span className="lyric-text" style={karaokeStyle}>
                {line.text || "♪"}
              </span>
              {style?.show_translation && line.translation ? (
                <span className="lyric-translation" style={{ color: textColor }}>
                  {line.translation}
                </span>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}
