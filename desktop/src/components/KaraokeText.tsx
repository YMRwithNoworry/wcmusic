import type { CSSProperties } from "react";

import { karaokeGradient, useSmoothPosition } from "@/lib/lyrics";

interface Props {
  text: string;
  /// 本行与下一行的时间戳（毫秒），用来算这一行的填充进度。
  lineTime: number;
  nextTime: number;
  /// 轮询到的播放位置与播放状态。
  positionMs: number;
  playing: boolean;
  offsetMs: number;
  highlight: string;
  textColor: string;
  className?: string;
  style?: CSSProperties;
}

/// 逐字填充的文字。
///
/// 自己用 rAF 把播放位置外推到每一帧，因此每帧只重渲染这一个元素；
/// 外层歌词列表不会跟着播放位置刷新而重渲染。
export default function KaraokeText({
  text,
  lineTime,
  nextTime,
  positionMs,
  playing,
  offsetMs,
  highlight,
  textColor,
  className,
  style,
}: Props) {
  const position = useSmoothPosition(positionMs, playing, offsetMs);
  const span = nextTime - lineTime;
  const progress = span > 0 ? Math.min(1, Math.max(0, (position - lineTime) / span)) : 0;

  return (
    <span
      className={className}
      style={{
        ...style,
        color: "transparent",
        backgroundImage: karaokeGradient(highlight, textColor, progress),
        WebkitBackgroundClip: "text",
        backgroundClip: "text",
      }}
    >
      {text || "♪"}
    </span>
  );
}
