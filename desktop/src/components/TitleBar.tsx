import { getCurrentWindow } from "@tauri-apps/api/window";

/// 无边框窗口的自定义标题栏：左边可拖拽，右边是最小化 / 最大化 / 关闭。
///
/// 关闭按钮走 Tauri 的 `close()`，后端把「关闭」拦成「收进托盘」。
export default function TitleBar() {
  const window = getCurrentWindow();

  return (
    <header className="titlebar" data-tauri-drag-region>
      <span className="titlebar-title" data-tauri-drag-region>
        WCMusic
      </span>
      <div className="titlebar-buttons">
        <button title="最小化" onClick={() => void window.minimize()}>
          ─
        </button>
        <button title="最大化 / 还原" onClick={() => void window.toggleMaximize()}>
          ▢
        </button>
        <button className="close" title="关闭到托盘" onClick={() => void window.close()}>
          ✕
        </button>
      </div>
    </header>
  );
}
