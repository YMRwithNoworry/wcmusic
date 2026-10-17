import type { AppSettings } from "@/types";

export const DEFAULT_FAVORITE_FOLDERS = ["试听列表", "我的收藏", "最近播放", "通勤"] as const;

/// 合并内置、已创建和旧设置里出现过的文件夹，避免升级后丢掉历史歌曲分组。
export function favoriteFolderNames(settings: AppSettings | null): string[] {
  const names = [
    ...DEFAULT_FAVORITE_FOLDERS,
    ...(settings?.saved_folders ?? []),
    ...(settings?.saved_tracks ?? []).map((saved) => saved.folder),
  ];
  return [...new Set(names.map((name) => name.trim()).filter(Boolean))];
}
