import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { player } from "./api";
import type { PlaybackSnapshot, Track } from "./types";

interface PlayerContextValue {
  snapshot: PlaybackSnapshot | null;
  /// 当前歌曲封面（data URL，后端带缓存）。
  artwork: string | null;
  play: (track: Track) => Promise<void>;
  toggle: () => Promise<void>;
  seek: (positionMs: number) => Promise<void>;
  setVolume: (volume: number) => Promise<void>;
  setSpatial: (enabled: boolean) => Promise<void>;
}

const PlayerContext = createContext<PlayerContextValue | null>(null);

export function PlayerProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<PlaybackSnapshot | null>(null);
  const [artwork, setArtwork] = useState<string | null>(null);
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

  const value = useMemo<PlayerContextValue>(
    () => ({
      snapshot,
      artwork,
      play: async (track) => {
        await player.play(track);
        await refresh();
      },
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
    }),
    [snapshot, artwork, refresh],
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
