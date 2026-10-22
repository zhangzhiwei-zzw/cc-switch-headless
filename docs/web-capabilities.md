# Web 功能支持矩阵

此文件由 `node scripts/web/generate-capabilities.mjs` 从能力契约生成。运行时以 `/api/capabilities` 为准。

供应商基础管理与应用专属功能分别声明；应用出现在列表中不代表桌面专属功能均可用。

| 功能 | Web 状态 | 必需命令数 |
| --- | --- | --- |
| `pageProviders` | 可用 | 13 |
| `pageRouting` | 可用 | 11 |
| `pageSessions` | 可用 | 6 |
| `pageImportExport` | 可用 | 5 |
| `pageMcp` | 可用 | 15 |
| `pageSkills` | 可用 | 27 |
| `pagePrompts` | 可用 | 13 |
| `pageUsage` | 可用 | 24 |
| `sessionStream` | 可用 | 1 |
| `pageAuth` | 未接入，隐藏入口 | 0 |
| `pageApps` | 未接入，隐藏入口 | 0 |
| `sessionReveal` | 未接入，隐藏入口 | 0 |
| `sessionTerminal` | 未接入，隐藏入口 | 0 |
| `pickDirectory` | 未接入，隐藏入口 | 0 |
| `openInFileManager` | 未接入，隐藏入口 | 0 |
| `tray` | 未接入，隐藏入口 | 0 |
| `updater` | 未接入，隐藏入口 | 0 |
| `stack` | 未接入，隐藏入口 | 0 |
| `omo` | 未接入，隐藏入口 | 0 |
| `workspace` | 未接入，隐藏入口 | 0 |
| `openclawConfig` | 未接入，隐藏入口 | 0 |
| `hermesMemory` | 未接入，隐藏入口 | 0 |
| `profiles` | 未接入，隐藏入口 | 0 |
| `cloudSync` | 未接入，隐藏入口 | 0 |
| `providerConnectivity` | 未接入，隐藏入口 | 0 |
| `providerTerminal` | 未接入，隐藏入口 | 0 |
| `globalProxy` | 未接入，隐藏入口 | 0 |
| `rectifier` | 未接入，隐藏入口 | 0 |
| `proxyClientHost` | 未接入，隐藏入口 | 0 |
| `settingsLog` | 可用 | 2 |
| `universalProviders` | 未接入，隐藏入口 | 0 |
| `modelDiscovery` | 可用 | 1 |

| 应用 | 基础管理依赖 |
| --- | --- |
| claude | 通用供应商命令 |
| claude-desktop | `ensure_claude_desktop_official_provider`, `get_claude_desktop_status`, `get_claude_desktop_default_routes` |
| codex | `ensure_codex_official_provider` |
| gemini | 通用供应商命令 |
| grokbuild | `ensure_grokbuild_official_provider` |
| opencode | `get_opencode_live_provider_ids`, `import_opencode_providers_from_live`, `get_opencode_default_model`, `set_opencode_default_model` |
| openclaw | `get_openclaw_live_provider_ids`, `get_openclaw_default_model`, `set_openclaw_default_model`, `get_openclaw_model_catalog` |
| hermes | `get_hermes_live_provider_ids`, `get_hermes_model_config` |
| pi | `get_pi_current_state`, `get_pi_session_discovery`, `update_pi_provider_usage_script` |
| mcode | 通用供应商命令 |
| kimi | 通用供应商命令 |
| agy | `get_agy_models` |

Antigravity 支持卡片编辑和服务器 agy CLI 模型发现；Web 不提供会话启动，卡片不会改变用户直接在 shell 启动的 agy。

OpenClaw/Hermes 支持供应商基础管理；工作区、专用配置、记忆及 Web UI 启动入口仍未接入。

## 参数与返回值契约

当前 12 个关键命令具有类型化调用和 Web 运行时校验；其他命令仍只有注册与覆盖检查。

- `get_current_provider`
- `switch_provider`
- `get_pi_current_state`
- `get_pi_session_discovery`
- `get_app_mode`
- `set_proxy_route`
- `get_direct_provider`
- `get_opencode_live_provider_ids`
- `get_opencode_default_model`
- `set_opencode_default_model`
- `get_openclaw_live_provider_ids`
- `get_hermes_live_provider_ids`

新增命令与变更契约见 [Web 接口契约](web-api-contract.md)。面向使用者的逐应用能力见 [应用支持表](web-app-support.md)。

完整使用、部署与限制见 [Web 模式](web-mode-zh.md)。
