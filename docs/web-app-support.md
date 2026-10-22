# Web 应用支持表

此表描述本 fork 的 Web 功能，由 `app-support.json` 与能力契约生成。实际可用性以 `/api/capabilities` 为准。

浏览器管理的是服务器上的 CLI 配置与会话。每个应用出现于侧栏，不代表它拥有桌面版的全部能力；需要先在服务器安装和运行对应 CLI 才会有会话数据。

## 供应商与配置

| 应用 | 供应商管理方式 | 本地路由 | MCP | Skills | 提示词 |
| --- | --- | --- | --- | --- | --- |
| Claude Code | 单供应商切换 | 支持 | 支持 | 支持 | 支持 |
| Claude Desktop | 直连 / 模型映射 | 支持 | 不提供 | 不提供 | 不提供 |
| Codex | 单供应商切换 | 支持 | 支持 | 支持 | 支持 |
| Gemini CLI | 单供应商切换 | 支持 | 支持 | 支持 | 支持 |
| Grok Build | 单供应商切换 | 支持 | 支持 | 支持 | 支持 |
| OpenCode | 多供应商共存 | 不提供 | 支持 | 支持 | 支持 |
| OpenClaw | 多供应商共存 | 不提供 | 不提供 | 支持 | 支持 |
| Hermes Agent | 多供应商共存 | 不提供 | 支持 | 支持 | 支持 |
| Pi | 多供应商共存 | 不提供 | 支持 | 支持 | 支持 |
| MiniMax Code | 多供应商共存 | 不提供 | 支持 | 支持 | 支持 |
| Kimi Code | 多供应商共存 | 不提供 | 支持 | 支持 | 支持 |
| Antigravity（agy） | 卡片编辑、模型发现 | 不提供 | 支持 | 支持 | 支持 |

本地路由可用于 Claude Code、Claude Desktop、Codex、Gemini CLI 与 Grok Build。Claude Desktop 采用模型映射，不提供完整路由/故障转移设置。Web 暂不提供 Stack 聚合。

MCP 与 Skills 的勾选会写入服务器上对应 CLI 的原生配置或目录。多供应商共存表示多个配置可以同时保留，实际模型仍由 CLI 选择。

## 会话与用量

| 应用 | 会话阅读 | Web 删除会话 | 用量数据来源 |
| --- | --- | --- | --- |
| Claude Code | 支持 | 支持 | 会话日志、代理请求 |
| Claude Desktop | 不提供 | 不提供 | 代理请求 |
| Codex | 支持 | 支持 | 会话日志、代理请求 |
| Gemini CLI | 支持 | 支持 | 会话日志、代理请求 |
| Grok Build | 支持 | 支持 | 会话日志、代理请求 |
| OpenCode | 支持 | 支持 | 会话日志 |
| OpenClaw | 支持 | 支持 | 未接入会话日志导入 |
| Hermes Agent | 支持 | 支持 | 未接入会话日志导入 |
| Pi | 支持 | 支持 | 会话日志 |
| MiniMax Code | 支持 | 不提供 | 会话日志 |
| Kimi Code | 支持 | 不提供 | 会话日志 |
| Antigravity（agy） | 支持 | 不提供 | 会话日志 |

用量列指自动导入或代理记录的来源，不表示已经接入托管账户额度。供应商余额/Coding Plan 查询取决于供应商接口、凭据和用量脚本。日志缺少模型或定价时，费用可能为 0，不表示服务免费。

Kimi、MiniMax Code 和 Antigravity 的会话须在各自 CLI 中删除，以维护原生索引和状态；Web 仅提供阅读。

## 应用限制

- **Claude Code**：修改服务端的 Claude 配置。
- **Claude Desktop**：模型映射经路由服务；不提供完整故障转移设置。
- **Codex**：托管 OAuth 账号中心未接入 Web；已有服务端 CLI 登录由 CLI 管理。
- **Gemini CLI**：与 Antigravity 是独立应用。
- **Grok Build**：xAI 托管登录与托管额度查询未接入 Web。
- **OpenCode**：可读写顶层默认模型；OMO 与 OMO Slim 未接入 Web。
- **OpenClaw**：支持供应商、默认模型和模型目录读取；工作区及专用配置入口未接入。
- **Hermes Agent**：默认隐藏，可在设置中启用；记忆管理、Dashboard 启动未接入。
- **Pi**：支持原生启用状态、用量脚本、SYSTEM.md 与提示词模板；会话目录可能需要项目上下文。
- **MiniMax Code**：会话删除交给 MCode，避免破坏原生状态；全局提示词有大小限制。
- **Kimi Code**：默认模型在 CLI 内用 /model 选择；OAuth 登录与会话删除交给 Kimi Code。
- **Antigravity（agy）**：模型发现需要服务器安装 agy；Web 不启动会话，卡片不改变直接在 shell 启动的 agy。

## 共通限制

- 单个实例的令牌具有管理员权限；不提供多用户、角色或租户隔离。
- 托管账号中心、CLI 安装升级、终端启动、目录选择、桌面自动更新尚未接入。
- OpenClaw/Hermes 的专用配置、记忆及工作区等入口未接入，不影响表中明确支持的功能。
- 会话分块传输仍需后端先解析文件；它不代表服务端解析的内存占用有固定上限。

[部署说明](web-mode-zh.md) · [排错指南](web-troubleshooting.md) · [开发者能力表](web-capabilities.md) · [安全边界](../SECURITY.md)
