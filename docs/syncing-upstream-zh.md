# 同步上游（farion1231/cc-switch）

这个仓库是 [farion1231/cc-switch](https://github.com/farion1231/cc-switch) 的 fork，
上游会持续加新供应商预设、新 CLI 支持、修代理和会话解析的 bug。这份文档是怎么把它们
合进来、以及合并时要注意什么。

基线记录在 [`upstream-v4.0.14.lock`](upstream-v4.0.14.lock)：那个文件里的
`last_merged_commit` 就是「这个 fork 跟到上游哪里」。

## 一次性配置

```bash
git remote add upstream https://github.com/farion1231/cc-switch.git
git fetch upstream --tags
```

`--tags` 是安全的：本 fork 的 tag 叫 `v4.0.0-headless` / `.1` / `.2`，和上游的 `v4.0.0`
不会撞名（这也是当初特意取这个前缀的原因）。

## 每次同步

```bash
git checkout main
git pull                                          # 先把自己 fork 的 main 更新到最新

git checkout -b sync/upstream-2026-11             # 开同步分支，别直接在 main 上解冲突
git fetch upstream
git log --oneline HEAD..upstream/main | head -50  # 先看上游都改了什么
git diff --stat HEAD...upstream/main -- src-tauri src   # 看看改动规模

git merge upstream/main
# 解冲突 → git commit → git push -u origin sync/upstream-2026-11
# 然后开 PR，等 CI 全绿，再合回 main
```

**走 PR，不要直接推 main。** 合并后最容易出问题的地方是编译（见下面「合并后必须验证」），
而 CI 里的 `Server Checks (headless)` 正是为这件事存在的——它会在 PR 上把 server feature
编一遍并跑完整 e2e。

## 为什么用 merge，不用 rebase

- `main` 已经推到公开仓库，rebase 会重写历史、必须 force-push，会打断任何 clone 过它的人；
- 本 fork 改了 **50+ 个上游已有的文件**（要让 crate 在 `server` feature 下编译，很多共享文件
  被加了 `#[cfg(feature = "desktop")]` 门），rebase 意味着**每次都要重新解一遍**这些冲突，
  merge 只解一次；
- 历史本来就是 merge 出来的（`git log --merges`）。

## 实测：一次合并要解多少冲突

| 同步 | 上游范围 | 上游提交 | 上游改动文件 | 冲突 |
| --- | --- | --- | --- | --- |
| 2026-10-11 | v4.0.0 → v4.0.8 | 147 | 479 | 10 个文件 / 15 处 |
| 2026-10-12 | v4.0.8 → v4.0.9 | 41 | 109 | 4 个文件 / 4 处 |
| 2026-10-17 | v4.0.9 → v4.0.14 | 175 | 335 | 5 个文件 / 6 处 |

第一次（v4.0.8）的冲突：3 处 `.github/workflows/release.yml`；2 处
`src-tauri/src/proxy/forwarder.rs`；2 处 `src-tauri/Cargo.toml`；2 处 `README.md`；
各 1 处 `.github/workflows/ci.yml`、`package.json`、`src-tauri/src/lib.rs`、
`src-tauri/src/commands/import_export.rs`、
`src-tauri/src/services/provider/codex_official_models.rs`、`src/App.tsx`。

第二次（v4.0.9）只有 4 处，都很好判：

| 文件 | 怎么解 |
| --- | --- |
| `package.json`、`src-tauri/Cargo.toml` | 取上游的**版本号**，留 fork 的 `description` |
| `README.md` | 整块取 fork 的——上游同一处加了一大段赞助商表格和桌面版安装说明，fork 的 README 是给无头服务端重写的 |
| `src-tauri/src/commands/settings.rs` | 取 fork 的：fork 把 `merge_settings_for_save` 挪进了 `crate::settings`（web 层也要用）；**但要把上游新加的 `incoming.proxy_client_host = existing...` 搬进 `crate::settings` 那份副本**，否则上游新增的测试会红 |

第三次（v4.0.14）5 个文件 6 处，也都是机械冲突：

| 文件 | 怎么解 |
| --- | --- |
| `package.json`、`src-tauri/Cargo.toml` | 取上游版本 `4.0.14`，留 fork 的 `description`；上游新增的 `tokio/net`、`png` 依赖自动合入 |
| `README.md`（2 处） | 整块取 fork 的，上游只在里面加赞助商和桌面版安装说明 |
| `src-tauri/src/lib.rs` | 取上游的深链接注释与新逻辑，保留 fork 的 `#[cfg(feature = "desktop")]` 门 |
| `src/components/shell/Sidebar.tsx` | 取上游的 `todayTrailing` / `trailingLabel`，再补回 fork 的 `feature: "pageUsage"` |

这次虽然冲突少，但**上游改了 `AppType::has_route_mode`（Claude Desktop 并入直连/路由模式）**，web 层要跟着改，见下一节。

三次都是「两边都改了同一处」的机械冲突，没有需要重新设计的。**想看当前会冲突什么，
自己干跑一次即可**：

```bash
git fetch upstream
git merge --no-commit --no-ff upstream/main   # 在同步分支上
git diff --name-only --diff-filter=U          # 冲突文件
git merge --abort                             # 看完撤销
```

## 冲突高发区

| 类别 | 文件 | 说明 |
| --- | --- | --- |
| feature 门 | `src-tauri/src/lib.rs`、`commands/*`、`services/*`、`proxy/*` | 为 `server` feature 加的 `#[cfg(feature = "desktop")]`；上游一动这些文件就会冲突 |
| 构建配置 | `src-tauri/Cargo.toml`、`build.rs` | `desktop`/`server` feature 声明、`dist-web` 占位逻辑 |
| 文档与流水线 | `README*`、`.github/workflows/*` | fork 的发布面（`release-server.yml` 等） |
| **基本不会冲突** | `src-tauri/src/web/**`、`src/web-shims/**`、`scripts/e2e/**`、`docs/web-mode-zh.md` | fork 自己的新文件，上游不会碰 |

唯一要动脑的情况：**上游改了 `web/` 用到的 service 函数签名或命令语义**。这时改 `web/routes.rs`
那层参数解析即可——业务逻辑在 `services/` 里只有一份，不用重复实现。

### 已固化的适配点（每次同步都要复查）

- `require_proxy_app`（有 `proxy_config` 的四个应用）与 `require_mode_app`
  （`AppType::has_route_mode()`，含 Claude Desktop）在 `web/routes.rs` 里是分开的，
  和 `commands/proxy.rs` 保持一致；
- `stop_proxy_server` 的「仍有应用接管」判断要包含 `takeover.claude_desktop`；
- 桌面新增的命令如果 web 页面会调用，要么在 `web/routes.rs` 里接一层，要么加
  `capabilities` 开关把入口隐藏。v4.0.14 新增的 `get_custom_icons` / `add_custom_icon` /
  `rename_custom_icon` / `delete_custom_icon` 已接入；深链接（`peek_pending_deeplink` 等）
  和「打开终端」在 web 下没有意义，前端已按 `isTauriRuntime()` / `pickDirectory` 能力隐藏；
- 设置保存的副作用要和 `commands/settings.rs` 对齐。v4.0.14 新增的 Codex 聚合设置
  （`codex_stack_classic_tools` 等）在 web 的 `save_settings` 里也会调
  `resync_codex_stack_catalog`，失败时回滚。
- 启动期任务也要跟 `lib.rs` 的 `setup` 对齐。`web/state.rs` 已补上
  `maybe_migrate_codex_legacy_official_history_to_openai_bucket`（排在统一迁移之前），
  会话日志兜底扫描也从 60 秒改成 10 分钟，和桌面版一致。

## 上游有、本 fork 删掉的文件

我们删掉的文件在合并时会**自己保持删除**；只有**上游同时也改了它**才会变成 modify/delete
冲突——那时按这张表决定要不要再删一次：

| 文件 | 为什么删 |
| --- | --- |
| `.github/workflows/wsl2-nightly.yml` | 上游每晚在 Windows runner 上把 `HOME`/`TEMP` 指到 WSL 的 9P 共享（`\\wsl.localhost`），验的是「**桌面版 + 家目录在 WSL 里**」的 SQLite 锁与原子写。fork 只发无头服务端（Windows 用普通 home），没有产品覆盖，每天白占一台 Windows runner。想看这个场景可以手动跑上游那份（`workflow_dispatch`）。 |

## 合并后必须验证

**两边都跑，一条都别省**——2026-10-11 那次合并就是因为只跑了 `cargo check` 和前端的部分命令，
漏掉 `cargo test` 与 `pnpm format:check`，推上去才发现三个平台的 `Run tests` 和
`Frontend Checks → Check formatting` 全红（前者是移植改动导致一个上游测试断言失败，
后者是手写的 import 不符合 prettier）。

前端 + 桌面（对应 CI 的 `frontend` / `backend` job）：

```bash
pnpm typecheck && pnpm format:check && pnpm test:unit
cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

服务端（对应 CI 的 `server` job）：

```bash
pnpm build:web
cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --features server --bin cc-switch-server -- -D warnings
cargo build  --manifest-path src-tauri/Cargo.toml --no-default-features --features server --bin cc-switch-server
bash scripts/e2e/run-all.sh
```

两点经验：

- `cargo clippy` / `cargo check` **不编译 `#[cfg(test)]` 代码**。移植改动破坏了测试断言时，
  只有 `cargo test` 会告诉你（还可能连锁：`--lib` 挂了后面 15 个集成测试目标根本不会跑，
  所以看到 "test failed" 要确认每个 target 都出过 `test result:` 行）。
- 手写的 import / 多行表达式过一遍 `pnpm format:check`，别等 CI。

（`CONTRIBUTING.md` 里也列了这些。）

## 合并完更新基线

把 `docs/upstream-v4.0.14.lock` 改名成新的基线并更新内容，例如合到上游 `v4.1.0` 之后：

```bash
git mv docs/upstream-v4.0.14.lock docs/upstream-v4.1.0.lock
# 改 upstream_tag / upstream_commit / last_merged_commit / resolved_at
```

`upstream_commit` 取 **tag peel 后的 commit**（`git ls-remote upstream 'refs/tags/v4.1.0*'`
能看到 tag 对象和它指向的 commit），`last_merged_commit` 填这次 merge 进来的上游最新提交。

## 发版

同步上游**不会**自动发版。要让新特性进到下载的产物里，还得打一个新 tag（`v*`），
`release-server.yml` 会重新构建三平台二进制、在各自的 runner 上跑一遍 smoke test，
再挂到 Release。
