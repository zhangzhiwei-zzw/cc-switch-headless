use crate::database::Database;
use crate::proxy::providers::codex_oauth_auth::CodexOAuthManager;
use crate::services::{ProxyService, UsageCache};
use std::sync::{Arc, OnceLock};

/// 全局应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub proxy_service: ProxyService,
    pub usage_cache: Arc<UsageCache>,
    // 内部已使用细粒度锁（accounts/access_tokens/refresh_locks），所有方法均为
    // `&self`，无需外层 RwLock；避免持有粗粒度锁跨网络刷新导致的连锁阻塞。
    pub codex_oauth_manager: Arc<CodexOAuthManager>,
}

/// 进程级 AppState 注册表。
///
/// 代理转发链路（故障转移切换）需要在**没有 `AppHandle`** 的地方拿到 AppState：
/// 桌面版在 `setup` 里注册，服务端在 `bootstrap` 里注册，之后任何地方都能取。
/// 未注册时返回 `None`（单元测试、启动早期）。
static CURRENT: OnceLock<Arc<AppState>> = OnceLock::new();

/// 注册进程级 AppState（只生效一次）。
pub fn set_current(state: Arc<AppState>) {
    if CURRENT.set(state).is_err() {
        log::warn!("[app-state] 重复注册 AppState，已忽略");
    }
}

/// 取进程级 AppState。
pub fn current() -> Option<Arc<AppState>> {
    CURRENT.get().cloned()
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(db: Arc<Database>) -> Self {
        let codex_oauth_manager =
            Arc::new(CodexOAuthManager::new(crate::config::get_app_config_dir()));
        let proxy_service = ProxyService::new(db.clone());

        Self {
            db,
            proxy_service,
            usage_cache: Arc::new(UsageCache::new()),
            codex_oauth_manager,
        }
    }
}
