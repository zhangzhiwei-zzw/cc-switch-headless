import fs from "node:fs";
import { fileURLToPath } from "node:url";
const root = new URL("../../", import.meta.url);
const contract = JSON.parse(
  fs.readFileSync(new URL("src/web-shims/command-contract.json", root), "utf8"),
);
const support = JSON.parse(
  fs.readFileSync(new URL("src/web-shims/app-support.json", root), "utf8"),
);
if (
  Object.keys(support).sort().join() !==
  Object.keys(contract.apps).sort().join()
) {
  throw new Error(
    "User support matrix must describe every contracted app exactly once",
  );
}
for (const [id, app] of Object.entries(support)) {
  for (const field of ["name", "providers", "usage", "notes"]) {
    if (typeof app[field] !== "string" || !app[field].trim()) {
      throw new Error(`${id}: missing support description ${field}`);
    }
  }
  for (const field of [
    "routing",
    "mcp",
    "skills",
    "prompts",
    "sessions",
    "deleteSessions",
  ]) {
    if (typeof app[field] !== "boolean") {
      throw new Error(`${id}: support flag ${field} must be boolean`);
    }
  }
}
const output = [
  "# Web 功能支持矩阵",
  "",
  "此文件由 `node scripts/web/generate-capabilities.mjs` 从能力契约生成。运行时以 `/api/capabilities` 为准。",
  "",
  "供应商基础管理与应用专属功能分别声明；应用出现在列表中不代表桌面专属功能均可用。",
  "",
  "| 功能 | Web 状态 | 必需命令数 |",
  "| --- | --- | --- |",
  ...Object.entries(contract.features).map(
    ([name, spec]) =>
      `| \`${name}\` | ${spec.enabled === false ? "未接入，隐藏入口" : "可用"} | ${spec.commands.length} |`,
  ),
  "",
  "| 应用 | 基础管理依赖 |",
  "| --- | --- |",
  ...Object.entries(contract.apps).map(
    ([name, spec]) =>
      `| ${name} | ${spec.commands.map((cmd) => `\`${cmd}\``).join(", ") || "通用供应商命令"} |`,
  ),
  "",
  "Antigravity 支持卡片编辑和服务器 agy CLI 模型发现；Web 不提供会话启动，卡片不会改变用户直接在 shell 启动的 agy。",
  "",
  "OpenClaw/Hermes 支持供应商基础管理；工作区、专用配置、记忆及 Web UI 启动入口仍未接入。",
  "",
  "## 参数与返回值契约",
  "",
  `当前 ${contract.validatedCommands.length} 个关键命令具有类型化调用和 Web 运行时校验；其他命令仍只有注册与覆盖检查。`,
  "",
  ...contract.validatedCommands.map((cmd) => `- \`${cmd}\``),
  "",
  "新增命令与变更契约见 [Web 接口契约](web-api-contract.md)。面向使用者的逐应用能力见 [应用支持表](web-app-support.md)。",
  "",
  "完整使用、部署与限制见 [Web 模式](web-mode-zh.md)。",
  "",
].join("\n");
const yes = (value) => (value ? "支持" : "不提供");
const userOutput = [
  "# Web 应用支持表",
  "",
  "此表描述本 fork 的 Web 功能，由 `app-support.json` 与能力契约生成。实际可用性以 `/api/capabilities` 为准。",
  "",
  "浏览器管理的是服务器上的 CLI 配置与会话。每个应用出现于侧栏，不代表它拥有桌面版的全部能力；需要先在服务器安装和运行对应 CLI 才会有会话数据。",
  "",
  "## 供应商与配置",
  "",
  "| 应用 | 供应商管理方式 | 本地路由 | MCP | Skills | 提示词 |",
  "| --- | --- | --- | --- | --- | --- |",
  ...Object.values(support).map(
    (app) =>
      `| ${app.name} | ${app.providers} | ${yes(app.routing)} | ${yes(app.mcp)} | ${yes(app.skills)} | ${yes(app.prompts)} |`,
  ),
  "",
  "本地路由可用于 Claude Code、Claude Desktop、Codex、Gemini CLI 与 Grok Build。Claude Desktop 采用模型映射，不提供完整路由/故障转移设置。Web 暂不提供 Stack 聚合。",
  "",
  "MCP 与 Skills 的勾选会写入服务器上对应 CLI 的原生配置或目录。多供应商共存表示多个配置可以同时保留，实际模型仍由 CLI 选择。",
  "",
  "## 会话与用量",
  "",
  "| 应用 | 会话阅读 | Web 删除会话 | 用量数据来源 |",
  "| --- | --- | --- | --- |",
  ...Object.values(support).map(
    (app) =>
      `| ${app.name} | ${yes(app.sessions)} | ${yes(app.deleteSessions)} | ${app.usage} |`,
  ),
  "",
  "用量列指自动导入或代理记录的来源，不表示已经接入托管账户额度。供应商余额/Coding Plan 查询取决于供应商接口、凭据和用量脚本。日志缺少模型或定价时，费用可能为 0，不表示服务免费。",
  "",
  "Kimi、MiniMax Code 和 Antigravity 的会话须在各自 CLI 中删除，以维护原生索引和状态；Web 仅提供阅读。",
  "",
  "## 应用限制",
  "",
  ...Object.values(support).map((app) => `- **${app.name}**：${app.notes}`),
  "",
  "## 共通限制",
  "",
  "- 单个实例的令牌具有管理员权限；不提供多用户、角色或租户隔离。",
  "- 托管账号中心、CLI 安装升级、终端启动、目录选择、桌面自动更新尚未接入。",
  "- OpenClaw/Hermes 的专用配置、记忆及工作区等入口未接入，不影响表中明确支持的功能。",
  "- 会话分块传输仍需后端先解析文件；它不代表服务端解析的内存占用有固定上限。",
  "",
  "[部署说明](web-mode-zh.md) · [排错指南](web-troubleshooting.md) · [开发者能力表](web-capabilities.md) · [安全边界](../SECURITY.md)",
  "",
].join("\n");
for (const [file, content] of [
  ["web-capabilities.md", output],
  ["web-app-support.md", userOutput],
]) {
  const target = new URL(`docs/${file}`, root);
  if (process.argv.includes("--check")) {
    if (!fs.existsSync(target) || fs.readFileSync(target, "utf8") !== content) {
      console.error(
        `${file} is stale. Run node scripts/web/generate-capabilities.mjs`,
      );
      process.exitCode = 1;
    }
  } else {
    fs.writeFileSync(target, content);
    console.log(fileURLToPath(target));
  }
}
