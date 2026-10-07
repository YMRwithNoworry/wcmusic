import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { api } from "./api";
import type { AppSettings } from "./types";

interface SettingsContextValue {
  settings: AppSettings | null;
  /// 合并式修改：改完立刻落盘，以后端返回的设置为准。
  update: (patch: Partial<AppSettings>) => Promise<void>;
  /// 用后端返回的整份设置替换本地状态（收藏类命令整份返回）。
  replace: (next: AppSettings) => void;
  reload: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings | null>(null);

  const reload = useCallback(async () => {
    setSettings(await api.getSettings());
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  // 深浅主题：Tailwind v4 的 dark 变体看 <html class="dark">。
  useEffect(() => {
    const dark = settings?.dark_theme !== false;
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  }, [settings?.dark_theme]);

  const update = useCallback(
    async (patch: Partial<AppSettings>) => {
      if (!settings) {
        return;
      }
      const next = { ...settings, ...patch };
      setSettings(next);
      setSettings(await api.saveSettings(next));
    },
    [settings],
  );

  const replace = useCallback((next: AppSettings) => setSettings(next), []);

  const value = useMemo(
    () => ({ settings, update, replace, reload }),
    [settings, update, replace, reload],
  );

  return (
    <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>
  );
}

export function useSettings(): SettingsContextValue {
  const value = useContext(SettingsContext);
  if (!value) {
    throw new Error("useSettings 必须在 SettingsProvider 内使用");
  }
  return value;
}
