import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { gzip } from "node:zlib";
import { promisify } from "node:util";
import react from "@vitejs/plugin-react";
import { codeInspectorPlugin } from "code-inspector-plugin";

/**
 * web 构建（由 `cc-switch-server` 托管、浏览器访问）把 `@tauri-apps/*` 换成
 * `src/web-shims/` 里的浏览器实现。
 *
 * 桌面构建不加任何 alias，解析结果与改造前完全一致。
 */
const webBuild = process.env.CC_SWITCH_WEB === "1";

/** Generate sidecars in the same build for CLI, Docker and Windows releases. */
function precompressWeb(): Plugin {
  let output = "";
  const compress = promisify(gzip);
  return {
    name: "cc-switch-web-precompress",
    apply: "build",
    configResolved(config) {
      output = path.resolve(config.root, config.build.outDir);
    },
    async closeBundle() {
      async function visit(dir: string): Promise<void> {
        // Sequential compression bounds peak build memory.
        for (const entry of await readdir(dir, { withFileTypes: true })) {
          const file = path.join(dir, entry.name);
          if (entry.isDirectory()) await visit(file);
          else if (
            entry.isFile() &&
            /\.(html|js|css|json|svg)$/.test(entry.name)
          ) {
            const raw = await readFile(file);
            if (raw.length < 1024) continue;
            const compressed = await compress(raw, { level: 6 });
            if (compressed.length < raw.length)
              await writeFile(`${file}.gz`, compressed);
          }
        }
      }
      await visit(output);
    },
  };
}

const webAliases: Record<string, string> = webBuild
  ? {
      "@tauri-apps/api/core": path.resolve(
        __dirname,
        "./src/web-shims/core.ts",
      ),
      "@tauri-apps/api/event": path.resolve(
        __dirname,
        "./src/web-shims/event.ts",
      ),
      "@tauri-apps/api/window": path.resolve(
        __dirname,
        "./src/web-shims/window.ts",
      ),
      "@tauri-apps/api/app": path.resolve(__dirname, "./src/web-shims/app.ts"),
      "@tauri-apps/api/path": path.resolve(
        __dirname,
        "./src/web-shims/path.ts",
      ),
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
      "@tauri-apps/plugin-log": path.resolve(
        __dirname,
        "./src/web-shims/log.ts",
      ),
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
    webBuild && precompressWeb(),
  ].filter(Boolean),
  base: "./",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      // 托盘面板是单独的入口，不加载主界面
      input: {
        main: path.resolve(__dirname, "src/index.html"),
        ...(!webBuild
          ? { "tray-panel": path.resolve(__dirname, "src/tray-panel.html") }
          : {}),
      },
    },
  },
  server: {
    port: 3000,
    strictPort: true,
    ...(webBuild
      ? {
          proxy: Object.fromEntries(
            ["/api", "/auth", "/healthz", "/readyz"].map((path) => [
              path,
              "http://127.0.0.1:15800",
            ]),
          ),
        }
      : {}),
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
