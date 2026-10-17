import { MinusIcon, SquareIcon, XIcon } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { Button } from "@/components/ui/button";

/// 无边框窗口的自定义标题栏：左边可拖拽，右边是最小化 / 最大化 / 关闭。
///
/// 关闭按钮走 Tauri 的 `close()`，后端隐藏主窗口并保留托盘和播放。
export default function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <header
      data-tauri-drag-region
      className="flex h-[38px] items-center justify-between bg-sidebar pl-5 text-sidebar-foreground select-none"
    >
      <span
        data-tauri-drag-region
        className="text-[11px] font-medium tracking-[0.12em] text-sidebar-foreground/60"
      >
        WCMusic
      </span>
      <div className="flex h-full">
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-full w-11 rounded-none text-sidebar-foreground/60 hover:bg-sidebar-accent hover:text-sidebar-foreground"
          title="最小化"
          onClick={() => void appWindow.minimize()}
        >
          <MinusIcon />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-full w-11 rounded-none text-sidebar-foreground/60 hover:bg-sidebar-accent hover:text-sidebar-foreground"
          title="最大化 / 还原"
          onClick={() => void appWindow.toggleMaximize()}
        >
          <SquareIcon />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-full w-11 rounded-none text-sidebar-foreground/60 hover:bg-destructive hover:text-white"
          title="关闭到托盘"
          onClick={() => void appWindow.close()}
        >
          <XIcon />
        </Button>
      </div>
    </header>
  );
}
