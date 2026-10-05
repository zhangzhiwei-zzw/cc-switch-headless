<div align="center">

# cc-switch-headless

### CC Switch 的无头 Web 服务端

**在浏览器里管理 Claude Code / Codex / Gemini CLI 等 10 个 AI CLI 的供应商、本地路由与会话。界面和桌面版一模一样，只是走 HTTP。**

[![基于 cc-switch v4.0.0](https://img.shields.io/badge/%E5%9F%BA%E4%BA%8E-cc--switch%20v4.0.0-blue)](https://github.com/farion1231/cc-switch)
[![License: MIT](https://img.shields.io/badge/license-MIT-lightgrey.svg)](LICENSE)

中文 | [English](README_EN.md) | [上游完整文档](README_ZH.md) | [Web 模式详解](docs/web-mode-zh.md)

</div>

## 为什么有这个 fork

上游 [cc-switch](https://github.com/farion1231/cc-switch) 是 Tauri 桌面应用，要求 **glibc 2.35+ /
WebKitGTK 4.1**（Ubuntu 22.04 起）——在 Ubuntu 20.04 这类系统上跑不起来，甚至**编译不过**。

本 fork 加了一个无头服务端 `cc-switch-server`：

- 用 HTTP 提供**同一套前端**（桌面版与 Web 版来自同一份源码，Web 构建只把 Tauri 的 IPC 层换成 HTTP）
- 二进制**不依赖 Tauri / WebKit**，能在 Ubuntu 20.04 上编译
- 前端**编进二进制**，部署就是拷一个文件
- 本地路由是真跑的：代理在服务端做协议转换与故障转移

桌面版未做改动。

## 快速开始

**装预编译包**（不用装 Rust，二进制在 Ubuntu 20.04 上构建，20.04 到 24.04 都能跑）：

```bash
curl -fsSL https://raw.githubusercontent.com/zhangzhiwei-zzw/cc-switch-headless/main/scripts/install-server.sh | bash
~/.local/bin/cc-switch-server          # 加 --service 顺带装成 systemd 用户服务
```

**或者从源码构建**：

```bash
# 1. 前端（需要 Node 22+ 与 pnpm）
pnpm install && pnpm build:web

# 2. 服务端（机器上不需要任何 webkit / gtk 开发包）
cd src-tauri
cargo build --release --no-default-features --features server --bin cc-switch-server

# 3. 启动，默认监听 127.0.0.1:15800
./target/release/cc-switch-server
```

启动时日志里会打印一条带令牌的地址，用浏览器打开它即可：

```
http://127.0.0.1:15800/auth?token=<64 位令牌>
```

打开一次后会种下 cookie，之后直接访问 `http://127.0.0.1:15800/` 就行。
令牌也保存在 `~/.cc-switch/web-token`。

## 换个地方用

**远程服务器** —— 服务端默认只监听回环，走 SSH 端口转发最省事：

```bash
ssh -L 15800:127.0.0.1:15800 user@your-server
```

**Docker**：

```bash
docker compose up -d                      # 配置见 docker-compose.yml
docker logs cc-switch-web | grep token    # 取令牌
```

**常驻** —— 仓库自带 systemd 单元：

```bash
cp scripts/systemd/cc-switch-server.service ~/.config/systemd/user/
systemctl --user enable --now cc-switch-server
loginctl enable-linger "$USER"            # 未登录也保持运行
```

## 能做什么

界面与桌面版一致，下面这些都能在浏览器里完成：

- **供应商**：新增 / 编辑 / 删除 / 切换 / 排序；编辑器会预览「切过去之后配置文件长什么样」
- **本地路由**：启动代理（默认 `127.0.0.1:15721`）、把 CLI 配置指向它、在直连与路由之间切换、配置故障转移队列
- **数据**：导出 SQL 备份（浏览器直接下载）、上传 SQL 恢复、备份的创建 / 恢复 / 重命名 / 删除
- **会话**：浏览**服务端这台机器**上各 CLI 留下的会话，支持搜索、查看、删除
- **MCP**：增删改、按应用开关、从各应用导入、一键重新同步回各应用
- **Skills**：安装 / 更新 / 卸载、仓库管理、从未托管目录导入、存储位置迁移、
  从 ZIP 安装（浏览器选文件 → 上传 → 服务端解压）、备份恢复、skills.sh 搜索
- **Prompts**：提示词库增删改与启用、从现有文件导入、Pi 原生提示词文件与模板
- **用量**：汇总 / 每日趋势 / 供应商与模型维度 / 请求日志、模型定价维护与 models.dev 同步、
  会话日志手动同步与 Codex 用量重建、供应商余额与额度查询

## 还没有的

托管账号登录（Copilot / Codex / xAI）及其用量查询、Stack 聚合模式、熔断器面板、
CLI 工具版本管理，以及目录选择、终端拉起、「在文件管理器里打开」这类桌面专属操作。

界面会通过 `GET /api/capabilities` 拿到支持矩阵，**没接入的入口直接隐藏**——不会让你点进去看报错。

## 安全

服务端能读写 `~/.claude`、`~/.codex` 等配置，持有各家 API Key，还能执行外部命令，所以：

- 默认**只监听 `127.0.0.1`**，并校验 `Host` / `Origin`（防 DNS rebinding）
- `/api/*` 需要令牌；`--no-token` 只允许配合回环地址（配非回环会被拒绝启动）
- 带 `nosniff` / `X-Frame-Options: DENY` / `Referrer-Policy: no-referrer` 等安全响应头
- 令牌可以**不重启轮换**（旧令牌当场失效）：

  ```bash
  curl -X POST http://127.0.0.1:15800/api/rotate-token \
    -H "Cookie: ccswitch_web_token=$(cat ~/.cc-switch/web-token)" \
    -H 'Content-Type: application/json' -d '{}'
  ```

  也可以在 `-d` 里带上 `{"token":"你自己的够长的令牌"}` 指定新值。
- 远程访问用 SSH 转发；容器里用 `--bind 0.0.0.0`，但端口只映射到宿主机回环
- 放在 TLS 反代后面时加 `--hsts`

参数、systemd、Docker 与实现原理见 **[docs/web-mode-zh.md](docs/web-mode-zh.md)**。

## 致谢

Fork 自 [farion1231/cc-switch](https://github.com/farion1231/cc-switch)（作者 Jason Young），MIT 许可。
**上游功能及其问题请反馈给上游仓库**；上游应用的完整中文文档见 [README_ZH.md](README_ZH.md)。
