import { ArrowLeftIcon } from "@phosphor-icons/react";
import { motion } from "motion/react";
import { memo, useCallback, useEffect, useRef, useState } from "react";

import { lyrics as lyricsApi } from "@/api";
import KaraokeText from "@/components/KaraokeText";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useCurrentLine } from "@/lib/lyrics";
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
  return (
    <motion.div
      // 当前行放大、变亮；其它行缩小、变暗。用缓动而不是直接跳变。
      animate={{ opacity: isCurrent ? 1 : 0.36, scale: isCurrent ? 1.08 : 0.9 }}
      transition={{ duration: 0.5, ease: [0.16, 1, 0.3, 1] }}
      onClick={() => onSeek(line.time_ms)}
      className={`cursor-pointer py-1.5 ${origin} ${isCurrent ? "lyric-line-current" : ""}`}
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
  const { snapshot, artwork, seek } = usePlayer();
  const track = snapshot?.current ?? null;
  const [lines, setLines] = useState<LyricLine[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /// 是否跟随当前行自动滚动；用户手动滚动后暂停，过几秒再恢复。
  const [following, setFollowing] = useState(true);
  const listRef = useRef<HTMLDivElement | null>(null);
  const resumeTimer = useRef<number | null>(null);

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
  const position = snapshot?.position_ms ?? 0;
  const playing = snapshot?.playing ?? false;
  const index = useCurrentLine(lines, position, playing, offset);

  const textColor = toCss(style?.text_color ?? 0xffffff, style?.text_alpha ?? 1);
  const highlight = toCss(style?.highlight_color ?? 0xffffff, style?.highlight_alpha ?? 1);
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

  const onSeek = useCallback((timeMs: number) => void seek(Math.max(0, timeMs - offset)), [seek, offset]);

  /// 用户开始自己滚动：先停止跟随，停手 5 秒后再接回。
  const pauseFollow = useCallback(() => {
    setFollowing(false);
    if (resumeTimer.current !== null) {
      window.clearTimeout(resumeTimer.current);
    }
    resumeTimer.current = window.setTimeout(() => setFollowing(true), 5000);
  }, []);

  const resumeFollow = useCallback(() => {
    if (resumeTimer.current !== null) {
      window.clearTimeout(resumeTimer.current);
    }
    setFollowing(true);
  }, []);

  useEffect(
    () => () => {
      if (resumeTimer.current !== null) {
        window.clearTimeout(resumeTimer.current);
      }
    },
    [],
  );

  // 当前行居中：只在自己跟随时滚，且已经在中间就不重复发起平滑滚动。
  useEffect(() => {
    if (!following) {
      return;
    }
    const container = listRef.current;
    const line = container?.querySelector<HTMLElement>(".lyric-line-current");
    if (!container || !line) {
      return;
    }
    const containerRect = container.getBoundingClientRect();
    const lineRect = line.getBoundingClientRect();
    const delta =
      lineRect.top + lineRect.height / 2 - (containerRect.top + containerRect.height / 2);
    if (Math.abs(delta) < 4) {
      return;
    }
    container.scrollTo({ top: container.scrollTop + delta, behavior: "smooth" });
  }, [index, following]);

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
          className="h-full overflow-y-auto pr-2"
          onWheel={pauseFollow}
          onTouchStart={pauseFollow}
          onPointerDown={pauseFollow}
          style={{
            paddingBottom: "38vh",
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
          <p className="pt-6 text-sm text-destructive">{error}</p>
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

        {!following ? (
          <button
            type="button"
            onClick={resumeFollow}
            className="absolute bottom-3 left-1/2 -translate-x-1/2 rounded-full border bg-card/90 px-3 py-1 text-xs text-foreground shadow-sm backdrop-blur transition-colors hover:bg-accent"
          >
            回到当前
          </button>
        ) : null}
      </div>
    </div>
  );
}
