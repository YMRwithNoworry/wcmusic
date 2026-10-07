import { ArrowLeftIcon } from "@phosphor-icons/react";
import { motion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { lyrics as lyricsApi } from "@/api";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
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
    listRef.current
      ?.querySelector(".lyric-line-current")
      ?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [index]);

  const textColor = toCss(style?.text_color ?? 0xffffff, style?.text_alpha ?? 1);
  const highlight = toCss(style?.highlight_color ?? 0xffffff, style?.highlight_alpha ?? 1);
  const karaoke = style?.karaoke ?? false;

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

      <div
        ref={listRef}
        className="min-h-0 overflow-y-auto pr-2"
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
              <motion.div
                key={`${line.time_ms}-${lineIndex}`}
                animate={{
                  opacity: isCurrent ? 1 : 0.42,
                  scale: isCurrent ? 1 : 0.985,
                }}
                transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
                onClick={() => void seek(Math.max(0, line.time_ms - offset))}
                className={
                  isCurrent
                    ? "lyric-line-current cursor-pointer origin-left py-1.5"
                    : "cursor-pointer origin-left py-1.5"
                }
                style={{ fontSize: `${style?.font_size ?? 28}px` }}
              >
                <span
                  className="inline-block leading-snug"
                  style={{
                    fontWeight: style?.font_weight ?? 500,
                    color: isCurrent && !karaoke ? highlight : textColor,
                    ...karaokeStyle,
                  }}
                >
                  {line.text || "♪"}
                </span>
                {style?.show_translation && line.translation ? (
                  <div
                    className="leading-snug"
                    style={{ fontSize: "0.6em", color: textColor, opacity: 0.75 }}
                  >
                    {line.translation}
                  </div>
                ) : null}
              </motion.div>
            );
          })
        )}
      </div>
    </div>
  );
}
