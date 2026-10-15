import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "sonner";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { useTrackLyrics } from "@/lib/use-track-lyrics";
import KaraokeText from "@/components/KaraokeText";
import { animateLyricPosition } from "@/lib/lyric-motion";
import { useCurrentLine } from "@/lib/lyrics";
import { usePlayer } from "@/player-context";
import { useSettings } from "@/settings-context";


/// `0xRRGGBB` + 透明度 → CSS 颜色。
function toCss(rgb: number, alpha: number): string {
  const r = (rgb >> 16) & 0xff;
  const g = (rgb >> 8) & 0xff;
  const b = rgb & 0xff;
  return alpha >= 1 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/// 桌面歌词窗口：置顶、无边框、透明，当前行居中并带上下文滚动。
export default function LyricsWindow() {
  const { settings } = useSettings();
  const { snapshot } = usePlayer();
  const track = snapshot?.current ?? null;
  const { lines, loading, error } = useTrackLyrics(
    track, settings?.lyrics.show_translation ?? true, settings?.use_network_proxy ?? false,
  );

  // 桌面歌词窗口要透明：给 body 加个类，CSS 里把背景清掉。
  useEffect(() => {
    document.body.classList.add("lyrics-mode");
    return () => document.body.classList.remove("lyrics-mode");
  }, []);

  const viewportRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const scrollPosition = useRef<number | null>(null);
  const [viewportHeight, setViewportHeight] = useState(0);
  const style = settings?.lyrics;
  const singleLine = style?.single_line ?? false;
  const offset = style?.offset_ms ?? 0;
  const positionMs = snapshot?.position_ms ?? 0;
  const playing = snapshot?.playing ?? false;
  const index = useCurrentLine(lines, positionMs, playing, offset);
  const current = index >= 0 ? lines[index] : null;
  const fontSize = style?.font_size ?? 28;
  const fontSizeCss = `${fontSize}px`;
  const rowHeight = fontSize * (style?.show_translation && lines.some((line) => line.translation) ? 2.65 : 2.05);
  const reducedMotion = useReducedMotion();
  const activeIndex = index >= 0 ? index : 0;
  const listOffset = viewportHeight > 0
    ? viewportHeight / 2 - (activeIndex * rowHeight + rowHeight / 2)
    : 0;

  useLayoutEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const update = () => setViewportHeight(element.clientHeight);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
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
  const textColor = toCss(style?.text_color ?? 0xffffff, style?.text_alpha ?? 1);
  const fontWeight = style?.font_weight ?? 600;
  const animation = style?.animation ?? "slide";
  useLayoutEffect(() => {
    const list = listRef.current;
    if (!list || singleLine || !lines.length) {
      scrollPosition.current = null;
      return;
    }
    const update = (position: number) => {
      scrollPosition.current = position;
      list.style.transform = `translate3d(0, ${position}px, 0)`;
    };
    const first = scrollPosition.current === null;
    return animateLyricPosition(scrollPosition.current ?? listOffset, listOffset, update,
      first || !!reducedMotion || animation === "off");
  }, [listOffset, singleLine, lines.length, reducedMotion, animation]);

  const enterFrom =
    animation === "scale"
      ? { opacity: 0, scale: 0.88 }
      : animation === "slide"
        ? { opacity: 0, y: 12 }
        : { opacity: 1 };

  return (
    <div
      className={`relative flex h-full w-full flex-col justify-center gap-1.5 overflow-hidden px-4 py-3 ${alignment}`}
      style={{ background, opacity: style?.opacity ?? 1, textAlign }}
    >
      {!style?.locked ? (
        <button
          type="button"
          aria-label="关闭桌面歌词"
          title="关闭桌面歌词"
          className="absolute right-1 top-1 z-10 flex h-7 w-7 items-center justify-center rounded bg-black/60 text-lg text-white hover:bg-black/80"
          onClick={() => void getCurrentWindow().close().catch((problem) => toast.error(`关闭桌面歌词失败：${problem}`))}
        >×</button>
      ) : null}
      <div ref={viewportRef} className="relative h-full w-full overflow-hidden">
        {!singleLine && lines.length > 0 ? (
          <div
            ref={listRef}
            className="absolute left-0 top-0 w-full will-change-transform"
            data-lyrics-list
          >
            {lines.map((line, lineIndex) => {
              const distance = index >= 0 ? Math.abs(lineIndex - index) : lineIndex;
              const isCurrent = lineIndex === index;
              const nextLine = lines[lineIndex + 1];
              const lineColor = lineIndex <= index ? highlight : textColor;
              const lineOpacity = isCurrent ? 1 : Math.max(0.2, 0.78 - distance * 0.12);
              return (
                <motion.div
                  key={`${line.time_ms}-${lineIndex}`}
                  data-lyric-index={lineIndex}
                  aria-current={isCurrent ? "true" : undefined}
                  className="flex w-full flex-col justify-center overflow-hidden px-1"
                  animate={{ opacity: lineOpacity }}
                  transition={{ duration: reducedMotion || animation === "off" ? 0 : 0.7, ease: [0.22, 1, 0.36, 1] }}
                  style={{ height: rowHeight, textAlign }}
                >
                  <div
                    className="w-full truncate leading-tight"
                    style={{ fontSize: fontSizeCss, fontWeight, fontFamily, textShadow, color: karaoke && isCurrent ? undefined : lineColor }}
                  >
                    {karaoke && isCurrent ? (
                      <KaraokeText
                        text={line.text}
                        lineTime={line.time_ms}
                        nextTime={nextLine?.time_ms ?? line.time_ms}
                        positionMs={positionMs}
                        playing={playing}
                        offsetMs={offset}
                        highlight={highlight}
                        textColor={textColor}
                      />
                    ) : line.text || "♪"}
                  </div>
                  {style?.show_translation && line.translation ? (
                    <div
                      className="w-full truncate leading-tight"
                      style={{ fontSize: `${fontSize * 0.52}px`, color: textColor, textShadow, fontFamily, opacity: isCurrent ? 0.86 : 0.68 }}
                    >
                      {line.translation}
                    </div>
                  ) : null}
                </motion.div>
              );
            })}
          </div>
        ) : current ? (
          <motion.div
            key={current.time_ms}
            initial={enterFrom}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
            className="absolute inset-x-0 top-1/2 -translate-y-1/2 px-1"
          >
            <div className="w-full truncate leading-tight" style={{ fontSize: fontSizeCss, fontWeight, fontFamily, textShadow, color: karaoke ? undefined : highlight, textAlign }}>
              {karaoke ? (
                <KaraokeText text={current.text} lineTime={current.time_ms} nextTime={lines[index + 1]?.time_ms ?? current.time_ms} positionMs={positionMs} playing={playing} offsetMs={offset} highlight={highlight} textColor={textColor} />
              ) : current.text || "♪"}
            </div>
            {style?.show_translation && current.translation ? (
              <div className="w-full truncate" style={{ fontSize: `${fontSize * 0.6}px`, color: textColor, textShadow, fontFamily, opacity: 0.8, textAlign }}>
                {current.translation}
              </div>
            ) : null}
          </motion.div>
        ) : (
          <div className="absolute inset-x-0 top-1/2 -translate-y-1/2 px-1 text-sm" style={{ color: textColor, textShadow, textAlign }}>
            {error ?? (loading ? "正在加载歌词…" : track ? "这首歌没有找到歌词" : "未在播放")}
          </div>
        )}
      </div>
    </div>
  );
}
