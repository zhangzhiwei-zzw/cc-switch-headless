# Web 模式（`cc-switch-server`）

> `cc-switch-headless` fork 的核心改动：在系统库过旧、跑不了桌面版的机器（如 **Ubuntu 20.04**：
> 只有 glibc 2.31 与 WebKitGTK 4.0，而桌面版要求 glibc 2.35+ / WebKitGTK 4.1）上，
> 用**同一个前端源码**起一个本地 HTTP 服务，浏览器打开后界面与桌面版一致。
>
> 上游项目：[farion1231/cc-switch](https://github.com/farion1231/cc-switch)（MIT）。
> 当前对标基线 **v4.0.18**（已合并至上游 `e4873bc8`），见
> [`upstream-v4.0.18.lock`](upstream-v4.0.18.lock)。
>
> **所有配置、密钥和会话都在运行服务端的机器上**：浏览器只是操作界面，在另一台电脑上
> 打开页面，改的是服务器的 `~/.claude`、`~/.codex`、`~/.cc-switch`，不是你本机的。

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
（`src-tauri/src/commands/`、`tray_panel.rs` 等）只在桌面版编译，服务端在
`src-tauri/src/web/routes.rs` 里用同一套 `services/` 接口重新实现了一部分命令
（当前 150+ 个命令名，`GET /api/commands` 可实时核对）。

## 安装

预编译包（不用装 Rust，一行装完；`--service` 顺带装成 systemd 用户服务）：

```bash
curl -fsSL https://raw.githubusercontent.com/zhangzhiwei-zzw/cc-switch-headless/main/scripts/install-server.sh | bash
```

脚本会**强制校验 sha256**：校验文件取不到就直接失败，不会「跳过校验继续装」。
换下载源（镜像站/内网）用 `CC_SWITCH_BASE_URL`，换版本用 `--version <tag>`。

产物随 Release 发布，由 `.github/workflows/release-server.yml` 生成：

| 资产 | 说明 |
| --- | --- |
| `cc-switch-server-linux-x86_64` | Linux x86_64 可执行文件 |
| `cc-switch-server-linux-aarch64` | Linux arm64 可执行文件（树莓派、ARM 云主机） |
| `cc-switch-server-windows-x86_64.exe` | Windows x86_64 可执行文件（未签名，SmartScreen 会提示） |
| 同名 `.sha256` | 各产物的校验和 |

上面那个安装脚本只面向 Linux（它会按 `uname -m` 自动选 x86_64 / aarch64）。Windows 没有安装器，
下 `cc-switch-server-windows-x86_64.exe` 直接运行即可；校验用
`certutil -hashfile .\cc-switch-server-windows-x86_64.exe SHA256` 与同名 `.sha256` 比对。
SmartScreen 会拦一次（未签名）：**更多信息 → 仍要运行**；关掉终端进程就结束，想常驻可以用
任务计划程序。

Linux 产物**在 `ubuntu:20.04` 容器里编译**（GitHub 已经没有 20.04 runner，所以复用仓库
自带的 Dockerfile；两个架构各自在自己的原生 runner 上构建，不走 QEMU，所以 glibc 底线
不受 runner 版本影响），构建末尾有两道断言：最高 glibc 符号引用不得高于 **2.31**，且
不得动态链接 `libssl`/`libcrypto`。两者任一不满足就直接失败，避免推出一个「看着能下、
装上跑不起来」的包。这也是为什么 `reqwest` 关掉了 `default-features`——它的
`default-tls` 会把 OpenSSL 链进来，而 20.04 是 libssl 1.1.1、22.04+ 是 3.x，
两边互不兼容。

## 从源码构建与运行

```bash
# 1. 前端（Node 22+ 与 pnpm）
pnpm install
pnpm build:web            # 产物在 dist-web/（桌面版仍用 dist/）

# 2. 服务端（机器上不需要任何 webkit/gtk 开发包）
cd src-tauri
cargo build --release --no-default-features --features server --bin cc-switch-server

# 3. 运行（默认 127.0.0.1:15800）
./target/release/cc-switch-server
#   前端产物已编进二进制，直接跑就行；首次访问的地址与令牌会打印在启动日志里：
#   http://127.0.0.1:15800/auth?token=<64 位令牌>
```

**前端产物被编译进二进制**（`rust-embed`），所以部署只需要拷一个文件，从任何目录启动
都不会被别处的旧产物顶掉。改了前端又不想重新编译服务端时，用 `--dist ../dist-web` 指向
新产物即可——**只有显式指定才会读目录**；目录里没有 `index.html` 会拒绝启动，不会悄悄
退回内置版本（否则你会在浏览器里对着另一份前端产物排查问题）。

浏览器打开上面那条带 token 的链接即可；服务端会下发 `HttpOnly` cookie，之后正常访问
`http://127.0.0.1:15800/` 就行。令牌保存在 `<配置目录>/web-token`，也可用
`--token` / `CC_SWITCH_WEB_TOKEN` 指定，或 `--no-token` 关闭校验（**只能配合回环地址**，
配非回环监听会被拒绝启动）。

### 远程机器

服务端默认只监听回环地址，远程访问请用 SSH 端口转发：

```bash
ssh -L 15800:127.0.0.1:15800 user@your-server
# 然后在本机浏览器打开 http://127.0.0.1:15800/auth?token=...
```

### TLS 反向代理（Nginx / Caddy）

用域名 + HTTPS 访问时，把服务端放在反向代理后面：

1. 服务端保持默认只监听 `127.0.0.1`，由反代访问它——`15800` 不要对外发布；
2. 启动时加 `--allow-host <域名>`（可多次，或 `CC_SWITCH_ALLOW_HOSTS=a.com,b.com`）。
   `Host` 与 `Origin` 默认只放行回环地址，白名单是**精确匹配**（可以带端口，不支持通配符），
   所以反代必须把浏览器原始的 `Host` 透传给服务端；域名也必须进白名单——浏览器发
   `POST /api/invoke` 时会带 `Origin: https://<域名>`，它同样要过校验；
3. 加 `--hsts`，让响应带 `Strict-Transport-Security: max-age=31536000`。

systemd 单元的 `ExecStart` 相应改成：

```ini
ExecStart=%h/.local/bin/cc-switch-server --allow-host cc-switch.example.com --hsts
```

Nginx（`access_log` 用 `$uri` 而不是 `$request`，避免 `/auth?token=...` 里的令牌写进日志）：

```nginx
# log_format 要放在 http 上下文里
log_format ccswitch '$remote_addr - $remote_user [$time_local] '
                    '"$request_method $uri $server_protocol" $status '
                    '$body_bytes_sent "$http_user_agent"';

server {
    listen 80;
    server_name cc-switch.example.com;
    return 301 https://$host$request_uri;      # HTTPS 部署只保留重定向入口
}

server {
    listen 443 ssl http2;
    server_name cc-switch.example.com;

    ssl_certificate     /etc/letsencrypt/live/cc-switch.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/cc-switch.example.com/privkey.pem;

    access_log /var/log/nginx/cc-switch.access.log ccswitch;
    client_max_body_size 64m;                  # 与服务端的上传上限一致

    location / {
        proxy_pass http://127.0.0.1:15800;
        proxy_http_version 1.1;
        proxy_set_header Host $host;           # 必须保留原始 Host，否则过不了 Host/Origin 校验
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header Connection "";

        proxy_buffering off;                   # /api/events 是 SSE，别缓冲
        proxy_read_timeout 3600s;              # 同理，别让空闲的 SSE 连接被掐断
    }
}
```

Caddy（自动申请证书、默认透传原始 `Host`，但不替你加 HSTS——`--hsts` 仍然要加）：

```
cc-switch.example.com {
    reverse_proxy 127.0.0.1:15800
}
```

首次访问走 `https://cc-switch.example.com/auth?token=<令牌>`，通过后服务端下发
按实例区分名称的 cookie（`Path=/; HttpOnly; SameSite=Strict`）并跳回首页。页面本身
（`/` 与静态资源）不做令牌校验，但所有 `/api/*` 调用都要带这个 cookie。两点注意：

- 使用 `--hsts` 时 cookie 带 `Secure` 标记：别在同一域名下再开明文入口（上面 80 → 443 的跳转就是为此），
  也别把后端端口发布出去；
- `SameSite=Strict` 意味着从外部站点跳进来不会带上 cookie，首次访问必须用 `/auth?token=...` 链接。

脚本 / 命令行用 Bearer 即可，不依赖 cookie：

```bash
curl -H "Authorization: Bearer $(cat ~/.cc-switch/web-token)" \
  https://cc-switch.example.com/api/env
```

### Docker

```bash
docker build -t cc-switch-web .
docker run -d --name cc-switch-web -p 127.0.0.1:15800:15800 \
  -v cc-switch-data:/data cc-switch-web
docker logs cc-switch-web | grep token      # 取首次访问的令牌
```

或 `docker compose up -d`（见仓库根目录的 `docker-compose.yml`，里面有挂载宿主机 CLI
配置的注释示例）。容器内配置目录是 `/data/.cc-switch`，挂卷持久化即可。

容器里必须绑 `0.0.0.0`（镜像的默认 CMD 已经带了），否则端口映射进不来；对外仍然只把
端口映射到宿主机回环。

### 作为 systemd 服务运行（推荐）

```bash
mkdir -p ~/.local/bin ~/.config/systemd/user
cp src-tauri/target/release/cc-switch-server ~/.local/bin/
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
| `--bind <地址>` | `CC_SWITCH_WEB_BIND` | 监听地址，默认 `127.0.0.1`（容器里用 `0.0.0.0`） |
| `--dist <目录>` | `CC_SWITCH_WEB_DIST` | 前端产物目录；不指定时用编译进二进制的那份，指定的目录缺 `index.html` 会拒绝启动 |
| `--token <令牌>` | `CC_SWITCH_WEB_TOKEN` | 访问令牌（默认自动生成并写入配置目录） |
| `--no-token` | — | 关闭令牌校验（只能配合回环地址） |
| `--hsts` | `CC_SWITCH_WEB_HSTS` | 下发 `Strict-Transport-Security`（放在 TLS 反代后面时才需要） |
| `--allow-host <主机名>` | `CC_SWITCH_ALLOW_HOSTS`（逗号分隔） | 除回环外额外允许的 `Host`，用域名/局域网访问时才需要 |
| — | `CC_SWITCH_CONFIG_DIR` | 覆盖配置目录（默认 `~/.cc-switch`） |

## 安全

服务端能读写 `~/.claude`、`~/.codex` 等配置文件，持有各家 API Key，并能执行外部
CLI 命令。因此：

- 默认**只监听 127.0.0.1**，且校验 `Host` 与 `Origin`（防 DNS rebinding）；
- `/api/*` 需要令牌（cookie 或 `Authorization: Bearer <token>`）；
- 每个响应都带 `X-Content-Type-Options: nosniff`、`X-Frame-Options: DENY`、
  `Referrer-Policy: no-referrer`、`Permissions-Policy`；
- 上传体积上限 64 MiB；
- 不要把端口暴露到公网/局域网；远程用 SSH 转发。

> 没上 CSP：前端 `src/index.html` 里有一段内联的主题初始化脚本（避免深色模式闪烁），
> `script-src 'self'` 会把它拦掉，写死 sha256 又会在上游改动那段脚本时静默失效。
> 要加 CSP，得先把那段脚本抽成独立文件。

### 轮换访问令牌

令牌泄露、或者只是不想再用启动日志里那串，可以不重启换掉：

```bash
curl -X POST http://127.0.0.1:15800/api/rotate-token \
  -H "Authorization: Bearer $(cat ~/.cc-switch/web-token)" \
  -H 'Content-Type: application/json' -d '{}'
```

- 请求体缺省（或 `token` 为空）时随机生成新令牌（64 个字符）；传
  `{"token":"..."}` 可以指定自己的值（至少 16 个字符，仅支持英文字母、数字和 `-_.~`）。
- 新令牌立刻写进 `<配置目录>/web-token` 并生效，**旧令牌当场失效**——别的浏览器
  里的 cookie 也一样，已有 SSE 连接同时断开，需要重新登录。令牌使用原子写入（Unix 权限 0600），写盘失败返回 500 并保留旧令牌。
- 令牌来自 `--token` / `CC_SWITCH_WEB_TOKEN` 时，轮换只对本次运行有效：下次启动
  那两个来源仍然优先，日志会提醒你同步更新。
- `--no-token` 启动时没有令牌可换，接口返回 400。

## 当前范围

已实现（浏览器里可用）：

- 启动链路：设置读取、数据库初始化与迁移、供应商列表/当前项、环境冲突检查；
- 供应商：新增 / 编辑 / 删除 / 切换 / 排序 / 从 live 导入 / 编辑器预览；
  **「我的图标」**（本地上传、重命名、删除、跨应用复用）也在服务端落库：单张 PNG
  ≤ 256×256、data URI ≤ 64 KiB，整个图标库最多 100 个，名称 ≤ 64 字符；
- **Kimi Code**：支持供应商及模型管理、MCP、Skills、提示词、会话阅读和用量同步；配置位于服务端的 `~/.kimi-code`（可通过 `KIMI_CODE_HOME` 覆盖）。采用多供应商共存模式，默认模型在 CLI 中用 `/model` 选择，不提供本地路由；OAuth 登录和会话删除由 Kimi Code 管理。详见 [Kimi 接入](kimi-code-integration-zh.md)。
- **Antigravity（agy）供应商**：可配置 API Key、地址和模型；模型发现由服务器上安装的
  `agy` CLI 执行。Web 不启动 agy 会话，也不会改变直接在 shell 启动的 agy；页面明确显示此限制。OpenCode 供应商卡也可读取 / 设置 `opencode.json` 顶层默认模型；
- 设置保存、配置目录查询（含 Codex 聚合设置变更后的即时重写）；用量页的显示设置
  （货币与汇率、侧栏 Today 显示项、请求日志列）存在服务端 `settings.json`，**所有浏览器
  共享**；「记住上次 Tab / 时间范围」用浏览器 localStorage，按浏览器各自独立；
- **Pi 供应商状态**：读取原生启用列表、默认供应商与会话目录发现状态，支持保存用量脚本；
- **本地路由（代理）**：启动/停止代理、全局与应用级代理配置、进入/退出路由模式、
  指定路由目标、故障转移队列与自动故障转移开关；Claude Desktop 和 Claude Code /
  Codex / Gemini CLI / Grok Build 共用直连 / 路由模式面板（Desktop 没有故障转移，
  也不支持聚合）。Desktop 写的是**服务端本机**的 `~/.config/Claude`（Linux）/
  `Application Support/Claude`（macOS）/ `%LOCALAPPDATA%\Claude`（Windows），
  远程浏览器操作不到你自己电脑上的 Desktop；
- **导入导出与备份**：导出 SQL 备份（浏览器直接下载）、上传 SQL 恢复配置、
  备份的创建 / 列表 / 重命名 / 删除 / 恢复；
- **会话浏览**：列出 / 读取 / 删除各 CLI 工具在**服务器上**留下的会话，
  搜索与分块渲染照常（浏览器通过认证后的 NDJSON HTTP 响应逐批接收）；后台兜底扫描每 10 分钟
  一次，打开用量页时按所选间隔加扫；
- **MCP**：统一结构的增删改、按应用开关、从各应用导入、重新同步回各应用，
  以及旧接口（直接读写 `~/.claude.json` 的 `mcpServers`）的兼容命令；
- **Skills**：已安装列表、仓库管理与发现、安装 / 更新 / 卸载、按应用开关、
  从未托管目录导入、存储位置迁移、从 ZIP 安装（浏览器选文件 → 上传 → 服务端解压）、
  备份的列表 / 恢复 / 删除、skills.sh 搜索；
- **Prompts**：提示词库增删改与启用、从现有文件导入、Pi 原生提示词文件与模板；
- **用量统计**：汇总 / 按应用拆分 / 每日趋势 / 供应商与模型维度 / 请求日志与详情、
  模型定价的增删改与 models.dev 同步配置、会话日志手动同步与 Codex 用量重建、
  供应商用量查询（余额 / Coding Plan / 官方订阅额度 / 通用 JS 脚本）；
- 事件推送（SSE，事件名与桌面版一致）。

服务端还提供 `GET /api/capabilities`：返回各功能是否可用（页面级与页面内动作），
界面据此**隐藏**没接入的入口，而不是让用户点进去看报错。`GET /api/commands`
列出全部已实现的命令，排查时很有用。

两处与桌面不同的实现方式：

- **本地路由是真跑的**：进入路由模式会把该应用的 CLI 配置改写到本地代理地址，
  请求经代理做协议转换后转发到供应商，退出时按直连那家写回。
- **文件对话框换成上传/下载**：桌面版用系统对话框选路径，浏览器里改成——
  导出时服务端在 `<配置目录>/exports/` 下分配文件名、导出后浏览器自动下载；
  导入时浏览器选文件先上传到 `<配置目录>/uploads/`，再把服务端路径交给同一条导入流程。
  下载接口只允许读取配置目录内的文件（防路径穿越）。上传/导出每次使用独立私有目录；临时文件 24 小时后过期，每小时及分配时清理，传输目录总容量限制 512 MiB。数据库备份不在此清理范围内。

未实现（会返回 `E_NOT_IMPLEMENTED`，界面会提示而不是白屏）：

- 托盘、自动更新（含便携版更新下载）、`ccswitch://` 深链、开机自启、窗口控制；
- 目录选择对话框（`pick_directory`）——浏览器无法为服务端选路径；
- 托管账号中心（Copilot / Codex / xAI / Antigravity）的**登录、切换与账号管理**
  （转发链路已支持已有托管账号，OAuth 管理命令尚未接入）；连带 Copilot 与 xAI 供应商的**用量查询**
  也不可用（额度要问 OAuth 管理器），余额 / Coding Plan / 官方订阅额度 /
  JS 脚本四条路径不受影响；
- Stack（聚合）模式、独立熔断器诊断与健康状态接口；应用级故障转移参数仍可通过路由设置保存；
- CLI 工具（Apps 页）：版本检测、安装、升级、卸载与安装冲突诊断（`pageApps: false`）；
- 「打开终端」——服务端没有可拉起的终端，入口按 `pickDirectory` 能力隐藏；
- 「在文件管理器里打开」这类动作（`open_config_folder` 等）——服务端没法替
  **用户**弹窗口；入口由 `openInFileManager` 隐藏；
- 深链导入——服务端收不到 `ccswitch://`，确认框按 `isTauriRuntime()` 不挂载；
- 配置档案（Profiles）、WebDAV / S3 云同步与自动同步、每日记忆 / 工作区文件编辑；
- 端点测试、流检查、测速等供应商连通性诊断；供应商表单的模型发现已接入共享服务，不属于此限制；
- 通用供应商（Universal Provider）、OpenClaw / Hermes / OMO 等仍在迁移中的页面。

## 与桌面版的行为差异

- 数据库版本过新或初始化失败时，服务端**不退出**：前端会显示「升级应用」恢复界面；
- 日志写到 stderr 与 `<配置目录>/logs/cc-switch.log`（桌面版还会输出到托盘/窗口）；
- `open_external`（打开链接）与复制到剪贴板由浏览器直接完成，不走服务端。

## 常见问题

### 登录与连接状态

直接访问首页会先检查登录与功能能力。未登录或令牌失效时显示登录页，可输入服务端
启动日志或 `<配置目录>/web-token` 中的令牌；登录通过 `POST /auth` 提交，令牌不会
加入浏览器地址或历史记录。启动日志中的 `/auth?token=...` 链接仍然可用。

服务不可达时显示重新连接入口；能力表异常或缺少已声明功能所需的命令时阻止进入应用，
提示检查代理和前后端版本。运行中遇到 401、网络断连或 SSE 断开也会检查连接状态。
重连成功后重新加载页面，避免沿用旧缓存；失败的写操作不会自动重放。

### Windows + WSL 两个实例，怎么判断打开的是哪个？

两边都监听 `127.0.0.1:15800` 时，WSL2 的 localhost 转发只在 Windows 侧没有进程占用
该端口时生效：Windows 版 `cc-switch-server.exe` 先启动，浏览器连到的就是 Windows 实例；
反之才是 WSL 里的实例。判断方法：

- 看启动日志里打印的配置目录与令牌；
- 两个实例的数据库互相独立（`%USERPROFILE%\.cc-switch` vs `~/.cc-switch`）；
- 页面里显示的供应商 / 会话是哪一份数据，就是哪边。

同一个监听地址和端口无法同时提供两个实例；不同数据目录的实例应使用不同端口。

### 想同时跑多个实例

用 `--port` 和 `CC_SWITCH_CONFIG_DIR` 隔离，例如：

```bash
CC_SWITCH_CONFIG_DIR=~/.cc-switch-work cc-switch-server --port 15801
```

Cookie **不按端口隔离**。服务端会在数据目录保存 `web-instance-id`，使用包含该标识和
监听端口的独立 Cookie 名称，避免同一浏览器访问多个实例时互相覆盖登录。
Docker 使用不同数据卷时，即使容器内部端口相同也会生成不同的标识。

复制已有数据目录（包括 `web-instance-id`）后，若实例监听端口也相同，可通过
`--cookie-name cc_switch_work` 或 `CC_SWITCH_WEB_COOKIE_NAME=cc_switch_work` 显式指定
不同名称。名称支持 1–128 个字母、数字、下划线或连字符，重启后保持一致。
Cookie 名称区分用于避免会话冲突；有不同信任级别的实例应使用不同主机名部署。

从固定 Cookie 名称的旧版本升级后需要重新登录一次。脚本调用建议使用
`Authorization: Bearer <令牌>`，无需知道实例 Cookie 名称。

### 升级后界面没变化

前端产物编译在二进制里，替换二进制后**必须重启服务端进程**。`--dist` 只在本地开发时
临时指向 `dist-web/`，生产环境不要依赖它。指定目录时静态资源和 SPA 回退均只使用该目录，不混用内置资源；父路径与越界符号链接会被拒绝。

### 健康检查与日志

`GET /healthz` 检查 HTTP 存活；`GET /readyz` 在数据库初始化正常时返回 200，否则 503。
它们无需令牌，仍受 Host/Origin 检查。Docker 默认使用就绪检查。日志写到 stderr 和
`<配置目录>/logs/cc-switch.log`，单文件 10 MiB，最多保留 3 个归档；用 Nginx 反代时注意别把 `/auth?token=...` 的 query
写进访问日志。

## Web 命令覆盖与浏览器回归

`src/web-shims/command-contract.json` 记录功能依赖、各应用所需命令、浏览器适配及
尚未支持的命令说明。`pnpm check:web` 使用 TypeScript AST 检查前端的 `invoke` 字面量，
与真实 Rust 分发表及浏览器适配对照；新增调用必须实现或显式说明不支持的原因。
服务端能力表也读取这份清单，前端再次核验能力依赖，避免表面可用却返回 `E_NOT_IMPLEMENTED`。

本地运行真实浏览器回归：

```bash
pnpm build:web
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features server --bin cc-switch-server -j 1
pnpm exec playwright install chromium --only-shell
pnpm check:web
pnpm test:web
```

浏览器测试仅使用临时 HOME 和数据目录，覆盖登录、Pi 页面、功能契约、令牌轮换、
断连重试，以及两个实例的 Cookie 共存和重启。默认使用 15870 / 15871 端口；可用
`CC_SWITCH_SERVER_BIN` 指定服务端二进制。CI 的 server job 同时运行 HTTP 和浏览器回归。

## 升级、回滚与维护

- 升级前在设置中创建数据库备份，并下载 SQL 导出保存到服务器之外。数据库备份包含 API Key，应按凭据保护。
- 使用安装脚本升级：`bash scripts/install-server.sh --service`。新文件先执行验证，再原子替换；已有二进制保存在同目录的 `cc-switch-server.previous`，服务会重启。
- 验证：`curl -f http://127.0.0.1:15800/readyz`，再登录页面检查供应商和实际 CLI 配置。
- 回滚：停止服务，恢复旧二进制；若新版本迁移了数据库，应同时恢复升级前备份。不要让旧程序直接打开新版本数据库。现有数据库预检会拒绝这种操作。
- 收到 SIGTERM/Ctrl+C 时，HTTP 服务停止接收新连接、关闭事件连接并等待进行中的请求完成，然后停止本地代理。服务停止后路由配置可能仍指向代理，重新运行 CLI 前应恢复服务，或在停机前切回直连。
- “关于”页显示实例、构建提交和数据目录；`GET /api/env` 提供同样的诊断信息，需要令牌。
- 静态资源支持 gzip；用量、会话和设置等页面按需加载。普通命令等待上限 60 秒，导入/安装/同步等长任务 5 分钟，会话流 2 分钟。底层会话解析目前仍一次加载会话文件；分批传输减少浏览器的整包等待，但尚未实现服务端的增量解析。超时不表示服务端已回滚，不会自动重试写操作。

逐应用限制见 [应用支持表](web-app-support.md)，部署故障见 [排错指南](web-troubleshooting.md)。开发者功能状态由 [Web 支持矩阵](web-capabilities.md) 与运行时能力表提供。开发时新增能力后运行 `node scripts/web/generate-capabilities.mjs`；CI 会检查该文档是否过期。

### 旧访问令牌的兼容性

自动生成的令牌可直接沿用。若升级前通过参数、环境变量或文件使用了少于 16 字符、含空格或其他特殊字符的手动令牌，升级后会拒绝启动。请先记录原配置，换成符合 `A-Z a-z 0-9 - _ . ~` 规则的新令牌，再重新登录。
