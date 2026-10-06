import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { listen } from "@tauri-apps/api/event";

import { desktopWindow, lyricsWindow, player } from "./api";
import { useSettings } from "./settings-context";
import type { PlaybackSnapshot, Track } from "./types";

interface PlayerContextValue {
  snapshot: PlaybackSnapshot | null;
  /// 当前歌曲封面（data URL，后端带缓存）。
  artwork: string | null;
  /// 以整个列表为播放队列，从 `index` 开始播。
  playAt: (tracks: Track[], index: number) => Promise<void>;
  /// 只播一首（不进队列）。
  play: (track: Track) => Promise<void>;
  next: () => Promise<void>;
  previous: () => Promise<void>;
  toggle: () => Promise<void>;
  seek: (positionMs: number) => Promise<void>;
  setVolume: (volume: number) => Promise<void>;
  setSpatial: (enabled: boolean) => Promise<void>;
  queueLength: number;
  queueIndex: number;
}

const PlayerContext = createContext<PlayerContextValue | null>(null);

function trackKey(track: Track): string {
  return `${track.source}-${track.id}`;
}

export function PlayerProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();
  const [snapshot, setSnapshot] = useState<PlaybackSnapshot | null>(null);
  const [artwork, setArtwork] = useState<string | null>(null);
  const [queue, setQueue] = useState<Track[]>([]);
  const [queueIndex, setQueueIndex] = useState(-1);
  /// 已经因为「播完」自动切过的那一首，避免重复触发。
  const advanced = useRef<string | null>(null);
  const currentId = snapshot?.current?.id ?? null;

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await player.snapshot());
    } catch {
      // 后端还没起来时静默重试。
    }
  }, []);

  // 轮询进度：音频线程每 100ms 刷新一次，这里 500ms 拉一次就够顺滑。
  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 500);
    return () => window.clearInterval(timer);
  }, [refresh]);

  // 换歌时取封面（后端有落盘缓存）。
  useEffect(() => {
    const track = snapshot?.current;
    if (!track) {
      setArtwork(null);
      return;
    }
    let alive = true;
    player
      .artwork(track)
      .then((url) => {
        if (alive) {
          setArtwork(url);
        }
      })
      .catch(() => {
        if (alive) {
          setArtwork(null);
        }
      });
    return () => {
      alive = false;
    };
    // 只在换歌时重取封面。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentId]);

  const playAt = useCallback(
    async (tracks: Track[], index: number) => {
      if (index < 0 || index >= tracks.length) {
        return;
      }
      setQueue(tracks);
      setQueueIndex(index);
      advanced.current = null;
      await player.play(tracks[index]);
      await refresh();
    },
    [refresh],
  );

  const play = useCallback(
    async (track: Track) => {
      await playAt([track], 0);
    },
    [playAt],
  );

  /// 按播放方式决定下一首；`auto` 表示是「播完自动切」。
  const step = useCallback(
    async (delta: number, auto: boolean) => {
      if (queue.length === 0) {
        return;
      }
      const mode = settings?.playback_mode ?? "sequence";
      if (auto && mode === "singleLoop") {
        await playAt(queue, queueIndex);
        return;
      }
      let index = queueIndex + delta;
      if (mode === "shuffle") {
        index = Math.floor(Math.random() * queue.length);
      } else if (index >= queue.length) {
        // 顺序播完就停；列表循环回到开头。
        if (mode !== "listLoop") {
          return;
        }
        index = 0;
      } else if (index < 0) {
        index = mode === "listLoop" ? queue.length - 1 : 0;
      }
      await playAt(queue, index);
    },
    [queue, queueIndex, settings?.playback_mode, playAt],
  );

  const next = useCallback(() => step(1, false), [step]);
  const previous = useCallback(() => step(-1, false), [step]);

  // 播完自动切下一首（单曲循环时重播本首）。
  useEffect(() => {
    const track = snapshot?.current;
    if (!track || !snapshot?.finished) {
      return;
    }
    const key = trackKey(track);
    if (advanced.current === key) {
      return;
    }
    advanced.current = key;
    void step(1, true);
  }, [snapshot?.finished, snapshot?.current, step]);

  // 全局快捷键：后端用 Win32 注册，动作以事件发到前端，由这里执行。
  const lastVolume = useRef(0.8);
  if ((snapshot?.volume ?? 0) > 0) {
    lastVolume.current = snapshot?.volume ?? lastVolume.current;
  }
  const hotkeyHandler = useRef<(action: string) => void>(() => {});
  hotkeyHandler.current = (action: string) => {
    const volume = snapshot?.volume ?? 1;
    const position = snapshot?.position_ms ?? 0;
    switch (action) {
      case "previous":
        void step(-1, false);
        break;
      case "next":
        void step(1, false);
        break;
      case "toggle_play":
        void player.toggle().then(refresh);
        break;
      case "volume_up":
        void player.setVolume(Math.min(1, volume + 0.05)).then(refresh);
        break;
      case "volume_down":
        void player.setVolume(Math.max(0, volume - 0.05)).then(refresh);
        break;
      case "mute":
        void player.setVolume(volume > 0 ? 0 : lastVolume.current).then(refresh);
        break;
      case "seek_forward":
        void player.seek(position + 5000).then(refresh);
        break;
      case "seek_backward":
        void player.seek(Math.max(0, position - 5000)).then(refresh);
        break;
      case "toggle_window":
        void desktopWindow.toggleMain();
        break;
      case "toggle_lyrics":
        void lyricsWindow.toggle();
        break;
      default:
        break;
    }
  };

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void listen<string>("wcmusic://hotkey", (event) => {
      hotkeyHandler.current(event.payload);
    }).then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const value = useMemo<PlayerContextValue>(
    () => ({
      snapshot,
      artwork,
      playAt,
      play,
      next,
      previous,
      toggle: async () => {
        await player.toggle();
        await refresh();
      },
      seek: async (positionMs) => {
        await player.seek(positionMs);
        await refresh();
      },
      setVolume: async (volume) => {
        await player.setVolume(volume);
        await refresh();
      },
      setSpatial: async (enabled) => {
        await player.setSpatial(enabled);
        await refresh();
      },
      queueLength: queue.length,
      queueIndex,
    }),
    [
      snapshot,
      artwork,
      playAt,
      play,
      next,
      previous,
      refresh,
      queue.length,
      queueIndex,
    ],
  );

  return <PlayerContext.Provider value={value}>{children}</PlayerContext.Provider>;
}

export function usePlayer(): PlayerContextValue {
  const value = useContext(PlayerContext);
  if (!value) {
    throw new Error("usePlayer 必须在 PlayerProvider 内使用");
  }
  return value;
}

/// 毫秒转 `m:ss`。
export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
