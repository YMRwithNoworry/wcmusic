import { useSyncExternalStore } from "react";
import { online } from "@/api";
import type { OnlineSearchChannel, Track } from "@/types";

// 页面卸载后仍保留当前会话的输入、结果和请求；重新打开无需再次搜索。
let state = { channel: "Kuwo" as OnlineSearchChannel, query: "", results: [] as Track[], loading: false, error: null as string | null };
const listeners = new Set<() => void>();
function patch(next: Partial<typeof state>) {
  state = { ...state, ...next };
  listeners.forEach((listener) => listener());
}
const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
export function useSearchSession() {
  const snapshot = useSyncExternalStore(subscribe, () => state);
  return {
    ...snapshot,
    setQuery: (query: string) => patch({ query }),
    setChannel: (channel: OnlineSearchChannel) => patch({ channel }),
    run: async () => {
      const keyword = state.query.trim();
      if (!keyword || state.loading) return;
      const channel = state.channel;
      patch({ loading: true, error: null });
      try { patch({ results: await online.search(keyword, channel) }); }
      catch (problem) { patch({ error: String(problem) }); }
      finally { patch({ loading: false }); }
    },
  };
}
