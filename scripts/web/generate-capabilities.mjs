import fs from "node:fs";
import { fileURLToPath } from "node:url";
const root = new URL("../../", import.meta.url);
const contract = JSON.parse(fs.readFileSync(new URL("src/web-shims/command-contract.json", root), "utf8"));
const output = [
  "# Web 功能支持矩阵", "", "此文件由 `node scripts/web/generate-capabilities.mjs` 从能力契约生成。运行时以 `/api/capabilities` 为准。", "",
  "供应商基础管理与应用专属功能分别声明；应用出现在列表中不代表桌面专属功能均可用。", "",
  "| 功能 | Web 状态 | 必需命令数 |", "| --- | --- | --- |",
  ...Object.entries(contract.features).map(([name, spec]) => `| \`${name}\` | ${spec.enabled === false ? "未接入，隐藏入口" : "可用"} | ${spec.commands.length} |`),
  "", "| 应用 | 基础管理依赖 |", "| --- | --- |",
  ...Object.entries(contract.apps).map(([name, spec]) => `| ${name} | ${spec.commands.map((cmd) => `\`${cmd}\``).join(", ") || "通用供应商命令"} |`),
  "", "Antigravity 支持卡片编辑和服务器 agy CLI 模型发现；Web 不提供会话启动，卡片不会改变用户直接在 shell 启动的 agy。", "",
  "OpenClaw/Hermes 支持供应商基础管理；工作区、专用配置、记忆及 Web UI 启动入口仍未接入。", "",
  "完整使用、部署与限制见 [Web 模式](web-mode-zh.md)。", "",
].join("\n");
const target = new URL("docs/web-capabilities.md", root);
if (process.argv.includes("--check")) {
  if (!fs.existsSync(target) || fs.readFileSync(target, "utf8") !== output) {
    console.error("Web capability documentation is stale. Run node scripts/web/generate-capabilities.mjs");
    process.exitCode = 1;
  }
} else { fs.writeFileSync(target, output); console.log(fileURLToPath(target)); }
