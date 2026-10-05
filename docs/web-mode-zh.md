# Web 模式（`cc-switch-server`）

> 非官方改造：在系统库过旧、跑不了桌面版的机器（如 **Ubuntu 20.04**：只有 glibc 2.31 与
> WebKitGTK 4.0，而桌面版要求 glibc 2.35+ / WebKitGTK 4.1）上，用**同一个前端源码**
> 起一个本地 HTTP 服务，浏览器打开后界面与桌面版一致。

## 它是怎么做到的

**前端一行不改**：给 Vite 加一组条件 alias，web 构建时把 `@tauri-apps/*` 换成
`src/web-shims/` 里的浏览器实现（`invoke` → `fetch("/api/invoke")`，`listen` → SSE）。
桌面构建不加 alias，行为与产物完全不变。

**后端剥掉 Tauri**：`src-tauri` 增加两个 cargo feature：

| feature | 产物 | 依赖 |
| --- | --- | --- |
| `desktop`（默认） | `cc-switch`（原有桌面应用） | Tauri + WebKitGTK |
| `server` | `cc-switch-server`（本次新增） | 无 Tauri / 无 webkit |

业务层（数据库、供应商切换、配置写入、会话扫描……）两种构建共用；命令层
（`src-tauri/src/commands/`，302 个 `#[tauri::command]`）只在桌面版编译，服务端在
`src-tauri/src/web/routes.rs` 里用同一套 `services/` 接口重新实现了一部分命令。

## 构建与运行

```bash
# 1. 前端（Node 22+ 与 pnpm）
pnpm install
pnpm build:web            # 产物在 dist-web/（桌面版仍用 dist/）

# 2. 服务端（机器上不需要任何 webkit/gtk 开发包）
cd src-tauri
cargo build --release --no-default-features --features server --bin cc-switch-server

# 3. 运行（默认 127.0.0.1:15800）
./target/release/cc-switch-server --dist ../dist-web
#   首次访问的地址与令牌会打印在启动日志里，例如：
#   http://127.0.0.1:15800/auth?token=<32字节令牌>
```

浏览器打开上面那条带 token 的链接即可；服务端会下发 `HttpOnly` cookie，之后正常访问
`http://127.0.0.1:15800/` 就行。令牌保存在 `<配置目录>/web-token`，也可用
`--token` / `CC_SWITCH_WEB_TOKEN` 指定，或 `--no-token` 关闭校验（不建议）。

### 远程机器

服务端只监听回环地址，远程访问请用 SSH 端口转发：

```bash
ssh -L 15800:127.0.0.1:15800 user@your-server
# 然后在本机浏览器打开 http://127.0.0.1:15800/auth?token=...
```

### 作为 systemd 服务运行（推荐）

```bash
mkdir -p ~/.local/bin ~/.local/share/cc-switch ~/.config/systemd/user
cp src-tauri/target/release/cc-switch-server ~/.local/bin/
cp -r dist-web ~/.local/share/cc-switch/
cp scripts/systemd/cc-switch-server.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now cc-switch-server
loginctl enable-linger "$USER"          # 未登录时也保持运行
journalctl --user -u cc-switch-server -f
```

单元文件里的路径、端口都写在 `ExecStart` 一行，改完 `systemctl --user daemon-reload` 即可。

### 常用参数与环境变量

| 参数 | 环境变量 | 说明 |
| --- | --- | --- |
| `--port <端口>` | `CC_SWITCH_WEB_PORT` | 监听端口，默认 `15800` |
| `--dist <目录>` | `CC_SWITCH_WEB_DIST` | 前端产物目录，默认 `dist-web` |
| `--token <令牌>` | `CC_SWITCH_WEB_TOKEN` | 访问令牌 |
| `--no-token` | — | 关闭令牌校验（仅限本机自用） |
| — | `CC_SWITCH_CONFIG_DIR` | 覆盖配置目录（默认 `~/.cc-switch`） |

## 安全

服务端能读写 `~/.claude`、`~/.codex` 等配置文件，持有各家 API Key，并能执行外部
CLI 命令。因此：

- 默认**只监听 127.0.0.1**，且校验 `Host` 与 `Origin`（防 DNS rebinding）；
- `/api/*` 需要令牌（cookie 或 `Authorization: Bearer <token>`）；
- 不要把端口暴露到公网/局域网；远程用 SSH 转发。

## 当前范围

已实现（浏览器里可用）：

- 启动链路：设置读取、数据库初始化与迁移、供应商列表/当前项、环境冲突检查；
- 供应商：新增 / 编辑 / 删除 / 切换 / 排序 / 从 live 导入 / 编辑器预览；
- 设置保存、配置目录查询；
- **本地路由（代理）**：启动/停止代理、全局与应用级代理配置、进入/退出路由模式、
  指定路由目标、故障转移队列与自动故障转移开关；
- **导入导出与备份**：导出 SQL 备份（浏览器直接下载）、上传 SQL 恢复配置、
  备份的创建 / 列表 / 重命名 / 删除 / 恢复；
- **会话浏览**：列出 / 读取 / 删除各 CLI 工具在**服务器上**留下的会话，
  搜索与分块渲染照常（浏览器里用一次性拉取模拟 Channel）；
- 事件推送（SSE，事件名与桌面版一致）。

两处与桌面不同的实现方式：

- **本地路由是真跑的**：进入路由模式会把该应用的 CLI 配置改写到本地代理地址，
  请求经代理做协议转换后转发到供应商，退出时按直连那家写回。
- **文件对话框换成上传/下载**：桌面版用系统对话框选路径，浏览器里改成——
  导出时服务端在 `<配置目录>/exports/` 下分配文件名、导出后浏览器自动下载；
  导入时浏览器选文件先上传到 `<配置目录>/uploads/`，再把服务端路径交给同一条导入流程。
  下载接口只允许读取配置目录内的文件（防路径穿越）。

未实现（会返回 `E_NOT_IMPLEMENTED`，界面会提示而不是白屏）：

- 托盘、自动更新、`ccswitch://` 深链、开机自启、窗口控制；
- 目录选择对话框（`pick_directory`）——浏览器无法为服务端选路径；
- 托管账号（Copilot / Codex / xAI）的**登录流程**（转发链路已支持托管账号，
  但 OAuth 登录命令尚未接入）；
- Stack（聚合）模式、熔断器配置面板、定价来源切换；
- 会话浏览与流式读取、用量统计、MCP / Skills / Prompts 面板等其余命令。

## 与桌面版的行为差异

- 数据库版本过新或初始化失败时，服务端**不退出**：前端会显示「升级应用」恢复界面；
- 日志写到 stderr 与 `<配置目录>/logs/cc-switch.log`（桌面版还会输出到托盘/窗口）；
- `open_external`（打开链接）与复制到剪贴板由浏览器直接完成，不走服务端。
