import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { codeInspectorPlugin } from "code-inspector-plugin";

/**
 * web 构建（由 `cc-switch-server` 托管、浏览器访问）把 `@tauri-apps/*` 换成
 * `src/web-shims/` 里的浏览器实现。
 *
 * 桌面构建不加任何 alias，解析结果与改造前完全一致。
 */
const webBuild = process.env.CC_SWITCH_WEB === "1";

const webAliases: Record<string, string> = webBuild
  ? {
      "@tauri-apps/api/core": path.resolve(__dirname, "./src/web-shims/core.ts"),
      "@tauri-apps/api/event": path.resolve(__dirname, "./src/web-shims/event.ts"),
      "@tauri-apps/api/window": path.resolve(__dirname, "./src/web-shims/window.ts"),
      "@tauri-apps/api/app": path.resolve(__dirname, "./src/web-shims/app.ts"),
      "@tauri-apps/api/path": path.resolve(__dirname, "./src/web-shims/path.ts"),
      "@tauri-apps/plugin-dialog": path.resolve(
        __dirname,
        "./src/web-shims/dialog.ts",
      ),
      "@tauri-apps/plugin-process": path.resolve(
        __dirname,
        "./src/web-shims/process.ts",
      ),
      "@tauri-apps/plugin-updater": path.resolve(
        __dirname,
        "./src/web-shims/updater.ts",
      ),
      "@tauri-apps/plugin-log": path.resolve(__dirname, "./src/web-shims/log.ts"),
    }
  : {};

export default defineConfig(({ command }) => ({
  root: "src",
  plugins: [
    command === "serve" &&
      codeInspectorPlugin({
        bundler: "vite",
      }),
    react(),
  ].filter(Boolean),
  base: "./",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  server: {
    port: 3000,
    strictPort: true,
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      ...webAliases,
    },
  },
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
}));
