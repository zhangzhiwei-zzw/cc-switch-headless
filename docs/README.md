# cc-switch-headless 文档

本仓库在上游 CC Switch 业务层上提供无头 Web 服务。浏览器操作的是**运行服务端的机器**的配置和会话。

- [安装、Docker、systemd、HTTPS、升级与回滚](web-mode-zh.md)
- [会话读取限额与性能](web-mode-zh.md#会话性能与读取限额) · [请求诊断与浏览器策略](web-mode-zh.md#请求诊断与浏览器策略)
- [应用支持表：供应商、路由、MCP、Skills、会话与用量](web-app-support.md)
- [排错指南：登录、反代、容器、数据库与恢复](web-troubleshooting.md)
- [开发者能力与命令覆盖矩阵](web-capabilities.md)
- [接口契约与新增命令示例](web-api-contract.md)
- [上游同步流程与记录](syncing-upstream-zh.md)
- [安全策略与漏洞报告](../SECURITY.md)
- [参与开发](../CONTRIBUTING.md)
- [桌面开发参考](contributing-desktop.md)
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

### 控制本地构建占用

开发和测试 profile 默认 `debug=0`、`incremental=true`：减少调试符号，保留增量编译。需要堆栈中的源码行号时，可临时设置 `CARGO_PROFILE_DEV_DEBUG=line-tables-only`；测试对应 `CARGO_PROFILE_TEST_DEBUG`。设置变化会引发重新编译，定位完成后恢复默认即可。

日常 Web 开发固定使用 server feature 和 debug profile；检查 Rust 类型时用 `cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features server --bin cc-switch-server`，需要启动服务时再 build。前端改动使用 `pnpm dev:web`，无需每次重编 Rust。完整桌面矩阵和 release 构建可交给 CI，保留 Cargo 下载缓存与固定工具链，避免反复冷编译。

空间紧张时可用 `cargo clean --manifest-path src-tauri/Cargo.toml -p cc-switch` 仅清理本包产物；整个 target 清空后需要重新编译依赖。关闭增量编译可进一步减少缓存，但会牺牲后续修改的编译速度。具体取舍见 [Cargo profile 文档](https://doc.rust-lang.org/cargo/reference/profiles.html)。

## 发布

推送 main 只触发 CI；推送 `v*-headless*` 标签触发服务端发布，也可以手动运行 Release server 并填写已有标签。手动发布会检出该标签，所有架构从同一提交构建；不填标签只产生构建产物。

每个平台提供二进制、SHA256 和 `.build.txt` 源码提交信息。发布版本与上游基线分别记录，Web 修改不应覆盖旧标签来复用同一个版本。可以使用 `v4.0.18-headless` 一类独立标签，并在 Release 说明中标注上游基线。

安装脚本校验二进制 SHA256。构建可以与 CI 并行，公开 Release 前会先校验三平台产物的源码提交与上游基线，然后查询 **产物对应提交** 的 `ci.yml` 运行：

- 只接受本仓库 `main` 上 push、手动或定时触发的 CI；PR、其他提交和其他工作流的结果不算。
- 同一提交有多次 CI 时，以最新一次运行为准；必须完成且结论为 `success`。
- CI 排队、运行中或尚未出现时最多等待 30 分钟；失败、取消或等待超时均停止公开发布，构建产物仍可从工作流下载。
- 修复或重新运行该提交的 CI 后，再重新运行 Release server。修改代码会产生新提交，需要使用新标签，不能覆盖旧标签。
- 例外发布仅限手动运行：填写已有 `tag`，勾选 `skip_ci` 并填写 `ci_bypass_reason`。原因、操作者和提交会写入工作流摘要，Release 正文也会记录跳过原因。自动标签发布不能跳过 CI。

手动发布时建议选择包含最新发布策略的 main 工作流；产物仍检出填写的标签，CI 校验也针对该标签的产物提交。GitHub 运行记录可能被清理，发布旧标签遇到无记录时需要重建对应 CI 或显式记录例外。实现见 [`require-release-ci.cjs`](../scripts/ci/require-release-ci.cjs)，API 依据见 [GitHub 工作流运行文档](https://docs.github.com/en/rest/actions/workflow-runs#list-workflow-runs-for-a-workflow)。

## 仓库维护配置

`.github/CODEOWNERS` 指向本 fork 的维护者 `@zhangzhiwei-zzw`。该文件只声明代码负责人；在 GitHub 的 main 分支保护或 ruleset 中启用必需检查、Code Owner 审核等规则，才会强制执行。仓库保护规则须在 GitHub 单独配置。

Sponsor 按钮保留对上游作者 `farion1231` 的支持入口，并非本 fork 的收款入口。

以下继承的可选流程默认不运行，需要维护者在 GitHub **Settings → Secrets and variables → Actions → Variables** 显式设置：

| 仓库变量 | 启用值 | 用途与前提 |
| --- | --- | --- |
| `ENABLE_CLAUDE_REVIEW` | `true` | 响应维护者的 `@claude` 请求；需要 `CLAUDE_CODE_OAUTH_TOKEN` secret |
| `ENABLE_R2_MIRROR` | `true` | 上游桌面下载/更新镜像；需要 R2 secrets 和桌面 updater 资产，不适用于当前 headless 产物 |
| `ENABLE_DESKTOP_RELEASE` | `true` | 手动桌面打包；需要对应平台的签名、公证配置 |

常规 Web CI、桌面兼容性 CI 和 headless 发布不依赖这些变量。可选流程的事件仍可能产生跳过记录，但未启用时不会分配构建 runner。
