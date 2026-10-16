import {
  ArrowsDownUpIcon,
  RepeatIcon,
  RepeatOnceIcon,
  ShuffleIcon,
  MusicNotesIcon,
  PauseIcon,
  PlayIcon,
  SkipBackIcon,
  SkipForwardIcon,
  SpeakerHighIcon,
  SpeakerSlashIcon,
  TextAaIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";

import { lyricsWindow } from "@/api";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useSettings } from "@/settings-context";
import type { PlaybackMode } from "@/types";
import { formatTime, usePlayer } from "@/player-context";

const PLAYBACK_MODES = [
  { value: "sequence", label: "顺序播放", icon: ArrowsDownUpIcon },
  { value: "listLoop", label: "列表循环", icon: RepeatIcon },
  { value: "shuffle", label: "随机播放", icon: ShuffleIcon },
  { value: "singleLoop", label: "单曲循环", icon: RepeatOnceIcon },
] satisfies { value: PlaybackMode; label: string; icon: typeof RepeatIcon }[];

/// 正在出声的三条跳动竖条：用动画表达「正在播放」这个状态。
function PlayingBars() {
  return (
    <span className="flex h-3.5 items-end gap-[3px]" aria-hidden="true">
      {[0, 1, 2].map((index) => (
        <span
          key={index}
          className="player-bar-pulse w-[3px] rounded-full bg-primary"
          style={{ animationDelay: `${index * 0.16}s` }}
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

  const { settings, update } = useSettings();
  const [savingMode, setSavingMode] = useState(false);
  const modeIndex = Math.max(0, PLAYBACK_MODES.findIndex((mode) => mode.value === settings?.playback_mode));
  const mode = PLAYBACK_MODES[modeIndex];
  const nextMode = PLAYBACK_MODES[(modeIndex + 1) % PLAYBACK_MODES.length];
  const ModeIcon = mode.icon;

  async function cyclePlaybackMode() {
    setSavingMode(true);
    try {
      await update({ playback_mode: nextMode.value });
    } finally {
      setSavingMode(false);
    }
  }

  const [scrubPosition, setScrubPosition] = useState<number | null>(null);
  const track = snapshot?.current ?? null;
  const duration = track?.durationMs ?? 0;
  const position = snapshot?.position_ms ?? 0;
  const playing = snapshot?.playing ?? false;
  const volume = snapshot?.volume ?? 1;

  return (
    <footer className="grid grid-cols-[minmax(150px,1fr)_minmax(260px,1.6fr)_minmax(210px,1fr)] items-center gap-5 border-t border-border/70 bg-background px-5">
      {/* 左：当前曲目 */}
      <button
        type="button"
        onClick={onOpenNowPlaying}
        disabled={!track}
        title={track ? "查看歌曲详情和歌词" : "选择一首歌曲开始播放"}
        aria-label={track ? `查看歌词：${track.title}` : "尚未播放歌曲"}
        className="flex min-w-0 items-center gap-3 rounded-xl py-2 text-left outline-none transition-colors hover:bg-accent/60 focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default"
      >
        <div className="relative size-12 shrink-0 overflow-hidden rounded-[10px] bg-muted">
          {artwork ? (
            <img src={artwork} alt="" className="size-full object-cover" />
          ) : (
            <div className="flex size-full items-center justify-center text-muted-foreground"><MusicNotesIcon size={22} weight="duotone" /></div>
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
                : track ? `${track.artist} · 查看歌词` : "搜索音乐，或打开一个歌单"}
          </div>
        </div>
      </button>

      {/* 中：走带 */}
      <div className="flex min-w-0 flex-col items-center gap-2">
        <div className="relative flex w-full items-center justify-center gap-4">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="absolute left-0 gap-1.5 px-2 text-muted-foreground hover:text-primary"
                disabled={!settings || savingMode}
                aria-label={`播放方式：${mode.label}，点击切换为${nextMode.label}`}
                onClick={() => void cyclePlaybackMode()}
              >
                <ModeIcon /><span className="text-[11px]">{mode.label}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>当前：{mode.label} · 点击切换为{nextMode.label}</TooltipContent>
          </Tooltip>
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

        <div className="flex w-full items-center gap-3">
        <span className="tabular w-10 shrink-0 text-right text-[10px] text-muted-foreground">
          {formatTime(scrubPosition ?? position)}
        </span>
        <Slider
          className="min-w-0 flex-1"
          aria-label="播放进度"
          value={[Math.min(scrubPosition ?? position, Math.max(duration, 1))]}
          max={Math.max(duration, 1)}
          step={1000}
          disabled={!track || duration === 0}
          onValueChange={([value]) => setScrubPosition(value)}
          onValueCommit={([value]) => {
            void seek(value).finally(() => setScrubPosition(null));
          }}
        />
        <span className="tabular w-10 shrink-0 text-[10px] text-muted-foreground">
          {formatTime(duration)}
        </span>
        </div>
      </div>

      {/* 右：空间音频 / 桌面歌词 / 音量 */}
      <div className="flex items-center justify-end gap-1.5">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              className={snapshot?.spatial
                ? "bg-amber-400/15 text-amber-600 hover:bg-amber-400/25 hover:text-amber-700 dark:text-amber-300 dark:hover:bg-amber-400/25 dark:hover:text-amber-200"
                : undefined}
              aria-label="空间音效"
              aria-pressed={snapshot?.spatial ?? false}
              size="icon-sm"
              onClick={() => void setSpatial(!snapshot?.spatial)}
            >
              <WaveformIcon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>空间音效：{snapshot?.spatial ? "已开启" : "已关闭"}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="sm" aria-label="桌面歌词" title="显示或关闭桌面歌词" onClick={() => void lyricsWindow.toggle()}>
              <TextAaIcon /><span className="text-[11px]">歌词</span>
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
          className="w-[68px]"
          aria-label="音量"
          value={[Math.round(volume * 100)]}
          max={100}
          step={1}
          onValueChange={([value]) => void setVolume(value / 100)}
        />
      </div>
    </footer>
  );
}
