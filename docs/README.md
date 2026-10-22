# cc-switch-headless 文档

本仓库在上游 CC Switch 业务层上提供无头 Web 服务。浏览器操作的是**运行服务端的机器**的配置和会话。

- [安装、Docker、systemd、HTTPS、升级与回滚](web-mode-zh.md)
- [逐功能和应用支持矩阵](web-capabilities.md)
- [上游同步流程与记录](syncing-upstream-zh.md)
- [安全策略与漏洞报告](../SECURITY.md)
- [参与开发](../CONTRIBUTING.md)
- [上游用户手册](user-manual/README.md)：包含桌面版功能，请结合 Web 支持矩阵阅读。

## Web 开发

```bash
pnpm install --frozen-lockfile
pnpm build:web
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features server --bin cc-switch-server
./src-tauri/target/debug/cc-switch-server
# 第二个终端：开发页面与热更新，API 代理到 127.0.0.1:15800
pnpm dev:web
```

`build:web` 和 `dev:web` 通过 Node 设置环境变量，兼容 Windows。Windows 启动二进制路径使用 `src-tauri/target/debug/cc-switch-server.exe`。HTTP 回归脚本使用 Bash/Python，应在 Linux 或 WSL 中运行。

## 发布

推送 main 只触发 CI；推送 `v*` 标签才触发服务端发布，也可以手动运行 Release server 并填写已有标签。手动发布会检出该标签，所有架构从同一提交构建；不填标签只产生构建产物。

每个平台提供二进制、SHA256 和 `.build.txt` 源码提交信息。发布版本与上游基线分别记录，Web 修改不应覆盖旧标签来复用同一个版本。可以使用 `v4.0.17-web.1` 一类独立标签，并在 Release 说明中标注上游基线。

安装脚本校验二进制 SHA256；维护者发布前应核对构建提交和 CI 结果。Release 工作流不以 main CI 为依赖，手动发布时需自行确认目标提交的检查结果。
