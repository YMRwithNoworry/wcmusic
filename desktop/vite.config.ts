import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 固定用 1420 端口；`src-tauri` 由 cargo 自己监听，不需要 vite 再看。
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    // shadcn 生成的组件用 `@/` 前缀导入。
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // WebView2 常驻更新，直接按现代 Chromium 打包，省掉多余的降级代码。
    target: "chrome120",
    sourcemap: false,
  },
});
