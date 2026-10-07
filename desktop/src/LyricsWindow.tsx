import { motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";

import { lyrics as lyricsApi } from "@/api";
import { usePlayer } from "@/player-context";
import { useSettings } from "@/settings-context";
import type { LyricLine } from "@/types";

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

/// 桌面歌词窗口：置顶、无边框、透明，只显示当前行（与下一行）。
export default function LyricsWindow() {
  const { settings } = useSettings();
  const { snapshot } = usePlayer();
  const track = snapshot?.current ?? null;
  const [lines, setLines] = useState<LyricLine[]>([]);
  const trackKey = track ? `${track.source}-${track.id}` : null;

  // 桌面歌词窗口要透明：给 body 加个类，CSS 里把背景清掉。
  useEffect(() => {
    document.body.classList.add("lyrics-mode");
    return () => document.body.classList.remove("lyrics-mode");
  }, []);

  useEffect(() => {
    if (!track) {
      setLines([]);
      return;
    }
    let alive = true;
    lyricsApi
      .load(track)
      .then((next) => {
        if (alive) {
          setLines(next);
        }
      })
      .catch(() => {
        if (alive) {
          setLines([]);
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

  const textColor = toCss(style?.text_color ?? 0xffffff, style?.text_alpha ?? 1);
  const highlight = toCss(style?.highlight_color ?? 0xffffff, style?.highlight_alpha ?? 1);
  const strokeWidth = style?.stroke_width ?? 0;
  const strokeColor = toCss(style?.stroke_color ?? 0x000000, style?.stroke_alpha ?? 1);
  const textShadow =
    strokeWidth > 0
      ? `0 0 ${strokeWidth}px ${strokeColor}, 0 0 ${strokeWidth * 2}px ${strokeColor}`
      : undefined;
  const background = toCss(
    style?.background_color ?? 0x000000,
    style?.background_opacity ?? 0,
  );
  const alignment =
    style?.alignment === "left" ? "items-start" : style?.alignment === "right" ? "items-end" : "items-center";
  const textAlign =
    style?.alignment === "left" ? "left" : style?.alignment === "right" ? "right" : "center";
  const fontFamily = style?.font_family ? `"${style.font_family}"` : undefined;
  const karaoke = style?.karaoke ?? false;

  return (
    <div
      className={`flex h-full w-full flex-col justify-center gap-1.5 overflow-hidden px-4 py-3 ${alignment}`}
      style={{ background, opacity: style?.opacity ?? 1, textAlign }}
    >
      {current ? (
        <motion.div
          key={current.time_ms}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.24, ease: [0.22, 1, 0.36, 1] }}
          className="w-full truncate leading-tight"
          style={{
            fontSize: `${style?.font_size ?? 28}px`,
            fontWeight: style?.font_weight ?? 600,
            fontFamily,
            textShadow,
            color: karaoke ? "transparent" : highlight,
            backgroundImage: karaoke
              ? `linear-gradient(90deg, ${highlight} ${progress * 100}%, ${textColor} ${progress * 100}%)`
              : undefined,
            WebkitBackgroundClip: karaoke ? "text" : undefined,
            backgroundClip: karaoke ? "text" : undefined,
          }}
        >
          {current.text || "♪"}
        </motion.div>
      ) : (
        <div className="text-sm opacity-80" style={{ color: textColor, textShadow }}>
          {track ? "这首歌没有找到歌词" : "未在播放"}
        </div>
      )}

      {style?.show_translation && current?.translation ? (
        <div
          className="w-full truncate"
          style={{ fontSize: "0.6em", color: textColor, textShadow, fontFamily, opacity: 0.8 }}
        >
          {current.translation}
        </div>
      ) : null}

      {!style?.single_line && next ? (
        <div
          className="w-full truncate opacity-70"
          style={{ fontSize: "0.62em", color: textColor, textShadow, fontFamily }}
        >
          {next.text}
        </div>
      ) : null}
    </div>
  );
}
