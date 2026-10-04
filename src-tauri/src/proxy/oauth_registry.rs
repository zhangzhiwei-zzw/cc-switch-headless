//! 托管 OAuth 管理器的进程级注册表。
//!
//! 桌面版把这些管理器放进 Tauri 的 `.manage()` 容器里，转发链路用
//! `AppHandle::state::<T>()` 取。服务端没有这套容器，因此这里再存一份：
//! 两种构建都在启动时注册，转发链路只认这里——`proxy/forwarder.rs` 于是
//! 彻底不再依赖 `AppHandle`。
//!
//! 未注册时取值为 `None`：对应"没有托管账号"的情形，调用方已有明确的错误分支。

use std::sync::{Arc, OnceLock};

use tokio::sync::RwLock;

use super::providers::codex_oauth_auth::CodexOAuthManager;
use super::providers::copilot_auth::CopilotAuthManager;
use super::providers::xai_oauth_auth::XaiOAuthManager;

static CODEX: OnceLock<Arc<CodexOAuthManager>> = OnceLock::new();
static COPILOT: OnceLock<Arc<RwLock<CopilotAuthManager>>> = OnceLock::new();
static XAI: OnceLock<Arc<RwLock<XaiOAuthManager>>> = OnceLock::new();

/// 注册 Codex (ChatGPT) OAuth 管理器。
pub fn set_codex(manager: Arc<CodexOAuthManager>) {
    if CODEX.set(manager).is_err() {
        log::warn!("[oauth-registry] CodexOAuthManager 重复注册，已忽略");
    }
}

/// 注册 GitHub Copilot 管理器。
pub fn set_copilot(manager: Arc<RwLock<CopilotAuthManager>>) {
    if COPILOT.set(manager).is_err() {
        log::warn!("[oauth-registry] CopilotAuthManager 重复注册，已忽略");
    }
}

/// 注册 xAI (SuperGrok) OAuth 管理器。
pub fn set_xai(manager: Arc<RwLock<XaiOAuthManager>>) {
    if XAI.set(manager).is_err() {
        log::warn!("[oauth-registry] XaiOAuthManager 重复注册，已忽略");
    }
}

pub fn codex() -> Option<Arc<CodexOAuthManager>> {
    CODEX.get().cloned()
}

pub fn copilot() -> Option<Arc<RwLock<CopilotAuthManager>>> {
    COPILOT.get().cloned()
}

pub fn xai() -> Option<Arc<RwLock<XaiOAuthManager>>> {
    XAI.get().cloned()
}
