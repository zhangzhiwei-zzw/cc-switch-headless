# Web 模式（`cc-switch-server`）

> `cc-switch-headless` fork 的核心改动：在系统库过旧、跑不了桌面版的机器（如 **Ubuntu 20.04**：
> 只有 glibc 2.31 与 WebKitGTK 4.0，而桌面版要求 glibc 2.35+ / WebKitGTK 4.1）上，
> 用**同一个前端源码**起一个本地 HTTP 服务，浏览器打开后界面与桌面版一致。
>
> 上游项目：[farion1231/cc-switch](https://github.com/farion1231/cc-switch)（MIT）。
> 当前对标基线 **v4.0.0**（已合并至上游 `a33c156e`，PR #7874），见
> [`upstream-v4.0.0.lock`](upstream-v4.0.0.lock)。

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

上面那个安装脚本只面向 Linux；Windows 直接下 `.exe` 手动跑即可。

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

**前端产物被编译进二进制**（`rust-embed`），所以部署只需要拷一个文件。改了前端又不想
重新编译服务端时，用 `--dist ../dist-web` 指向新产物即可（目录优先于内置版本）。

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
    return 301 https://$host$request_uri;      # 不要留明文入口（cookie 没有 Secure 标记）
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
`ccswitch_web_token` cookie（`Path=/; HttpOnly; SameSite=Strict`）并跳回首页。页面本身
（`/` 与静态资源）不做令牌校验，但所有 `/api/*` 调用都要带这个 cookie。两点注意：

- cookie **没有 `Secure` 标记**：别在同一域名下再开明文入口（上面 80 → 443 的跳转就是为此），
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
| `--dist <目录>` | `CC_SWITCH_WEB_DIST` | 前端产物目录；缺省时用编译进二进制的那份 |
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
  -H "Cookie: ccswitch_web_token=$(cat ~/.cc-switch/web-token)" \
  -H 'Content-Type: application/json' -d '{}'
```

- 请求体缺省（或 `token` 为空）时随机生成 256 位新令牌（64 个字符）；传
  `{"token":"..."}` 可以指定自己的值（至少 16 个字符）。
- 新令牌立刻写进 `<配置目录>/web-token` 并生效，**旧令牌当场失效**——别的浏览器
  里的 cookie 也一样，需要重新用 `/auth?token=<新令牌>` 打开一次。
- 令牌来自 `--token` / `CC_SWITCH_WEB_TOKEN` 时，轮换只对本次运行有效：下次启动
  那两个来源仍然优先，日志会提醒你同步更新。
- `--no-token` 启动时没有令牌可换，接口返回 400。

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
  下载接口只允许读取配置目录内的文件（防路径穿越）。

未实现（会返回 `E_NOT_IMPLEMENTED`，界面会提示而不是白屏）：

- 托盘、自动更新、`ccswitch://` 深链、开机自启、窗口控制；
- 目录选择对话框（`pick_directory`）——浏览器无法为服务端选路径；
- 托管账号（Copilot / Codex / xAI）的**登录流程**（转发链路已支持托管账号，
  但 OAuth 登录命令尚未接入）；连带 Copilot 与 xAI 供应商的**用量查询**
  也不可用（额度要问 OAuth 管理器），余额 / Coding Plan / 官方订阅额度 /
  JS 脚本四条路径不受影响；
- Stack（聚合）模式、熔断器配置面板；
- CLI 工具（Apps 页）的版本检测与安装；
- 「在文件管理器里打开」这类动作（`open_config_folder` 等）——服务端没法替
  **用户**弹窗口；入口由 `openInFileManager` 隐藏。
- 深链导入、通用供应商（Universal Provider）、OpenClaw / Hermes / OMO 等
  仍在迁移中的页面。

## 与桌面版的行为差异

- 数据库版本过新或初始化失败时，服务端**不退出**：前端会显示「升级应用」恢复界面；
- 日志写到 stderr 与 `<配置目录>/logs/cc-switch.log`（桌面版还会输出到托盘/窗口）；
- `open_external`（打开链接）与复制到剪贴板由浏览器直接完成，不走服务端。
