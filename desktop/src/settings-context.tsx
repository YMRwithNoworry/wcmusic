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

  // 深浅主题跟着设置走：CSS 里用 :root[data-theme="light"] 覆盖变量。
  useEffect(() => {
    document.documentElement.dataset.theme =
      settings?.dark_theme === false ? "light" : "dark";
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

  const value = useMemo(
    () => ({ settings, update, reload }),
    [settings, update, reload],
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
