import { useEffect, useRef, useState } from "react";
import { lyrics } from "@/api";
import type { LyricLine, Track } from "@/types";

// 同一窗口的详情页重新打开时复用正文；失败请求移除，允许再次尝试。
const requests = new Map<string, Promise<LyricLine[]>>();
function load(track: Track, key: string) {
  let request = requests.get(key);
  if (!request) {
    if (requests.size >= 64) requests.delete(requests.keys().next().value!);
    request = lyrics.load(track).catch((error) => {
      requests.delete(key);
      throw error;
    });
    requests.set(key, request);
  }
  return request;
}

export function useTrackLyrics(track: Track | null, translate: boolean, useProxy: boolean) {
  const [lines, setLines] = useState<LyricLine[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const trackRef = useRef(track);
  trackRef.current = track;
  const key = track ? `${track.source}-${track.sourceId ?? track.id}-${useProxy}` : null;

  useEffect(() => {
    let alive = true;
    setLines([]);
    setError(null);
    if (!key || !trackRef.current) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void load(trackRef.current, key)
      .then((body) => {
        if (!alive) return;
        setLines(body);
        setLoading(false);
        // 正文已经显示后再请求翻译；翻译失败保留正文。
        if (translate && body.length > 0 && body.every((line) => !line.translation)) {
          void lyrics.translate(body)
            .then((translated) => { if (alive) setLines(translated); })
            .catch((problem) => console.warn("[lyrics translation]", problem));
        }
      })
      .catch((problem) => {
        if (alive) {
          setError(String(problem));
          setLoading(false);
        }
      });
    return () => { alive = false; };
  }, [key, translate, retry]);

  return { lines, loading, error, retry: () => {
    if (key) requests.delete(key);
    setRetry((value) => value + 1);
  } };
}

// 歌曲详情页采用页面背景的对比度，桌面歌词仍使用用户设置的原始颜色。
export function pageLyricColor(rgb: number, alpha: number, dark: boolean): string {
  const linear = (value: number) => {
    value /= 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * linear((rgb >> 16) & 255)
    + 0.7152 * linear((rgb >> 8) & 255) + 0.0722 * linear(rgb & 255);
  const background = dark ? 0.015 : 0.96;
  const contrast = (Math.max(luminance, background) + 0.05) / (Math.min(luminance, background) + 0.05);
  if (contrast < 3) return "var(--foreground)";
  return `rgba(${(rgb >> 16) & 255}, ${(rgb >> 8) & 255}, ${rgb & 255}, ${Math.max(0.5, alpha)})`;
}
