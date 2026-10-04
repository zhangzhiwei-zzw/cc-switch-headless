//! 全局事件出口抽象。
//!
//! 业务代码（`usage_events`、自动同步、代理、故障转移……）需要把状态变化推给
//! 前端，但推的方式在两种构建里不同：
//!
//! - 桌面版（feature = `desktop`）：交给 Tauri 的 `AppHandle::emit`；
//! - 服务端（feature = `server`）：交给广播通道，由 `/api/events` 以 SSE 推给浏览器。
//!
//! 因此这里放一个进程级的事件出口，由各自的启动流程安装实现。业务代码只调用
//! [`emit`]，不再直接持有 `AppHandle`——这也是把服务端从 Tauri 依赖里剥出来的
//! 关键一步。

use std::sync::OnceLock;

/// 事件出口：接收事件名与 JSON 负载，必须线程安全（可能从任意线程调用）。
pub type EmitFn = Box<dyn Fn(&str, serde_json::Value) + Send + Sync>;

static SINK: OnceLock<EmitFn> = OnceLock::new();

/// 安装全局事件出口。进程内只允许安装一次，重复安装会被忽略并记录警告。
pub fn set_sink(f: EmitFn) {
    if SINK.set(f).is_err() {
        log::warn!("[event-sink] 重复安装事件出口，已忽略");
    }
}

/// 事件出口是否已就绪。
///
/// 未安装时（单元测试、启动早期）调用方应当直接跳过推送，而不是报错。
pub fn is_ready() -> bool {
    SINK.get().is_some()
}

/// 发送一个事件；出口未安装时静默丢弃。
pub fn emit(event: &str, payload: serde_json::Value) {
    if let Some(f) = SINK.get() {
        f(event, payload);
    }
}
