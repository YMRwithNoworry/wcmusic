import {
  PauseIcon,
  PlayIcon,
  SkipBackIcon,
  SkipForwardIcon,
  SpeakerHighIcon,
  SpeakerSlashIcon,
  TextAaIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";

import { lyricsWindow } from "@/api";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { formatTime, usePlayer } from "@/player-context";

/// 正在出声的三条跳动竖条：用动画表达「正在播放」这个状态。
function PlayingBars() {
  return (
    <span className="flex h-3.5 items-end gap-[3px]">
      {[0, 1, 2].map((index) => (
        <motion.span
          key={index}
          className="w-[3px] rounded-full bg-primary"
          animate={{ height: ["30%", "100%", "45%"] }}
          transition={{
            duration: 1.1,
            repeat: Infinity,
            ease: "easeInOut",
            delay: index * 0.16,
          }}
        />
      ))}
    </span>
  );
}

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
  const playing = snapshot?.playing ?? false;
  const volume = snapshot?.volume ?? 1;

  return (
    <footer className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-6 border-t bg-sidebar px-4">
      {/* 左：当前曲目 */}
      <button
        type="button"
        onClick={onOpenNowPlaying}
        className="flex min-w-0 items-center gap-3 rounded-[10px] px-2 py-1.5 text-left transition-colors hover:bg-accent/60 active:scale-[0.995]"
      >
        <div className="relative size-12 shrink-0 overflow-hidden rounded-[10px] bg-muted">
          {artwork ? (
            <img src={artwork} alt="" className="size-full object-cover" />
          ) : (
            <div className="size-full bg-gradient-to-br from-muted to-accent" />
          )}
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            {playing ? <PlayingBars /> : null}
            <span className="truncate text-sm font-medium">
              {track?.title ?? "未在播放"}
            </span>
          </div>
          <div className="truncate text-xs text-muted-foreground">
            {snapshot?.error
              ? snapshot.error
              : snapshot?.loading
                ? "正在解析整曲地址"
                : (track?.artist ?? "从搜索或榜单里点一首开始")}
          </div>
        </div>
      </button>

      {/* 中：走带 */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={queueLength === 0}
            onClick={() => void previous()}
            title="上一首"
          >
            <SkipBackIcon weight="fill" />
          </Button>
          <Button
            size="icon"
            className="size-10 rounded-full"
            disabled={!track}
            onClick={() => void toggle()}
            title={playing ? "暂停" : "播放"}
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.span
                key={playing ? "pause" : "play"}
                initial={{ scale: 0.6, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.6, opacity: 0 }}
                transition={{ duration: 0.14 }}
              >
                {playing ? (
                  <PauseIcon weight="fill" className="size-5" />
                ) : (
                  <PlayIcon weight="fill" className="size-5" />
                )}
              </motion.span>
            </AnimatePresence>
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={queueLength === 0}
            onClick={() => void next()}
            title="下一首"
          >
            <SkipForwardIcon weight="fill" />
          </Button>
        </div>

        <span className="tabular w-10 text-right text-[11px] text-muted-foreground">
          {formatTime(position)}
        </span>
        <Slider
          className="w-[clamp(180px,26vw,420px)]"
          value={[Math.min(position, Math.max(duration, 1))]}
          max={Math.max(duration, 1)}
          step={1000}
          disabled={!track || duration === 0}
          onValueChange={([value]) => void seek(value)}
        />
        <span className="tabular w-10 text-[11px] text-muted-foreground">
          {formatTime(duration)}
        </span>
      </div>

      {/* 右：空间音频 / 桌面歌词 / 音量 */}
      <div className="flex items-center justify-end gap-1.5">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={snapshot?.spatial ? "secondary" : "ghost"}
              size="icon-sm"
              onClick={() => void setSpatial(!snapshot?.spatial)}
            >
              <WaveformIcon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>空间音频</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-sm" onClick={() => void lyricsWindow.toggle()}>
              <TextAaIcon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>桌面歌词</TooltipContent>
        </Tooltip>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => void setVolume(volume > 0 ? 0 : 0.8)}
          title={volume > 0 ? "静音" : "恢复音量"}
        >
          {volume > 0 ? <SpeakerHighIcon /> : <SpeakerSlashIcon />}
        </Button>
        <Slider
          className="w-24"
          value={[Math.round(volume * 100)]}
          max={100}
          step={1}
          onValueChange={([value]) => void setVolume(value / 100)}
        />
      </div>
    </footer>
  );
}
