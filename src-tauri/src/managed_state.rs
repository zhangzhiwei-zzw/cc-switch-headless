//! Tauri `.manage()` 状态容器的薄包装（newtype）。
//!
//! 这几个类型原先定义在 `commands/*.rs` 里，但 `proxy/forwarder.rs` 需要通过
//! `AppHandle::state::<T>()` 取它们，而 `commands` 模块只在桌面版（`desktop`
//! feature）编译。挪到这里后两种构建都能引用；`commands` 侧保留 `pub use`
//! 转出去，命令签名与 `lib.rs` 的 `.manage(...)` 调用都不用改。

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::proxy::providers::codex_oauth_auth::CodexOAuthManager;
use crate::proxy::providers::copilot_auth::CopilotAuthManager;
use crate::proxy::providers::xai_oauth_auth::XaiOAuthManager;
use crate::services::skill::SkillService;

/// Codex OAuth 认证状态
///
/// `CodexOAuthManager` 内部已使用细粒度锁且所有方法均为 `&self`，因此这里
/// 直接持有 `Arc`，不再包一层 `RwLock`——避免任一命令持有粗粒度锁跨网络刷新
/// 时阻塞其他命令（切换 / 认证中心操作 / token 读取）。
pub struct CodexOAuthState(pub Arc<CodexOAuthManager>);

/// Copilot 认证状态
pub struct CopilotAuthState(pub Arc<RwLock<CopilotAuthManager>>);

/// xAI (SuperGrok) OAuth 认证状态
pub struct XaiOAuthState(pub Arc<RwLock<XaiOAuthManager>>);

/// SkillService 状态包装
pub struct SkillServiceState(pub Arc<SkillService>);
