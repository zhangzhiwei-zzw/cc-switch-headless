//! 宿主环境抽象：桌面版是 Tauri，服务端（`server` feature）没有 Tauri。
//!
//! 共享代码（代理转发、自动同步、后台任务）需要的宿主能力只有两类：
//! 1. 异步运行时——见下面的运行时辅助；
//! 2. 可选的 `AppHandle`——只用于托盘等**纯桌面**能力，见 [`handle`]。
//!
//! 服务端模式下 [`handle`] 恒为 `None`，调用点要么已经 `#[cfg]` 掉，要么按
//! "没有托盘"降级处理。

#[cfg(feature = "desktop")]
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// 注册桌面版 AppHandle（`setup` 里调用一次）。
#[cfg(feature = "desktop")]
pub fn set_handle(handle: tauri::AppHandle) {
    if APP_HANDLE.set(handle).is_err() {
        log::warn!("[host] AppHandle 重复注册，已忽略");
    }
}

/// 取桌面版 AppHandle；服务端没有这个类型，因此该函数只在桌面构建里存在。
#[cfg(feature = "desktop")]
pub fn handle() -> Option<tauri::AppHandle> {
    APP_HANDLE.get().cloned()
}

// ============================================================================
// 异步运行时辅助
//
// 桌面版直接复用 `tauri::async_runtime`（Tauri 自己装了 tokio 运行时）；
// 服务端用 `#[tokio::main]`，启动时把 Handle 存下来，供那些「在普通线程里
// 阻塞等待」的既有代码（`live.rs`、`codex_official_models.rs` 等）使用。
// ============================================================================

#[cfg(feature = "desktop")]
pub use tauri::async_runtime::{block_on, spawn, spawn_blocking};

#[cfg(not(feature = "desktop"))]
mod server_runtime {
    use std::future::Future;
    use std::sync::OnceLock;

    static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();

    /// 服务端启动时调用一次，记录当前 tokio 运行时句柄。
    pub fn install() {
        let _ = HANDLE.set(tokio::runtime::Handle::current());
    }

    fn handle() -> &'static tokio::runtime::Handle {
        HANDLE
            .get()
            .expect("host::server_runtime::install() 未在启动阶段调用")
    }

    /// 阻塞等待一个 future（可在任意线程调用，与 `tauri::async_runtime::block_on` 同语义）。
    pub fn block_on<F: Future>(future: F) -> F::Output {
        handle().block_on(future)
    }

    /// 后台执行一个 future。
    pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        handle().spawn(future)
    }

    /// 放到阻塞线程池执行。
    pub fn spawn_blocking<F, T>(work: F) -> tokio::task::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        handle().spawn_blocking(work)
    }
}

#[cfg(not(feature = "desktop"))]
pub use server_runtime::{block_on, install as install_runtime, spawn, spawn_blocking};

// ============================================================================
// 本机 CLI 工具的探测与执行
//
// 这几个能力的实现活在 `commands/misc.rs`（命令层只在桌面版编译），而业务层
// （`services/provider/codex_*`）要用到它们。这里开一个稳定的门面：
// 桌面版原样转发，服务端给出明确的降级行为。
// ============================================================================

/// 探测本机已安装的 CLI 工具版本。
///
/// 服务端拿不到（命令层不参与编译）时返回 `None`，调用方按"版本未知"回退。
#[cfg(feature = "desktop")]
pub fn local_tool_version(tool: &str) -> Option<String> {
    crate::commands::local_tool_version(tool)
}

#[cfg(not(feature = "desktop"))]
pub fn local_tool_version(_tool: &str) -> Option<String> {
    None
}

/// 运行已探测到的 CLI 工具命令。
#[cfg(feature = "desktop")]
pub fn run_tool_command(
    tool: &str,
    args: &[&str],
    timeout: Option<std::time::Duration>,
    extra_env: &[(&str, String)],
    working_dir: &std::path::Path,
) -> Result<std::process::Output, String> {
    crate::commands::run_detected_tool_command_with_timeout(
        tool,
        args,
        timeout,
        extra_env,
        working_dir,
    )
}

#[cfg(not(feature = "desktop"))]
pub fn run_tool_command(
    tool: &str,
    _args: &[&str],
    _timeout: Option<std::time::Duration>,
    _extra_env: &[(&str, String)],
    _working_dir: &std::path::Path,
) -> Result<std::process::Output, String> {
    Err(format!("Web 模式暂不支持运行本机 CLI 工具（{tool}）"))
}

/// 解码命令输出（桌面版在 Windows 上还要处理代码页）。
#[cfg(feature = "desktop")]
pub fn decode_command_output(bytes: &[u8]) -> String {
    crate::commands::decode_command_output(bytes)
}

#[cfg(not(feature = "desktop"))]
pub fn decode_command_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
