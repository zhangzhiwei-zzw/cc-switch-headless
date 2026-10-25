# 同步上游（farion1231/cc-switch）

这个仓库是 [farion1231/cc-switch](https://github.com/farion1231/cc-switch) 的 fork，
上游会持续加新供应商预设、新 CLI 支持、修代理和会话解析的 bug。这份文档是怎么把它们
合进来、以及合并时要注意什么。

基线记录在 [`upstream-v4.0.20.lock`](upstream-v4.0.20.lock)：那个文件里的
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
git switch main
git status --short                            # 有未提交修改时先保存，保持工作区干净
git fetch origin                             # 如需核对自己的远程，按需执行
git merge --ff-only origin/main               # 远程无新增提交时无需这一步
git fetch upstream --prune --tags
git log --oneline HEAD..upstream/main          # 查看这次上游变化
git diff --stat HEAD...upstream/main -- src-tauri src

git merge --no-commit --no-ff upstream/main
# 解冲突、适配 Web、验证、更新 lock 和本文记录
git add <已检查的修改文件>
git commit                                   # 完成合并，仅保存在本地
# 只有明确需要发布时再 git push origin main
```

日常只维护本地 `main` 与远程跟踪引用 `upstream/main`。后者由 `git fetch upstream`
更新，表示上游的最新内容；不需要另建本地 `upstream` 分支，也不需要每次创建
`sync/upstream-日期`。本 fork 的 Web 适配保留在本地 `main`，所以它不会与上游逐文件相同。
旧的两个 `sync/upstream-*` 分支已合入 `main`，可用 `git branch -d <分支名>` 安全删除；
删除分支名称不删除已合并的提交。`origin` 是我们自己的仓库，`upstream` 是原项目，
同步上游和推送自己的远程是两件独立操作。

**这个 fork 是自己用的，直接合 `main` 即可，不需要 PR。** 注意 `ci.yml` 只在
**push `main`** 或 **PR → `main`** 时触发：推送同步分支本身不会跑 CI，合并到 `main`
之后的 push 才会。CI 里的 `Server Checks (headless)` 会把 server feature 编一遍并跑
完整 e2e，合并后看它的结果即可。

**本机 22 端口不通时**（`kex_exchange_identification: Connection closed`），
把推送地址换成 443：

```bash
git push ssh://git@ssh.github.com:443/zhangzhiwei-zzw/cc-switch-headless.git main
```

也可以写进 `~/.ssh/config`，之后照常 `git push origin`：

```sshconfig
Host github.com
  HostName ssh.github.com
  Port 443
  User git
```

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
| 2026-10-18 | v4.0.14 → v4.0.15 + 2 commits | 20 | 93 | 4 个文件 / 5 处 |
| 2026-10-21 | v4.0.15 → v4.0.17 + 上游后续提交 | 89 | 298 | 7 个文件 |
| 2026-10-22 | v4.0.17 后续提交 → v4.0.18 + Kimi Code | 9 | 120 | 11 个文件 |
| 2026-10-25 | v4.0.18 + Kimi Code → v4.0.20 + Codex 路由修复 | 86 | 291 | 9 个文件 |

统计口径：表中「上游提交 / 上游改动文件」都是**相对 merge base** 的上游侧改动，用三点
diff 与 `rev-list` 计算，别用 `git diff A B`（会把 fork 独有的文件也算成删除）：

```bash
BASE=<上次合并进来的上游 commit>
git rev-list --count "$BASE"..upstream/main          # 上游提交数
git diff --name-only "$BASE"...upstream/main | wc -l # 上游改动文件数
```

基线比 tag 多几个提交，用 `git rev-list --count <tag-peel>..<tip>` 算——这次是
`5de6feb4..eb09fbb1 = 15`，不要凭印象写。

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
git merge --no-commit --no-ff upstream/main   # 在干净的本地 main 上
git diff --name-only --diff-filter=U          # 冲突文件
git merge --abort                             # 看完撤销
```

本次（2026-10-18，v4.0.15）在本地 `main` 上按用户要求直接合并，未推送。
冲突为 `package.json`（版本与描述）、`Cargo.toml`（版本与 Tauri 可选依赖）、
`commands/failover.rs` 和 `proxy/failover_switch.rs`（托盘刷新）。保留 fork 的共享
故障转移服务与 `desktop` 门，托盘刷新改用上游接口；新增的 `tray_panel` 模块及
Vite 面板入口只在桌面构建启用。Web 的 `set_proxy_route` 改为返回上游的切换结果。
端到端覆盖补充 Pi 状态、脚本保存、路由结果和趋势图样式持久化。

本次（2026-10-21，v4.0.17）从 `sync/upstream-2026-10` 合并上游 `main`。
上游新增 Antigravity CLI 与账号管理、OpenCode 默认模型、托盘面板及多项 Codex/代理修复。
保留 fork 的 headless README、server feature 门与 Web 登录能力；Web 增加 Antigravity
本机模型发现和 OpenCode 默认模型读写，托管账号中心仍由 `pageAuth: false` 明确隐藏。
冲突涉及 `README.md`、`package.json`、`Cargo.toml`、Provider 命令、`lib.rs`、MCP 服务和
`App.tsx`。命令覆盖契约补入 11 个应用及新命令，Antigravity 托管账号查询列为 Web 未支持。

本次（2026-10-22）合并至上游 `e4873bc8`（v4.0.18 后 1 个提交）。保留上一轮 Web
安全与部署修复；新增 Kimi Code 的 Web 应用声明和配置目录映射，复用上游供应商、
MCP、Skills、提示词、会话和用量服务。Kimi 使用共存模式，不支持本地路由，
默认模型仍在 CLI 中用 `/model` 选择；Web 不提供 CLI 安装、OAuth 登录或终端启动。
Unix 提升文件描述符软上限的修复同步应用到服务端入口。

本次合并的检查：前端类型与格式、Web 构建、命令覆盖（327 个前端命令、168 个服务端
命令、12 个应用）、能力文档一致性、Rust 格式、server Clippy（`-D warnings`）及
server 二进制构建通过。本次未运行单元测试、HTTP/浏览器回归或跨平台发布构建。

### 2026-10-25：直接在 main 同步 v4.0.20 后续修复

合并至 `de0a235a`，保留前两批接口契约与发布保护，以及第三批的会话限制、预压缩、
CSP 和请求诊断。新增 Oh My Pi（第 13 个应用）的配置目录、启动导入与原生状态桥接；
Kimi 新的供应商添加接口、Pi/omp 用量脚本参数同步到 HTTP 层。用量页新增的最早记录日期
查询已接入，避免热力图出现未实现接口。Antigravity 的后台模型发现同步隐藏 Windows 控制台窗口。

托管 Kimi OAuth 登录、官方模型发现与额度仍未接入 Web，相关入口按能力隐藏；
已有账号和凭据继续由服务端 CLI 管理。桌面版新增的安装、应用排序和浏览器登录能力
仍遵循原有 Web 能力边界。两个旧同步分支均已合入 main，本轮清理后不再按日期创建分支。

本轮验证：前端类型与格式、Web 构建、命令覆盖（337 个前端命令、171 个服务端命令、
13 个应用）、13 项 RPC 契约及能力文档一致性通过；Rust 格式、server `cargo check`
和 Clippy（`-D warnings`）通过。修复 Kimi 共享服务对桌面托盘语言函数的直接依赖；
server 读取设置语言、缺省按系统区域选择。为兼容已有 HTTP 调用，Pi 用量脚本的 `app`
参数缺省仍按 `pi` 处理，新前端显式传入 `pi` 或 `omp`。

本轮未运行单元测试、HTTP/浏览器回归、桌面编译或跨平台发布构建。使用 dev 无调试符号
配置，只做 Rust 检查，结束时 target 约 814 MiB；没有重建测试或 release 产物。

### v4.0.18 发布前的 CI 修复

上次提交 `c84be35` 的 [CI #23](https://github.com/zhangzhiwei-zzw/cc-switch-headless/actions/runs/39626013401)
在 Linux、macOS、Windows 的桌面 Clippy 阶段失败，Web/server 检查通过。本地复现了
`services/provider/usage.rs` 中 8 个 `dead_code` 错误：Web 专用的用量分发函数与常量
被无条件编进桌面构建。对应接口改为 `server` feature 编译；凭据解析辅助函数保留
`server` 或 `test` 编译，继续覆盖已有测试。没有放宽 `-D warnings`。

修复后桌面/server Clippy 均通过，Kimi 和供应商相关 67 项单元测试、10 项浏览器回归
以及 172 项 HTTP、安全和安装回归通过。Windows Release 的 `.build.txt` 同时补齐上游提交与标签；发布步骤要求三平台
元数据完整且源码提交、上游提交和上游标签分别一致。

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

- 先运行 `pnpm check:web`：新增前端命令不能静默漏过 Web；更新
  `src/web-shims/command-contract.json` 时，未实现命令必须保留明确原因；
- Web 的登录、能力异常、断连和多实例行为由 `pnpm test:web` 的真实浏览器回归覆盖，
  修改入口或桥接层后也要运行。运行准备见 `web-mode-zh.md`。


- v4.0.15 的 `set_proxy_route` 返回 `RouteSwitchResult`（`kind` / `clientConfigChanged`），
  Web 必须序列化真实结果，不能继续返回 `true`，否则路由切换提示会丢失；
- Coding Plan 查询和智谱无套餐时的余额回退统一在 `services/provider/usage.rs::query_coding_plan_usage`；
  同步上游时不要在桌面命令或 Web 桥接层重新实现这段业务；
- Pi 的 `get_pi_current_state` / `get_pi_session_discovery` /
  `update_pi_provider_usage_script` 必须在 Web 注册；只合并上游业务实现不会自动接通 HTTP；
- 新增 `tray_panel` Rust 模块只在 `desktop` feature 下编译；Vite 的托盘面板入口也只在
  桌面构建加入，Web 仅构建主页面，避免引入桌面窗口 API；
- 故障转移托盘刷新采用上游 `refresh_tray_menu`，共享代理模块保留 `desktop` 门。

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

## 上游新增命令：接不接、怎么接

每次同步先量一下命令面变了多少。下面两个脚本都是只读的，复制即用（把 `<BASE>` 换成
上次合并进来的上游 commit）。

**1. 上游注册了哪些新命令**（对比 `generate_handler!`）：

```bash
python3 - <<'PY'
import re, subprocess
def cmds(rev):
    src = subprocess.run(['git','show',f'{rev}:src-tauri/src/lib.rs'],
                         capture_output=True, text=True).stdout
    block = re.search(r'generate_handler!\s*\[(.*?)\n\s*\]', src, re.S).group(1)
    return {x.strip().rstrip(',').split('::')[-1] for x in block.split(',') if x.strip()}
old, new = cmds('<BASE>'), cmds('upstream/main')
print('新增:', sorted(new - old))
PY
```

**2. 前端新调用了哪些命令**（对比 `src/**` 里的 `invoke("...")` 字面量）：

```bash
python3 - <<'PY'
import re, subprocess
def cmds(rev):
    files = subprocess.run(['git','ls-tree','-r','--name-only',rev,'src'],
                           capture_output=True, text=True).stdout.splitlines()
    out = set()
    for f in files:
        if f.endswith(('.ts','.tsx')):
            src = subprocess.run(['git','show',f'{rev}:{f}'],
                                 capture_output=True, text=True).stdout
            out |= set(re.findall(
                r'invoke(?:<[^>]*>)?\(\s*["\']([a-zA-Z0-9_]+)["\']', src))
    return out
print('新增:', sorted(cmds('upstream/main') - cmds('<BASE>')))
PY
```

**注意**：第 1 个脚本只能发现**新增**命令。上游新页面开始调用一个**旧命令**时（v4.0.14 的
Claude Desktop 三个命令就是这种情况），只能靠第 2 个脚本发现。

判定规则：

| 情况 | 做法 |
| --- | --- |
| 命令属于 web 已支持的页面（`pageProviders` / `pageUsage` / `pageMcp` …） | 在 `web/routes.rs` 接一层，补 e2e |
| 命令属于 `pageApps: false`、`updater: false` 的页面（工具安装/卸载、自动更新） | 不接，靠能力表隐藏入口 |
| 桌面专属（深链、终端、文件对话框、目录选择） | 不接，前端用 `isTauriRuntime()` / `pickDirectory` / `openInFileManager` 隐藏 |
| 新的业务语义（模式、设置保存副作用、启动迁移） | 对齐 `commands/*.rs` 的实现，见上一节 |

接入时**不要**复制业务逻辑：`web/routes.rs` 只做参数解析与 `services/` 调用。

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

内存有限的本地环境（例如 8–12 GiB 的 WSL2）请用 `cargo test -j 1`，并让
server / desktop 构建串行执行。多个大型测试二进制同时链接会耗尽内存和交换空间，
连前端测试也会被拖慢；这次同步实际遇到了这个问题，停止并发构建后恢复。

Linux / WSL2 上如果剩余配置读写集成测试被磁盘同步拖慢，可单独把它们的临时目录放到
内存文件系统（先确认 `/dev/shm` 空间够用）：

```bash
mkdir -p /dev/shm/cc-switch-test-tmp
TMPDIR=/dev/shm/cc-switch-test-tmp cargo test --manifest-path src-tauri/Cargo.toml -j 1 \
  --test provider_service --test proxy_commands --test skill_sync
```

2026-10-18 本次同步的验证结果：

- 类型检查、前端格式、Rust 格式、桌面 / Web 前端构建、桌面 / server Clippy 与 server 构建均通过；
- Web 完整端到端 7 组、140 项通过。首次安全用例遇到 60 秒启动超时，后续整套重跑全部通过；
- 前端整套 3034 项通过、1 项因自身的 10 秒限制超时；对应 `App.test.tsx` 单独重跑 22 项全部通过；
- Rust 的 17 个测试目标全部覆盖，合计 3967 项通过、9 项忽略、0 项失败。
  单元测试和前面的集成目标（包括 golden）使用默认磁盘临时目录，最后的
  `provider_service` / `proxy_commands` / `skill_sync` 改用上述内存临时目录完成。

三点经验：

- `cargo clippy` / `cargo check` **不编译 `#[cfg(test)]` 代码**。移植改动破坏了测试断言时，
  只有 `cargo test` 会告诉你（还可能连锁：`--lib` 挂了后面的集成测试目标根本不会跑，
  所以看到 "test failed" 要确认每个 target 都出过 `test result:` 行）。
- 手写的 import / 多行表达式过一遍 `pnpm format:check`，别等 CI。
- 本机是 WSL2：完整 `cargo test` 约 25 分钟（`tests/golden` 一个目标就 6 分钟，fsync 慢），
  放后台跑；`pnpm test:unit` 整套会因负载出现大面积 5 秒超时（不是断言失败），复核时
  按文件串行跑 `pnpm exec vitest run --no-file-parallelism <files>`，个别慢文件加
  `--testTimeout=30000`。最终以 CI（原生 runner）的结果为准。

（`CONTRIBUTING.md` 里也列了这些。）

## 合并后的收尾清单

版本号不止 `package.json` / `Cargo.toml` 两处，合并完统一搜一遍旧版本号（把 `v4.0.9`
换成这次的旧基线）：

```bash
git grep -n "v4\.0\.9" -- README* docs/syncing-upstream-zh.md docs/web-mode-zh.md .github scripts
```

必须跟着改的：

| 位置 | 改什么 |
| --- | --- |
| `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` | 版本号（`tauri.conf.json` 通常随上游一起变） |
| `README.md`、`README_EN.md` | 徽章里的基线版本 |
| `docs/web-mode-zh.md` | 开头「当前对标基线」与 lock 链接 |
| `docs/syncing-upstream-zh.md` | 本文的基线链接、冲突实测表新增一行、已固化适配点 |
| `.github/workflows/release-server.yml` | 发版正文里的上游版本号（**v4.0.14 这次就漏过，发布前才发现要补**） |
| `docs/upstream-*.lock` | 改名为新基线并更新内容（见下一节） |

## 合并完更新基线

把 `docs/upstream-v4.0.20.lock` 改名成新的基线并更新内容，例如合到上游 `v4.1.0` 之后：

```bash
git mv docs/upstream-v4.0.20.lock docs/upstream-v4.1.0.lock
# 改 upstream_tag / upstream_commit / last_merged_commit / resolved_at
```

`upstream_commit` 取 **tag peel 后的 commit**（`git ls-remote upstream 'refs/tags/v4.1.0*'`
能看到 tag 对象和它指向的 commit；本地可用 `git rev-parse 'v4.1.0^{}'`），
`last_merged_commit` 填这次 merge 进来的上游最新提交（通常是 `upstream/main` 的 tip）。

`last_merged_commit` 往往比 tag 多几个提交（上游 tag 之后还在继续修 bug），多几个用
`git rev-list --count <tag-peel>..<last_merged_commit>` 算，并写进 lock 的注释里。

## 发版

同步上游**不会**自动发版。要让新特性进到下载的产物里，还得打一个新 tag（以 v4.0.14
这次为例，`e55a4d20` 是当天的 `main`）：

```bash
git tag -a v4.0.14-headless e55a4d20 \
  -m "cc-switch-headless v4.0.14: headless server on the upstream v4.0.14 baseline"
git push ssh://git@ssh.github.com:443/zhangzhiwei-zzw/cc-switch-headless.git v4.0.14-headless
```

注意：

- tag 必须用 **`vX.Y.Z-headless`**，不要用上游的 `vX.Y.Z`（会撞名）；
- `release-server.yml` 由 `v*-headless*` 标签推送触发，构建三条腿（Linux x86_64 / aarch64 / Windows）；
  Linux 检查 glibc ≤ 2.31 且不链 OpenSSL，各平台执行 smoke test；
- 产物元数据必须来自同一提交与上游基线，公开发布还要求该提交在 main 的最新 CI 成功；
  排队或运行中最多等待 30 分钟。失败或超时后，先处理 CI，再重新运行 Release server；
- 每个平台有二进制、`.sha256` 与 `.build.txt`，发布后核对 `releases/latest` 及 9 个资产；
- `workflow_dispatch` 填写标签后会检出该标签构建，留空只构建。例外跳过 CI 需要显式填写
  `skip_ci` 和原因，记录见 [发布策略](README.md#发布)。

## 合并错了怎么回滚

已经合进 `main` 的改动出问题时，用 revert 保留历史，不要 force-push：

```bash
git revert -m 1 <merge-commit>   # 撤销这次 merge 引入的改动
```

合并尚未提交时用 `git merge --abort` 撤销，要求开始合并前工作区干净。
