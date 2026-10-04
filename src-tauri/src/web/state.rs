//! 服务端启动流程。
//!
//! 复刻 `lib.rs` 中 `.setup()` 里**不依赖 Tauri** 的部分，顺序保持一致：
//! 配置目录 → 日志 → 数据库预检 → 建库 → 旧 JSON 迁移 → 崩溃恢复 → 种子
//! → 出站代理客户端 → 后台任务。桌面版独有的对话框、托盘、窗口、Updater
//! 在这里没有对应物。

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app_config::AppType;
use crate::init_status::InitErrorPayload;
use crate::store::AppState;

/// Tauri Store 文件所在的应用数据目录名（与 `tauri.conf.json` 的 identifier 一致）。
const APP_IDENTIFIER: &str = "com.ccswitch.desktop";

/// 执行启动流程。
///
/// 数据库不可用（版本过新或初始化失败）时返回 `None`：此时已写入 init_error，
/// 前端会渲染「升级应用」恢复界面，其余命令统一报错。
pub fn bootstrap() -> Option<Arc<AppState>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 1. 配置目录覆盖：环境变量 → app_paths.json → ~/.cc-switch
    resolve_config_dir_override();

    crate::panic_hook::init_app_config_dir(crate::config::get_app_config_dir());
    init_logging();
    log::info!(
        "=== CC Switch server v{} started ===",
        env!("CARGO_PKG_VERSION")
    );

    let app_config_dir = crate::config::get_app_config_dir();
    if let Err(error) = std::fs::create_dir_all(&app_config_dir) {
        log::warn!("创建配置目录 {app_config_dir:?} 失败: {error}");
    }
    let db_path = app_config_dir.join("cc-switch.db");
    let json_path = app_config_dir.join("config.json");

    // 2. 预检：磁盘上的库比本进程支持的还新时不要碰它
    match crate::database::Database::stored_user_version_exceeds_supported(&db_path) {
        Ok(Some(version)) => {
            log::error!(
                "数据库版本过新（磁盘 {version} > 支持 {}），拒绝打开",
                crate::database::SCHEMA_VERSION
            );
            crate::init_status::set_init_error(InitErrorPayload {
                path: db_path.to_string_lossy().to_string(),
                error: format!(
                    "数据库版本 {version} 高于本程序支持的 {}",
                    crate::database::SCHEMA_VERSION
                ),
                kind: Some("db_version_too_new".to_string()),
                db_version: Some(version),
                supported_version: Some(crate::database::SCHEMA_VERSION),
            });
            return None;
        }
        Ok(None) => {}
        Err(error) => log::warn!("预检数据库版本失败，继续初始化: {error}"),
    }

    // 3. 旧 config.json 预加载（与桌面版一致：先验证，再建库）
    let has_json = json_path.exists();
    let has_db = db_path.exists();
    let migration_config = if !has_db && has_json {
        match crate::app_config::MultiAppConfig::load() {
            Ok(config) => {
                log::info!("检测到旧版 config.json，将在建库后迁移");
                Some(config)
            }
            Err(error) => {
                // 服务端没有交互式重试对话框：跳过迁移，保留文件，绝不丢数据
                log::error!("加载旧配置文件失败，已跳过迁移（文件保持原样）: {error}");
                None
            }
        }
    } else {
        None
    };

    // 4. 打开数据库
    let db = match crate::database::Database::init() {
        Ok(db) => Arc::new(db),
        Err(error) => {
            log::error!("初始化数据库失败: {error}");
            crate::init_status::set_init_error(InitErrorPayload {
                path: db_path.to_string_lossy().to_string(),
                error: error.to_string(),
                kind: None,
                db_version: None,
                supported_version: None,
            });
            return None;
        }
    };

    // 5. 应用持久化的日志级别
    match db.get_log_config() {
        Ok(config) => log::set_max_level(config.to_level_filter()),
        Err(error) => {
            log::set_max_level(log::LevelFilter::Info);
            log::warn!("读取日志配置失败，已回退到 info: {error}");
        }
    }

    // 6. 旧配置迁移
    if let Some(config) = migration_config {
        match db.migrate_from_json(&config) {
            Ok(_) => {
                log::info!("✓ 配置迁移成功");
                crate::init_status::set_migration_success();
                let archive_path = json_path.with_extension("json.migrated");
                match std::fs::rename(&json_path, &archive_path) {
                    Ok(()) => log::info!("✓ 旧配置已归档为 config.json.migrated"),
                    Err(error) => log::warn!("归档旧配置文件失败: {error}"),
                }
            }
            Err(error) => log::error!("配置迁移失败: {error}，将从现有配置导入"),
        }
    }

    let state = AppState::new(db);

    // 注册进程级 AppState 与托管 OAuth 管理器：转发链路（含故障转移切换）
    // 从注册表取，服务端没有 Tauri 的 `.manage()` 容器。
    crate::store::set_current(Arc::new(state.clone()));
    {
        let app_config_dir = crate::config::get_app_config_dir();
        let copilot = Arc::new(tokio::sync::RwLock::new(
            crate::proxy::providers::copilot_auth::CopilotAuthManager::new(app_config_dir.clone()),
        ));
        crate::proxy::oauth_registry::set_copilot(copilot);

        let xai = Arc::new(tokio::sync::RwLock::new(
            crate::proxy::providers::xai_oauth_auth::XaiOAuthManager::new(app_config_dir),
        ));
        crate::proxy::oauth_registry::set_xai(xai);

        crate::proxy::oauth_registry::set_codex(state.codex_oauth_manager.clone());
    }

    // 7. 补完上次崩溃时写到一半的客户端文件；必须在任何写客户端文件的步骤之前
    crate::mode::operation::recover_on_startup(&state.db);

    // 8. 种子：先导入 live 配置，再追加官方预设（与桌面版同序）
    match state.db.init_default_skill_repos() {
        Ok(count) if count > 0 => log::info!("✓ 初始化 {count} 个默认 Skills 仓库"),
        Ok(_) => {}
        Err(error) => log::warn!("✗ 初始化默认 Skills 仓库失败: {error}"),
    }
    migrate_skills_to_ssot_if_pending(&state);
    import_live_configs_and_seed(&state);

    // 9. 全局出站代理 HTTP 客户端
    init_outbound_http_client(&state);

    // 10. 后台任务
    spawn_background_tasks(&state);

    Some(Arc::new(state))
}

/// 解析配置目录覆盖：`CC_SWITCH_CONFIG_DIR` → `app_paths.json` → 默认 `~/.cc-switch`。
fn resolve_config_dir_override() {
    if let Ok(dir) = std::env::var("CC_SWITCH_CONFIG_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            log::info!("使用 CC_SWITCH_CONFIG_DIR 指定的配置目录: {dir}");
            crate::app_store::set_app_config_dir_override(Some(PathBuf::from(dir)));
            return;
        }
    }

    // 桌面版用 Tauri Store 读写 app_paths.json；服务端直接读同一个文件
    if let Some(config_dir) = dirs::config_dir() {
        let store_path = config_dir
            .join(APP_IDENTIFIER)
            .join(crate::app_store::APP_PATHS_STORE_FILE);
        if store_path.exists() {
            crate::app_store::load_override_from_json_file(&store_path);
        }
    }
}

/// 日志：stderr + `<配置目录>/logs/cc-switch.log`（追加）。
fn init_logging() {
    let log_dir = crate::panic_hook::get_log_dir();
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录 {log_dir:?} 失败: {error}");
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("cc-switch.log"))
        .ok();

    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder.format_timestamp_millis();
    builder.target(env_logger::Target::Pipe(Box::new(TeeWriter { file })));
    let _ = builder.try_init();
}

/// 同时写 stderr 与日志文件的 writer。
struct TeeWriter {
    file: Option<std::fs::File>,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        if let Some(file) = &mut self.file {
            let _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        if let Some(file) = &mut self.file {
            let _ = file.flush();
        }
        Ok(())
    }
}

/// Skills 统一管理迁移（与桌面版一致，由数据库标记驱动）。
fn migrate_skills_to_ssot_if_pending(state: &AppState) {
    match state.db.get_setting("skills_ssot_migration_pending") {
        Ok(Some(flag)) if flag == "true" || flag == "1" => {
            let has_existing = state
                .db
                .get_all_installed_skills()
                .map(|skills| !skills.is_empty())
                .unwrap_or(false);

            if has_existing {
                log::info!("skills 表非空，跳过自动导入");
                let _ = state
                    .db
                    .set_setting("skills_ssot_migration_pending", "false");
            } else {
                match crate::services::skill::migrate_skills_to_ssot(&state.db) {
                    Ok(count) => {
                        log::info!("✓ 自动导入 {count} 个 skill 到 SSOT");
                        if count > 0 {
                            crate::init_status::set_skills_migration_result(count);
                        }
                        let _ = state
                            .db
                            .set_setting("skills_ssot_migration_pending", "false");
                    }
                    Err(error) => {
                        log::warn!("✗ 自动导入旧 skills 失败: {error}");
                        crate::init_status::set_skills_migration_error(error.to_string());
                    }
                }
            }
        }
        Ok(_) => {}
        Err(error) => log::warn!("✗ 读取 skills 迁移标记失败: {error}"),
    }
}

/// 逐应用导入 live 配置，然后追加官方预设供应商。
fn import_live_configs_and_seed(state: &AppState) {
    for app_type in AppType::all().filter(|t| !t.is_additive_mode()) {
        if !crate::services::provider::should_import_default_config_on_startup(state, &app_type)
            .unwrap_or(false)
        {
            continue;
        }

        match crate::services::provider::import_default_config(state, app_type.clone()) {
            Ok(true) => log::info!("✓ 已从 live 配置导入 {} 的默认供应商", app_type.as_str()),
            Ok(false) => {}
            Err(error) => log::debug!("○ {} 没有可导入的 live 配置: {error}", app_type.as_str()),
        }
    }

    match state.db.init_default_official_providers() {
        Ok(count) if count > 0 => log::info!("✓ 追加 {count} 个官方预设供应商"),
        Ok(_) => {}
        Err(error) => log::warn!("✗ 追加官方预设供应商失败: {error}"),
    }
}

fn init_outbound_http_client(state: &AppState) {
    let proxy_url = state.db.get_global_proxy_url().ok().flatten();
    if let Err(error) = crate::proxy::http_client::init(proxy_url.as_deref()) {
        log::error!("[GlobalProxy] 按已保存配置初始化失败: {error}");
        if proxy_url.is_some() {
            if let Err(clear_error) = state.db.set_global_proxy_url(None) {
                log::error!("[GlobalProxy] 清除无效代理配置失败: {clear_error}");
            }
        }
        if let Err(fallback_error) = crate::proxy::http_client::init(None) {
            log::error!("[GlobalProxy] 直连模式初始化失败: {fallback_error}");
        }
    }
}

fn spawn_background_tasks(state: &AppState) {
    // Codex 历史迁移（阻塞线程池，失败只记日志）
    {
        let db = state.db.clone();
        crate::host::spawn_blocking(move || {
            crate::codex_history_migration::maybe_migrate_codex_third_party_history_provider_bucket(
                &db,
            )
            .ok();
            crate::codex_history_migration::maybe_migrate_codex_provider_template_bucket(&db).ok();
            crate::codex_history_migration::maybe_migrate_codex_official_history_to_unified_bucket(
            )
            .ok();
        });
    }

    // 与桌面版一致的前后台顺序：清理泄漏凭据 → 定模式 → 官方模型检查 → 备份 → 会话同步
    let state_for_startup = state.clone();
    crate::host::spawn(async move {
        if let Err(error) =
            crate::services::provider::ProviderService::scrub_leaked_gemini_common_config(
                &state_for_startup,
            )
            .await
        {
            log::warn!("清理 Gemini 通用配置泄漏凭据失败: {error}");
        }

        // 服务端模式下代理路由不可用（`ProxyService::start` 会直接报错），
        // 这里只负责把直连模式定下来，让供应商切换立刻生效。
        crate::mode::controller::startup(&state_for_startup).await;

        crate::services::provider::codex_official_models::start_background_checks(
            state_for_startup.clone(),
        );

        if let Err(error) = state_for_startup.db.periodic_backup_if_needed() {
            log::warn!("启动期定期备份失败: {error}");
        }
    });

    // 每日维护
    {
        let db = state.db.clone();
        crate::host::spawn(async move {
            const PERIODIC_MAINTENANCE_INTERVAL_SECS: u64 = 24 * 60 * 60;
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                PERIODIC_MAINTENANCE_INTERVAL_SECS,
            ));
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(error) = db.periodic_backup_if_needed() {
                    log::warn!("定期维护失败: {error}");
                }
            }
        });
    }

    // 会话日志用量同步：启动一次（含费用回填），之后每 60 秒
    {
        let db = state.db.clone();
        crate::host::spawn(async move {
            const SESSION_SYNC_INTERVAL_SECS: u64 = 60;
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(SESSION_SYNC_INTERVAL_SECS));

            loop {
                interval.tick().await;
                if !crate::settings::get_settings().session_auto_sync_enabled {
                    continue;
                }
                let _guard = crate::services::session_usage::session_sync_mutex()
                    .lock()
                    .await;
                let db = db.clone();
                let task = crate::host::spawn_blocking(move || {
                    crate::services::session_usage::sync_all_unlocked(&db)
                });
                if let Ok(result) = task.await {
                    if !result.errors.is_empty() {
                        log::warn!("会话用量同步有 {} 个错误", result.errors.len());
                    }
                }
            }
        });
    }
}
