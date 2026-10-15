import { player } from "@/api";
import type { Track } from "@/types";

const requests = new Map<string, Promise<string | null>>();
const waiting: (() => void)[] = [];
let active = 0;

async function download(track: Track, refreshUri: boolean): Promise<string | null> {
  if (active >= 4) await new Promise<void>((resolve) => waiting.push(resolve));
  else active++;
  try {
    return await player.artwork(track, refreshUri);
  } finally {
    const next = waiting.shift();
    if (next) next();
    else active--;
  }
}

// 列表与播放器共用请求；失败不长期缓存，切页后可以重试。
export function loadTrackArtwork(track: Track, refreshUri = false): Promise<string | null> {
  const key = JSON.stringify([track.source, track.sourceId ?? track.id, track.artworkUri, refreshUri]);
  const existing = requests.get(key);
  if (existing) return existing;
  const request = download(track, refreshUri).then((url) => {
    if (!url) requests.delete(key);
    return url;
  }, (error) => {
    requests.delete(key);
    throw error;
  });
  requests.set(key, request);
  if (requests.size > 256) requests.delete(requests.keys().next().value!);
  return request;
}
