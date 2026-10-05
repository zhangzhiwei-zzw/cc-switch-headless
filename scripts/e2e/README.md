# cc-switch-server 端到端用例

针对 web 模式（`cc-switch-server`）的黑盒验证：起真实服务端、用 HTTP 调它、
断言磁盘上的真实结果。**每个用例都在临时 HOME 里跑**，不会碰你真实的
`~/.claude`、`~/.cc-switch`。

## 运行

```bash
# 1. 构建服务端（前端产物会被编进二进制；想改前端不重编的话，也可用 --dist 指向新的 dist-web）
cd src-tauri
cargo build --no-default-features --features server --bin cc-switch-server

# 2. 跑全部
bash scripts/e2e/run-all.sh

# 或单个
bash scripts/e2e/smoke.sh
bash scripts/e2e/security.sh
bash scripts/e2e/proxy.sh
bash scripts/e2e/import-export.sh
bash scripts/e2e/sessions.sh
bash scripts/e2e/pages.sh
```

依赖：`curl`、`python3`。可用环境变量覆盖路径：

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `CC_SWITCH_SERVER_BIN` | `src-tauri/target/debug/cc-switch-server` | 服务端二进制 |
| `CC_SWITCH_WEB_DIST` | `dist-web` | 前端产物目录（不存在时用二进制内置的那份） |

## 用例覆盖

| 脚本 | 覆盖 |
| --- | --- |
| `smoke.sh` | 启动链路命令、供应商增删切换、**真实改写 `~/.claude/settings.json` 且保留用户自有配置**、401/400/403 安全校验、未实现命令的降级 |
| `flags.sh` | 启动旗标的安全组合：非回环 + `--no-token` 被拒绝、`--allow-host` 白名单生效、白名单外仍 400 |
| `proxy.sh` | 起假上游 → 本地代理 → 请求真的被转发（路径、凭据、模型都对）、进入/退出路由模式改写与回退 CLI 配置 |
| `import-export.sh` | 导出 → 下载 → 上传 → 恢复的完整回环、`/api/download` 的目录越权防护（`/etc/passwd` 与 `../` 都是 403）、备份创建与列表 |
| `sessions.sh` | 会话列表（标题 / resume 命令）、消息与整段读取、删除 |
| `pages.sh` | 能力表开关、MCP（含真实写进 `~/.claude.json`）、Prompts 增删启用、Skills 仓库与存储路径、用量汇总 / 定价增删 / 计费来源 |
| `security.sh` | 安全响应头（页面 / API / 401 响应都带）、令牌轮换（旧令牌当场失效、自定义值、太短被拒、重启后沿用）、`--no-token` 下轮换被拒 |

失败时测试目录会保留并打印路径，里面有服务端日志（`server.log`）与各步骤产物。
