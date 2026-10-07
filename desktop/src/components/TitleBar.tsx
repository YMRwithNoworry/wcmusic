import { MinusIcon, SquareIcon, XIcon } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { Button } from "@/components/ui/button";

/// 无边框窗口的自定义标题栏：左边可拖拽，右边是最小化 / 最大化 / 关闭。
///
/// 关闭按钮走 Tauri 的 `close()`，后端把「关闭」拦成「收进托盘」。
export default function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <header
      data-tauri-drag-region
      className="flex h-9 items-center justify-between border-b bg-sidebar pl-3 select-none"
    >
      <span
        data-tauri-drag-region
        className="text-xs font-medium tracking-wide text-muted-foreground"
      >
        WCMusic
      </span>
      <div className="flex h-full">
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-full w-11 rounded-none text-muted-foreground hover:bg-accent"
          title="最小化"
          onClick={() => void appWindow.minimize()}
        >
          <MinusIcon />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-full w-11 rounded-none text-muted-foreground hover:bg-accent"
          title="最大化 / 还原"
          onClick={() => void appWindow.toggleMaximize()}
        >
          <SquareIcon />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-full w-11 rounded-none text-muted-foreground hover:bg-destructive hover:text-white"
          title="关闭到托盘"
          onClick={() => void appWindow.close()}
        >
          <XIcon />
        </Button>
      </div>
    </header>
  );
}
