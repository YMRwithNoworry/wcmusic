import { useEffect, useRef, useState, type RefObject } from "react";

import type { LyricLine } from "@/types";

/// 当前行下标：最后一条 `time_ms <= position` 的歌词。
export function currentIndex(lines: LyricLine[], position: number): number {
  let index = -1;
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i].time_ms > position) {
      break;
    }
    index = i;
  }
  return index;
}

/// 逐字填充的渐变：高亮色从左侧按 `progress` 铺开，右侧仍是底色。
export function karaokeGradient(highlight: string, textColor: string, progress: number): string {
  const percent = Math.min(100, Math.max(0, progress * 100));
  return `linear-gradient(90deg, ${highlight} ${percent}%, ${textColor} ${percent}%)`;
}

/// 播放头基准：每次轮询记下「真实位置 + 采样时刻 + 是否在播」，
/// rAF 里据此外推出两次轮询之间的位置。
interface Playhead {
  position: number;
  at: number;
  playing: boolean;
}

function usePlayhead(
  positionMs: number,
  playing: boolean,
  offsetMs: number,
): RefObject<Playhead> {
  const base = useRef<Playhead>({ position: 0, at: 0, playing: false });
  useEffect(() => {
    base.current = {
      position: Math.max(0, positionMs + offsetMs),
      at: performance.now(),
      playing,
    };
  }, [positionMs, playing, offsetMs]);
  return base;
}

/// 每帧外推出的播放位置。
///
/// 后端每 500ms 才给一次位置，直接拿它做逐字填充只能 500ms 跳一格；
/// 这里按真实时间外推，填充就能每帧推进。调用方每帧重渲染一次，
/// 所以只该给「当前行」这种小组件用。
export function useSmoothPosition(
  positionMs: number,
  playing: boolean,
  offsetMs: number,
): number {
  const base = usePlayhead(positionMs, playing, offsetMs);
  const [position, setPosition] = useState(0);

  useEffect(() => {
    let frame = 0;
    const tick = () => {
      const { position, at, playing: isPlaying } = base.current;
      setPosition(isPlaying ? position + (performance.now() - at) : position);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [base]);

  return position;
}

/// 当前行下标（按外推后的位置算，换行不会被 500ms 的轮询拖慢）。
///
/// 只在真正换行时才 `setState`，播放位置刷新不会带动整页歌词重渲染。
export function useCurrentLine(
  lines: LyricLine[],
  positionMs: number,
  playing: boolean,
  offsetMs: number,
): number {
  const base = usePlayhead(positionMs, playing, offsetMs);
  const [index, setIndex] = useState(-1);
  const indexRef = useRef(-1);

  useEffect(() => {
    if (lines.length === 0) {
      if (indexRef.current !== -1) {
        indexRef.current = -1;
        setIndex(-1);
      }
      return;
    }

    let frame = 0;
    const tick = () => {
      const { position, at, playing: isPlaying } = base.current;
      const now = isPlaying ? position + (performance.now() - at) : position;
      const next = currentIndex(lines, now);
      if (next !== indexRef.current) {
        indexRef.current = next;
        setIndex(next);
      }
      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [lines, base]);

  return index;
}
