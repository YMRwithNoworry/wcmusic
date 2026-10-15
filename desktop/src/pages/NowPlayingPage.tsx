import { ArrowLeftIcon } from "@phosphor-icons/react";
import { motion, useReducedMotion } from "motion/react";
import { memo, useCallback, useEffect, useRef, useState } from "react";

import { useTrackLyrics, pageLyricColor } from "@/lib/use-track-lyrics";
import KaraokeText from "@/components/KaraokeText";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { animateLyricPosition } from "@/lib/lyric-motion";
import { useCurrentLine } from "@/lib/lyrics";
import { usePlayer } from "@/player-context";
import { useSettings } from "@/settings-context";
import type { LyricLine } from "@/types";

interface LyricRowProps {
  line: LyricLine;
  nextTime: number;
  isCurrent: boolean;
  karaoke: boolean;
  fontSize: number;
  fontWeight: number;
  textColor: string;
  highlight: string;
  showTranslation: boolean;
  /// 放大时的变换原点（跟随对齐方式，居中时不会左右偏移）。
  origin: string;
  positionMs: number;
  playing: boolean;
  offsetMs: number;
  onSeek: (timeMs: number) => void;
}

/// 单行歌词：用 memo 隔离。轮询刷新播放位置时，只有当前行的 props 变化，
/// 其余行直接跳过渲染。
const LyricRow = memo(function LyricRow({
  line,
  nextTime,
  isCurrent,
  karaoke,
  fontSize,
  fontWeight,
  textColor,
  highlight,
  showTranslation,
  origin,
  positionMs,
  playing,
  offsetMs,
  onSeek,
}: LyricRowProps) {
  const reducedMotion = useReducedMotion();
  return (
    <motion.div
      // 当前行放大、变亮；其它行缩小、变暗。用缓动而不是直接跳变。
      animate={{ opacity: isCurrent ? 1 : 0.36, scale: isCurrent ? 1.08 : 0.9 }}
      transition={{ duration: reducedMotion ? 0 : 0.75, ease: [0.22, 1, 0.36, 1] }}
      onClick={() => onSeek(line.time_ms)}
      className={`cursor-pointer py-4 ${origin} ${isCurrent ? "lyric-line-current" : ""}`}
      style={{ fontSize: `${fontSize}px` }}
    >
      {isCurrent && karaoke ? (
        <KaraokeText
          text={line.text}
          lineTime={line.time_ms}
          nextTime={nextTime}
          positionMs={positionMs}
          playing={playing}
          offsetMs={offsetMs}
          highlight={highlight}
          textColor={textColor}
          className="inline-block leading-snug"
          style={{ fontWeight }}
        />
      ) : (
        <span
          className="inline-block leading-snug"
          style={{ fontWeight, color: isCurrent ? highlight : textColor }}
        >
          {line.text || "♪"}
        </span>
      )}
      {showTranslation && line.translation ? (
        <div
          className="leading-snug"
          style={{ fontSize: "0.6em", color: textColor, opacity: 0.75 }}
        >
          {line.translation}
        </div>
      ) : null}
    </motion.div>
  );
});

/// 歌曲详情页：大封面 + 歌曲信息 + 逐字歌词（点歌词跳转）。
export default function NowPlayingPage({ onBack }: { onBack: () => void }) {
  const { settings } = useSettings();
  const reducedMotion = useReducedMotion();
  const { snapshot, artwork, seek } = usePlayer();
  const track = snapshot?.current ?? null;
  const { lines, loading, error, retry } = useTrackLyrics(
    track, settings?.lyrics.show_translation ?? true, settings?.use_network_proxy ?? false,
  );
  /// 仅手动滚动时短暂暂停跟随，停手后自动接回。
  const [following, setFollowing] = useState(true);
  const listRef = useRef<HTMLDivElement | null>(null);
  const resumeTimer = useRef<number | null>(null);

  const style = settings?.lyrics;
  const offset = style?.offset_ms ?? 0;
  const position = snapshot?.position_ms ?? 0;
  const playing = snapshot?.playing ?? false;
  const index = useCurrentLine(lines, position, playing, offset);

  const dark = settings?.dark_theme ?? true;
  const textColor = pageLyricColor(style?.text_color ?? 0xffffff, style?.text_alpha ?? 1, dark);
  const highlight = pageLyricColor(style?.highlight_color ?? 0xffffff, style?.highlight_alpha ?? 1, dark);
  const karaoke = style?.karaoke ?? false;
  const fontSize = style?.font_size ?? 28;
  const fontWeight = style?.font_weight ?? 500;
  const showTranslation = style?.show_translation ?? true;
  // 放大时围绕对齐方向缩放，居中歌词不会左右偏移。
  const origin =
    style?.alignment === "left"
      ? "origin-left"
      : style?.alignment === "right"
        ? "origin-right"
        : "origin-center";

  const onSeek = useCallback((timeMs: number) => {
    if (resumeTimer.current !== null) {
      window.clearTimeout(resumeTimer.current);
      resumeTimer.current = null;
    }
    setFollowing(true);
    void seek(Math.max(0, timeMs - offset));
  }, [seek, offset]);

  /// 只有滚轮/触摸拖动暂停跟随，普通点击不打断换行动画。
  const pauseFollow = useCallback(() => {
    setFollowing(false);
    if (resumeTimer.current !== null) {
      window.clearTimeout(resumeTimer.current);
    }
    resumeTimer.current = window.setTimeout(() => {
      resumeTimer.current = null;
      setFollowing(true);
    }, 1500);
  }, []);

  useEffect(() => {
    if (resumeTimer.current !== null) {
      window.clearTimeout(resumeTimer.current);
      resumeTimer.current = null;
    }
    setFollowing(true);
  }, [track?.id, track?.source]);

  useEffect(
    () => () => {
      if (resumeTimer.current !== null) {
        window.clearTimeout(resumeTimer.current);
      }
    },
    [],
  );

  // 首尾歌词也能停在中线；尺寸变化时更新留白。
  useEffect(() => {
    const container = listRef.current;
    if (!container) return;
    const resize = () => {
      container.style.paddingBlock = `${container.clientHeight / 2}px`;
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  // 使用明确的缓动曲线；换行、跳转或用户滚动时中止上一段动画。
  useEffect(() => {
    if (!following) return;
    const container = listRef.current;
    const line = container?.querySelector<HTMLElement>(".lyric-line-current");
    if (!container || !line) return;
    const target = line.offsetTop + line.offsetHeight / 2 - container.clientHeight / 2;
    return animateLyricPosition(container.scrollTop, target,
      (value) => { container.scrollTop = value; },
      !!reducedMotion || style?.animation === "off");
  }, [index, following, fontSize, showTranslation, reducedMotion, style?.animation]);

  return (
    <div className="grid h-full grid-cols-[300px_minmax(0,1fr)] gap-8 px-7 py-6">
      <div className="flex min-h-0 flex-col gap-4">
        <Button variant="ghost" size="sm" className="self-start" onClick={onBack}>
          <ArrowLeftIcon />
          返回
        </Button>

        <div className="relative">
          {/* 封面背后的暖色光晕：随主题走，静态、不循环。 */}
          <div className="absolute -inset-6 rounded-full bg-primary/12 blur-3xl" />
          <div className="relative aspect-square w-full overflow-hidden rounded-xl bg-muted ring-1 ring-border">
            {artwork ? (
              <img src={artwork} alt="" className="size-full object-cover" />
            ) : (
              <div className="size-full bg-gradient-to-br from-muted to-accent" />
            )}
          </div>
        </div>

        <div className="min-w-0">
          <div className="truncate text-xl font-semibold">{track?.title ?? "未在播放"}</div>
          <div className="mt-1 truncate text-sm text-muted-foreground">{track?.artist}</div>
          <div className="truncate text-xs text-muted-foreground">{track?.album}</div>
        </div>
      </div>

      <div className="relative min-h-0">
        <div
          ref={listRef}
          className="relative h-full overflow-y-auto pr-2"
          onWheel={pauseFollow}
          onTouchMove={pauseFollow}
          style={{
            overflowAnchor: "none",
            scrollBehavior: "auto",
            fontFamily: style?.font_family ? `"${style.font_family}"` : undefined,
            textAlign: style?.alignment ?? "center",
          }}
        >
        {loading ? (
          <div className="flex flex-col gap-3 pt-6">
            {Array.from({ length: 9 }).map((_, line) => (
              <Skeleton key={line} className="h-6 rounded-md" />
            ))}
          </div>
        ) : error ? (
          <div className="pt-6 text-sm text-destructive">
            <p>{error}</p>
            <Button variant="secondary" size="sm" onClick={retry}>重新加载歌词</Button>
          </div>
        ) : lines.length === 0 ? (
          <p className="pt-6 text-sm text-muted-foreground">
            {track ? "这首歌没有找到歌词。" : "先播放一首歌。"}
          </p>
        ) : (
          lines.map((line, lineIndex) => {
            const isCurrent = lineIndex === index;
            return (
              <LyricRow
                key={`${line.time_ms}-${lineIndex}`}
                line={line}
                nextTime={lines[lineIndex + 1]?.time_ms ?? line.time_ms}
                isCurrent={isCurrent}
                karaoke={karaoke}
                fontSize={fontSize}
                fontWeight={fontWeight}
                textColor={textColor}
                highlight={highlight}
                showTranslation={showTranslation}
                origin={origin}
                // 非当前行传固定值，memo 才能跳过它们的重渲染。
                positionMs={isCurrent ? position : 0}
                playing={isCurrent ? playing : false}
                offsetMs={offset}
                onSeek={onSeek}
              />
            );
          })
        )}
        </div>

      </div>
    </div>
  );
}
