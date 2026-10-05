//! `/api/invoke` 命令分发表。
//!
//! 桌面版的 302 个 `#[tauri::command]` 定义在 `commands/` 里，而那个模块只在
//! `desktop` feature 下编译。服务端因此**不复用命令函数**，而是把命令体里那层
//! 薄封装（解析参数 → 调 service 层）在这里重写一遍——业务逻辑仍然只有一份，
//! 在 `services/` 里。
//!
//! 目前实现的范围：启动链路、供应商页、设置保存、本地路由（代理）与故障转移队列、
//! 导入导出与数据库备份、会话浏览，以及 MCP / Skills / Prompts / 用量统计四页
//! （后四页各自成表，见 [`super::mcp`] 等同级模块）。
//! 未实现的命令返回 `E_NOT_IMPLEMENTED:<cmd>`，前端会以普通错误提示，不会白屏。
//! 凡是命令体里有真实逻辑的（如故障转移的开启流程），逻辑都放在 `services/` 里
//! 由两种构建共用——这里只做参数解析与调用。

use std::collections::HashMap;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

use axum::extract::State as AxumState;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use super::Context;
use crate::app_config::AppType;
use crate::provider::Provider;
use crate::services::provider::{ProviderService, ProviderSortUpdate};

/// 分发表的函数指针类型。
pub type HandlerFuture = Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send>>;
pub type Handler = fn(Arc<Context>, Value) -> HandlerFuture;

/// 前端发来的调用请求（与 Tauri 的 `invoke(cmd, args)` 一一对应）。
#[derive(serde::Deserialize)]
pub struct InvokeRequest {
    pub cmd: String,
    #[serde(default)]
    pub args: Value,
}

/// `POST /api/invoke`
pub async fn invoke(
    AxumState(context): AxumState<Arc<Context>>,
    Json(request): Json<InvokeRequest>,
) -> Response {
    // 数据库不可用（版本过新/初始化失败）时只放行 get_init_error，
    // 让前端渲染恢复界面；其余命令给出明确错误。
    if context.state.is_none() && request.cmd != "get_init_error" {
        return Json(json!({
            "ok": false,
            "error": "E_INIT: 数据库未就绪，请查看服务端日志",
        }))
        .into_response();
    }

    let Some(handler) = dispatch_table().get(request.cmd.as_str()).copied() else {
        return Json(json!({
            "ok": false,
            "error": format!("E_NOT_IMPLEMENTED: {}", request.cmd),
        }))
        .into_response();
    };

    match handler(context, request.args).await {
        Ok(data) => Json(json!({ "ok": true, "data": data })).into_response(),
        Err(error) => Json(json!({ "ok": false, "error": error })).into_response(),
    }
}

/// `GET /api/commands`：已实现的命令名（便于排查）。
pub async fn list_commands() -> Response {
    let mut names: Vec<&str> = dispatch_table().keys().copied().collect();
    names.sort_unstable();
    Json(json!({ "commands": names })).into_response()
}

/// `GET /api/capabilities`：这个服务端有哪些能力。
///
/// 前端据此**主动隐藏**不可用的入口，而不是让用户点进去看报错。键名按界面概念命名
/// （页面级 / 页面内动作），布尔值直接对应"能不能用"。
pub async fn capabilities() -> Response {
    let table = dispatch_table();
    let has = |cmd: &str| table.contains_key(cmd);

    let features = json!({
        // —— 页面级 ——
        "pageProviders": has("get_providers"),
        "pageRouting": has("start_proxy_server"),
        "pageSessions": has("list_sessions"),
        "pageImportExport": has("export_config_to_file"),
        "pageUsage": has("get_usage_summary"),
        "pageMcp": has("get_mcp_servers"),
        "pageSkills": has("get_installed_skills"),
        "pagePrompts": has("get_prompts"),
        "pageAuth": false,    // 托管账号（Copilot / Codex / xAI）登录流程未接入
        "pageApps": false,    // CLI 工具版本检测与安装未接入

        // —— 页面内动作 ——
        "sessionStream": has("web_session_transcript"),
        "sessionReveal": false,   // 服务端没有文件管理器
        "sessionTerminal": false, // 服务端没有桌面终端
        "pickDirectory": false,   // 浏览器无法为服务端选目录
        "openInFileManager": false, // 「在文件管理器里打开」类按钮

        // —— 桌面专属 ——
        "tray": false,
        "updater": false,
    });

    let mut commands: Vec<&str> = table.keys().copied().collect();
    commands.sort_unstable();

    Json(json!({
        "mode": "web",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "features": features,
        "commands": commands,
    }))
    .into_response()
}

fn dispatch_table() -> &'static HashMap<&'static str, Handler> {
    static TABLE: OnceLock<HashMap<&'static str, Handler>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = HashMap::new();

        // ---- 启动 ----
        table.insert("get_init_error", get_init_error as Handler);
        table.insert("get_migration_result", get_migration_result as Handler);
        table.insert(
            "get_skills_migration_result",
            get_skills_migration_result as Handler,
        );
        table.insert(
            "take_startup_attach_failures",
            take_startup_attach_failures as Handler,
        );
        table.insert("take_tray_navigation", take_tray_navigation as Handler);
        table.insert("tray_app_page_seen", tray_app_page_seen as Handler);
        table.insert("check_env_conflicts", check_env_conflicts as Handler);

        // ---- 设置 ----
        table.insert("get_settings", get_settings as Handler);
        table.insert("save_settings", save_settings as Handler);
        table.insert("get_config_dir", get_config_dir as Handler);
        table.insert("open_config_folder", open_config_folder as Handler);
        table.insert("update_tray_menu", update_tray_menu as Handler);

        // ---- 供应商 ----
        table.insert("get_providers", get_providers as Handler);
        table.insert("get_current_provider", get_current_provider as Handler);
        table.insert("add_provider", add_provider as Handler);
        table.insert("update_provider", update_provider as Handler);
        table.insert("delete_provider", delete_provider as Handler);
        table.insert(
            "remove_provider_from_live_config",
            remove_provider_from_live_config as Handler,
        );
        table.insert("switch_provider", switch_provider as Handler);
        table.insert(
            "get_provider_editor_view",
            get_provider_editor_view as Handler,
        );
        table.insert("import_default_config", import_default_config as Handler);
        table.insert(
            "update_providers_sort_order",
            update_providers_sort_order as Handler,
        );
        table.insert(
            "ensure_codex_official_provider",
            ensure_codex_official_provider as Handler,
        );
        table.insert(
            "ensure_grokbuild_official_provider",
            ensure_grokbuild_official_provider as Handler,
        );

        // ---- 代理状态 / 本地路由 ----
        table.insert("get_proxy_status", get_proxy_status as Handler);
        table.insert(
            "get_proxy_takeover_status",
            get_proxy_takeover_status as Handler,
        );
        table.insert("start_proxy_server", start_proxy_server as Handler);
        table.insert("stop_proxy_server", stop_proxy_server as Handler);
        table.insert(
            "stop_proxy_with_restore",
            stop_proxy_with_restore as Handler,
        );
        table.insert(
            "get_global_proxy_config",
            get_global_proxy_config as Handler,
        );
        table.insert(
            "update_global_proxy_config",
            update_global_proxy_config as Handler,
        );
        table.insert(
            "get_proxy_config_for_app",
            get_proxy_config_for_app as Handler,
        );
        table.insert(
            "update_proxy_config_for_app",
            update_proxy_config_for_app as Handler,
        );

        // ---- 模式（直连 / 路由）----
        table.insert("get_app_mode", get_app_mode as Handler);
        table.insert(
            "set_proxy_takeover_for_app",
            set_proxy_takeover_for_app as Handler,
        );
        table.insert("set_proxy_route", set_proxy_route as Handler);
        table.insert("get_direct_provider", get_direct_provider as Handler);

        // ---- 故障转移队列 ----
        table.insert("get_failover_queue", get_failover_queue as Handler);
        table.insert(
            "get_available_providers_for_failover",
            get_available_providers_for_failover as Handler,
        );
        table.insert("add_to_failover_queue", add_to_failover_queue as Handler);
        table.insert(
            "remove_from_failover_queue",
            remove_from_failover_queue as Handler,
        );
        table.insert(
            "get_auto_failover_enabled",
            get_auto_failover_enabled as Handler,
        );
        table.insert(
            "set_auto_failover_enabled",
            set_auto_failover_enabled as Handler,
        );

        // ---- 导入导出与数据库备份 ----
        table.insert("export_config_to_file", export_config_to_file as Handler);
        table.insert(
            "import_config_from_file",
            import_config_from_file as Handler,
        );
        table.insert("create_db_backup", create_db_backup as Handler);
        table.insert("list_db_backups", list_db_backups as Handler);
        table.insert("restore_db_backup", restore_db_backup as Handler);
        table.insert("rename_db_backup", rename_db_backup as Handler);
        table.insert("delete_db_backup", delete_db_backup as Handler);
        // 浏览器里没有"保存文件对话框"：由前端 shim 调它拿一个服务端导出路径
        table.insert(
            "web_allocate_export_path",
            web_allocate_export_path as Handler,
        );

        // ---- 会话浏览 ----
        table.insert("list_sessions", list_sessions as Handler);
        table.insert("get_session_messages", get_session_messages as Handler);
        table.insert(
            "get_session_block_content",
            get_session_block_content as Handler,
        );
        table.insert("delete_session", delete_session as Handler);
        table.insert("delete_sessions", delete_sessions as Handler);
        table.insert(
            "export_session_markdown",
            export_session_markdown as Handler,
        );
        // 浏览器里没有 Channel：shim 用它读整段会话再分块喂回调
        table.insert("web_session_transcript", web_session_transcript as Handler);
        table.insert("reveal_session_path", reveal_session_path as Handler);
        table.insert(
            "launch_session_terminal",
            launch_session_terminal as Handler,
        );

        // ---- MCP / Prompts / Skills / 用量 ----
        // 这四页命令量大且各自独立，分表放在子模块里，这里只做注册。
        for (name, handler) in super::mcp::HANDLERS {
            table.insert(name, *handler);
        }
        for (name, handler) in super::prompts::HANDLERS {
            table.insert(name, *handler);
        }
        for (name, handler) in super::skills::HANDLERS {
            table.insert(name, *handler);
        }
        for (name, handler) in super::usage::HANDLERS {
            table.insert(name, *handler);
        }

        table
    })
}

// ============================================================================
// 工具函数
// ============================================================================

pub fn serializable<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

pub fn parse<T: DeserializeOwned>(raw: Value) -> Result<T, String> {
    serde_json::from_value(raw).map_err(|error| format!("参数解析失败: {error}"))
}

/// 在阻塞线程池里跑同步的 service 调用（SQLite、文件读写都在其中）。
pub async fn blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    crate::host::spawn_blocking(work)
        .await
        .map_err(|error| format!("后台任务异常退出: {error}"))?
}

/// 把一个同步调用包成 handler，并丢进阻塞线程池：SQLite 聚合、文件读写用它。
pub fn deferred<T, F>(work: F) -> HandlerFuture
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Serialize + Send + 'static,
{
    Box::pin(async move { blocking(work).await.and_then(serializable) })
}

pub fn to_app_type(app: &str) -> Result<AppType, String> {
    AppType::from_str(app).map_err(|error| error.to_string())
}

// ============================================================================
// 启动
// ============================================================================

fn get_init_error(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { serializable(crate::init_status::get_init_error()) })
}

fn get_migration_result(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { serializable(crate::init_status::take_migration_success()) })
}

fn get_skills_migration_result(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { serializable(crate::init_status::take_skills_migration_result()) })
}

fn take_startup_attach_failures(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { serializable(crate::mode::controller::take_startup_attach_failures()) })
}

/// 托盘导航在 web 模式没有对应物。
fn take_tray_navigation(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { Ok(Value::Null) })
}

fn tray_app_page_seen(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { Ok(Value::Null) })
}

fn check_env_conflicts(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        serializable(crate::services::env_checker::check_env_conflicts(&app)?)
    })
}

// ============================================================================
// 设置
// ============================================================================

fn get_settings(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { serializable(crate::settings::get_settings_for_frontend()) })
}

fn save_settings(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            settings: crate::settings::AppSettings,
        }
        let Args { settings } = parse(args)?;
        let state = context.require_state()?;

        blocking(move || {
            let existing = crate::settings::get_settings();
            let merged = crate::settings::merge_settings_for_save(settings, &existing);
            let unify_changed =
                merged.unify_codex_session_history != existing.unify_codex_session_history;

            crate::settings::update_settings(merged).map_err(|error| error.to_string())?;

            // 统一 Codex 会话开关变更时立刻重写当前官方供应商的 live 配置
            if unify_changed {
                if let Err(error) =
                    crate::services::provider::reapply_current_codex_official_live(&state)
                {
                    log::warn!("统一 Codex 会话历史开关变更后重写 live 配置失败: {error}");
                }
            }

            Ok(Value::Bool(true))
        })
        .await
    })
}

fn get_config_dir(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let dir = match to_app_type(&app)? {
            AppType::Claude => crate::config::get_claude_config_dir(),
            AppType::ClaudeDesktop => crate::claude_desktop_config::get_config_library_path()
                .map_err(|e| e.to_string())?,
            AppType::Codex => crate::codex_config::get_codex_config_dir(),
            AppType::Gemini => crate::gemini_config::get_gemini_dir(),
            AppType::GrokBuild => crate::grok_config::get_grok_config_dir(),
            AppType::OpenCode => crate::opencode_config::get_opencode_dir(),
            AppType::OpenClaw => crate::openclaw_config::get_openclaw_dir(),
            AppType::Hermes => crate::hermes_config::get_hermes_dir(),
            AppType::Pi => crate::pi_config::get_pi_agent_dir().map_err(|e| e.to_string())?,
            AppType::Mcode => crate::mcode_config::config_path()
                .parent()
                .map(|path| path.to_path_buf())
                .ok_or_else(|| "无法解析 MiniMax Code 配置目录".to_string())?,
        };
        Ok(Value::String(dir.to_string_lossy().to_string()))
    })
}

/// 服务端没有桌面会话，打不开「配置目录」窗口。
fn open_config_folder(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { Err("Web 模式下请在服务器上直接打开该目录".to_string()) })
}

/// 没有托盘；前端拿 `true` 当作「刷新成功」即可。
fn update_tray_menu(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { Ok(Value::Bool(true)) })
}

// ============================================================================
// 供应商
// ============================================================================

fn get_providers(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let providers = ProviderService::list(&state, app_type).map_err(|e| e.to_string())?;
            serializable(providers)
        })
        .await
    })
}

fn get_current_provider(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let current = ProviderService::current(&state, app_type).map_err(|e| e.to_string())?;
            serializable(current)
        })
        .await
    })
}

fn add_provider(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app: String,
            provider: Provider,
            add_to_live: Option<bool>,
            editor_save: Option<crate::services::provider::EditorSave>,
        }
        let Args {
            app,
            provider,
            add_to_live,
            editor_save,
        } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let added = ProviderService::add_from_editor(
                &state,
                app_type,
                provider,
                add_to_live.unwrap_or(true),
                editor_save,
            )
            .map_err(|e| e.to_string())?;
            Ok(Value::Bool(added))
        })
        .await
    })
}

fn update_provider(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app: String,
            provider: Provider,
            original_id: Option<String>,
            editor_save: Option<crate::services::provider::EditorSave>,
        }
        let Args {
            app,
            provider,
            original_id,
            editor_save,
        } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let updated = ProviderService::update_from_editor(
                &state,
                app_type,
                original_id.as_deref(),
                provider,
                editor_save,
            )
            .map_err(|e| e.to_string())?;
            Ok(Value::Bool(updated))
        })
        .await
    })
}

fn delete_provider(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
        }
        let Args { app, id } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            ProviderService::delete(&state, app_type, &id).map_err(|e| e.to_string())?;
            Ok(Value::Bool(true))
        })
        .await
    })
}

fn remove_provider_from_live_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
        }
        let Args { app, id } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            ProviderService::remove_from_live_config(&state, app_type, &id)
                .map_err(|e| e.to_string())?;
            Ok(Value::Bool(true))
        })
        .await
    })
}

fn switch_provider(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
        }
        let Args { app, id } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let result =
                ProviderService::switch(&state, app_type, &id).map_err(|e| e.to_string())?;
            serializable(result)
        })
        .await
    })
}

fn get_provider_editor_view(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app: String,
            settings_config: Value,
            category: Option<String>,
            provider_id: Option<String>,
        }
        let Args {
            app,
            settings_config,
            category,
            provider_id,
        } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let category = ProviderService::editor_category(
                &state,
                &app_type,
                provider_id.as_deref(),
                category,
            )
            .map_err(|e| e.to_string())?;
            let view = ProviderService::editor_view(
                &state,
                app_type,
                &settings_config,
                category.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            serializable(view)
        })
        .await
    })
}

fn import_default_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let imported = ProviderService::import_default_config(&state, app_type)
                .map_err(|e| e.to_string())?;
            Ok(Value::Bool(imported))
        })
        .await
    })
}

fn update_providers_sort_order(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            updates: Vec<ProviderSortUpdate>,
        }
        let Args { app, updates } = parse(args)?;
        let state = context.require_state()?;
        blocking(move || {
            let app_type = to_app_type(&app)?;
            let changed = ProviderService::update_sort_order(&state, app_type, updates)
                .map_err(|e| e.to_string())?;
            Ok(Value::Bool(changed))
        })
        .await
    })
}

fn ensure_codex_official_provider(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        blocking(move || {
            let ensured = state
                .db
                .ensure_official_seed_by_id(
                    crate::database::CODEX_OFFICIAL_PROVIDER_ID,
                    AppType::Codex,
                )
                .map_err(|e| e.to_string())?;
            Ok(Value::Bool(ensured))
        })
        .await
    })
}

fn ensure_grokbuild_official_provider(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        blocking(move || {
            let ensured = state
                .db
                .ensure_official_seed_by_id(
                    crate::database::GROKBUILD_OFFICIAL_PROVIDER_ID,
                    AppType::GrokBuild,
                )
                .map_err(|e| e.to_string())?;
            Ok(Value::Bool(ensured))
        })
        .await
    })
}

// ============================================================================
// 代理状态（服务端不跑本地代理，但界面需要能渲染）
// ============================================================================

fn get_proxy_status(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let status = state.proxy_service.get_status().await?;
        serializable(status)
    })
}

fn get_proxy_takeover_status(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let status = state.proxy_service.get_takeover_status().await?;
        serializable(status)
    })
}

// ============================================================================
// 本地路由（代理）
// ============================================================================

/// 只有支持本地路由的应用才有代理配置与模式。
fn require_proxy_app(app_type: &str) -> Result<AppType, String> {
    let app = AppType::from_str(app_type).map_err(|error| format!("无效的应用类型: {error}"))?;
    if !app.supports_local_proxy() {
        return Err(format!("{} 不支持本地路由", app.as_str()));
    }
    Ok(app)
}

fn start_proxy_server(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let info = state.proxy_service.start().await?;
        serializable(info)
    })
}

fn stop_proxy_server(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        // 与桌面版一致：还有应用处于接管状态时先不停止，避免 CLI 指向一个已关闭的端口
        let takeover = state.proxy_service.get_takeover_status().await?;
        if takeover.claude
            || takeover.codex
            || takeover.gemini
            || takeover.grokbuild
            || takeover.opencode
            || takeover.openclaw
        {
            return Err(
                "仍有应用处于代理接管状态，请先在设置中关闭对应应用接管后再停止本地路由。"
                    .to_string(),
            );
        }
        state.proxy_service.stop().await?;
        Ok(Value::Bool(true))
    })
}

/// 关闭本地路由：所有应用退回直连，再停止代理服务器。
fn stop_proxy_with_restore(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        crate::mode::controller::exit_all(&state).await?;
        Ok(Value::Bool(true))
    })
}

fn get_global_proxy_config(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let config = state
            .db
            .get_global_proxy_config()
            .await
            .map_err(|e| e.to_string())?;
        serializable(config)
    })
}

fn update_global_proxy_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            config: crate::proxy::types::GlobalProxyConfig,
        }
        let Args { config } = parse(args)?;
        let state = context.require_state()?;

        // 地址/端口变了就重启服务，并按新地址重写接上路由的客户端
        let restarted = state.proxy_service.update_global_config(&config).await?;
        if restarted {
            crate::mode::controller::resync_routes(&state).await?;
        }
        Ok(Value::Bool(true))
    })
}

fn get_proxy_config_for_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        require_proxy_app(&app_type)?;

        let state = context.require_state()?;
        let config = state
            .db
            .get_proxy_config_for_app(&app_type)
            .await
            .map_err(|e| e.to_string())?;
        serializable(config)
    })
}

fn update_proxy_config_for_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            config: crate::proxy::types::AppProxyConfig,
        }
        let Args { config } = parse(args)?;
        let state = context.require_state()?;

        let app_type = config.app_type.clone();
        let app = require_proxy_app(&app_type)?;
        let circuit_config = crate::proxy::CircuitBreakerConfig::from(&config);

        let mut config = config;
        // `enabled` 是模式的镜像，只由进入 / 退出代理改写
        config.enabled = crate::mode::current::is_proxy(&app);

        state
            .db
            .update_proxy_config_for_app(config)
            .await
            .map_err(|e| e.to_string())?;

        state
            .proxy_service
            .update_circuit_breaker_config_for_app(&app_type, circuit_config)
            .await?;
        Ok(Value::Bool(true))
    })
}

// ============================================================================
// 模式（直连 / 路由）
// ============================================================================

fn get_app_mode(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        let app = require_proxy_app(&app_type)?;
        let state = context.require_state()?;
        blocking(move || {
            let view =
                crate::mode::controller::app_mode_view(&state, &app).map_err(|e| e.to_string())?;
            serializable(view)
        })
        .await
    })
}

fn set_proxy_takeover_for_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            enabled: bool,
            stack: Option<bool>,
            route: Option<String>,
        }
        let Args {
            app_type,
            enabled,
            stack,
            route,
        } = parse(args)?;
        let app = require_proxy_app(&app_type)?;
        let state = context.require_state()?;

        if enabled {
            crate::mode::controller::enter_with_route(
                &state,
                &app,
                stack.unwrap_or(false),
                route.as_deref(),
            )
            .await?;
        } else {
            crate::mode::controller::exit(&state, &app).await?;
        }
        Ok(Value::Bool(true))
    })
}

fn set_proxy_route(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            provider_id: String,
        }
        let Args {
            app_type,
            provider_id,
        } = parse(args)?;
        let app = require_proxy_app(&app_type)?;
        let state = context.require_state()?;

        crate::mode::controller::set_route(&state, &app, &provider_id).await?;
        Ok(Value::Bool(true))
    })
}

fn get_direct_provider(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        let app = require_proxy_app(&app_type)?;
        let state = context.require_state()?;
        blocking(move || {
            let provider_id = crate::mode::controller::direct_provider_id(&state, &app)
                .map_err(|e| e.to_string())?;
            serializable(provider_id)
        })
        .await
    })
}

// ============================================================================
// 故障转移队列（与桌面命令共用 `services::failover`）
// ============================================================================

fn get_failover_queue(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        let state = context.require_state()?;
        let queue = crate::services::failover::get_queue(&state, &app_type).await?;
        serializable(queue)
    })
}

fn get_available_providers_for_failover(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        let state = context.require_state()?;
        let providers =
            crate::services::failover::get_available_providers(&state, &app_type).await?;
        serializable(providers)
    })
}

fn add_to_failover_queue(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            provider_id: String,
        }
        let Args {
            app_type,
            provider_id,
        } = parse(args)?;
        let state = context.require_state()?;
        crate::services::failover::add(&state, &app_type, &provider_id).await?;
        Ok(Value::Bool(true))
    })
}

fn remove_from_failover_queue(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            provider_id: String,
        }
        let Args {
            app_type,
            provider_id,
        } = parse(args)?;
        let state = context.require_state()?;
        crate::services::failover::remove(&state, &app_type, &provider_id).await?;
        Ok(Value::Bool(true))
    })
}

fn get_auto_failover_enabled(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        let state = context.require_state()?;
        let enabled = crate::services::failover::auto_failover_enabled(&state, &app_type).await?;
        serializable(enabled)
    })
}

fn set_auto_failover_enabled(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            enabled: bool,
        }
        let Args { app_type, enabled } = parse(args)?;
        let state = context.require_state()?;

        let p1_provider_id =
            crate::services::failover::set_auto_failover_enabled(&state, &app_type, enabled)
                .await?;

        if let Some(provider_id) = p1_provider_id {
            // 与桌面命令同名同 payload：前端据此刷新当前供应商
            crate::event_sink::emit(
                "provider-switched",
                json!({
                    "appType": app_type,
                    "providerId": provider_id,
                    "source": "failoverEnabled"
                }),
            );
        }
        Ok(Value::Bool(true))
    })
}

// ============================================================================
// 导入导出与数据库备份
//
// 桌面的文件对话框在浏览器里没有对应物：shim 用 `open_file_dialog` 选择文件后
// POST 到 `/api/upload`，拿到**服务端路径**再交给下面这些命令；导出方向则由
// `web_allocate_export_path` 分配一个服务端路径，导出成功后 shim 触发
// `/api/download`。
// ============================================================================

/// 为一次导出分配服务端路径（仅前端 shim 使用）。
fn web_allocate_export_path(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            default_name: String,
        }
        let Args { default_name } = parse(args)?;

        // 只取文件名部分，杜绝 `../` 穿越
        let file_name = std::path::Path::new(&default_name)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "cc-switch-export.sql".to_string());

        let dir = crate::config::get_app_config_dir().join("exports");
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建导出目录失败: {e}"))?;
        let path = dir.join(file_name);
        Ok(Value::String(path.to_string_lossy().into_owned()))
    })
}

fn export_config_to_file(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            file_path: String,
        }
        let Args { file_path } = parse(args)?;
        let state = context.require_state()?;
        crate::services::import_export::export_to_file(&state, &file_path).await
    })
}

fn import_config_from_file(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            file_path: String,
        }
        let Args { file_path } = parse(args)?;
        let state = context.require_state()?;
        crate::services::import_export::import_from_file(&state, &file_path).await
    })
}

fn create_db_backup(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let file_name = crate::services::import_export::create_backup(&state).await?;
        Ok(Value::String(file_name))
    })
}

fn list_db_backups(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        context.require_state()?;
        let backups = crate::services::import_export::list_backups()?;
        serializable(backups)
    })
}

fn restore_db_backup(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            filename: String,
        }
        let Args { filename } = parse(args)?;
        let state = context.require_state()?;
        let restored = crate::services::import_export::restore_backup(&state, &filename).await?;
        Ok(Value::String(restored))
    })
}

fn rename_db_backup(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            old_filename: String,
            new_name: String,
        }
        let Args {
            old_filename,
            new_name,
        } = parse(args)?;
        context.require_state()?;
        let renamed = crate::services::import_export::rename_backup(&old_filename, &new_name)?;
        Ok(Value::String(renamed))
    })
}

fn delete_db_backup(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            filename: String,
        }
        let Args { filename } = parse(args)?;
        context.require_state()?;
        crate::services::import_export::delete_backup(&filename)?;
        Ok(Value::Bool(true))
    })
}

// ============================================================================
// 会话浏览
//
// 读的是**服务端这台机器**上各 CLI 工具留下的会话文件——在服务器上跑 CLI
// 的场景里这正是要看的东西。桌面的终端/文件管理器相关命令在服务端没有对应物。
// ============================================================================

fn list_sessions(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let sessions = blocking(|| Ok(crate::session_manager::scan_sessions())).await?;
        serializable(sessions)
    })
}

fn get_session_messages(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            source_path: String,
        }
        let Args {
            provider_id,
            source_path,
        } = parse(args)?;

        blocking(move || {
            crate::session_manager::load_transcript(&provider_id, &source_path)
                .map(|loaded| loaded.transcript.messages.clone())
        })
        .await
        .and_then(serializable)
    })
}

/// 一次性返回整段会话（含轮次索引）。
///
/// 浏览器端不支持 Tauri 的 `Channel`，`stream_session_messages` 由前端 shim 用
/// 这个命令的数据在浏览器里分块喂给回调——分块逻辑在浏览器侧做，服务端只读一次。
fn web_session_transcript(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            source_path: String,
        }
        let Args {
            provider_id,
            source_path,
        } = parse(args)?;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Payload {
            messages: Vec<crate::session_manager::SessionMessage>,
            turns: Vec<crate::session_manager::model::TurnIndex>,
            approx_bytes: u64,
            cached: bool,
            parse_ms: u64,
        }

        blocking(move || {
            let loaded = crate::session_manager::load_transcript(&provider_id, &source_path)?;
            Ok(Payload {
                messages: loaded.transcript.messages.clone(),
                turns: loaded.transcript.turns.clone(),
                approx_bytes: loaded.transcript.approx_bytes as u64,
                cached: loaded.cached,
                parse_ms: loaded.parse_ms,
            })
        })
        .await
        .and_then(serializable)
    })
}

fn get_session_block_content(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            source_path: String,
            content_ref: crate::session_manager::model::ContentRef,
            offset: Option<u32>,
            limit: Option<u32>,
        }
        let Args {
            provider_id,
            source_path,
            content_ref,
            offset,
            limit,
        } = parse(args)?;

        blocking(move || {
            let source =
                crate::session_manager::content::validate_source(&provider_id, &source_path)?;
            let full = crate::session_manager::content::resolve_content_ref(&source, &content_ref)?;
            Ok(crate::session_manager::content::paginate(
                &full, offset, limit,
            ))
        })
        .await
        .and_then(serializable)
    })
}

fn delete_session(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            session_id: String,
            source_path: String,
        }
        let Args {
            provider_id,
            session_id,
            source_path,
        } = parse(args)?;

        blocking(move || {
            crate::session_manager::delete_session(&provider_id, &session_id, &source_path)
        })
        .await
        .and_then(serializable)
    })
}

fn delete_sessions(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            items: Vec<crate::session_manager::DeleteSessionRequest>,
        }
        let Args { items } = parse(args)?;
        blocking(move || Ok(crate::session_manager::delete_sessions(&items)))
            .await
            .and_then(serializable)
    })
}

/// 导出会话为 Markdown：服务端写进 `<配置目录>/exports/`，前端随后下载
/// （与配置导出一致，见 `web_allocate_export_path`）。
fn export_session_markdown(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            default_name: String,
            content: String,
        }
        let Args {
            default_name,
            content,
        } = parse(args)?;

        let file_name = std::path::Path::new(&default_name)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "session.md".to_string());

        let dir = crate::config::get_app_config_dir().join("exports");
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建导出目录失败: {e}"))?;
        let path = dir.join(file_name);
        crate::config::write_text_file(&path, &content).map_err(|e| e.to_string())?;
        Ok(Value::String(path.to_string_lossy().into_owned()))
    })
}

/// 在文件管理器中显示路径：服务端没有桌面会话。
fn reveal_session_path(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        Err("Web 模式下没有文件管理器，请在服务器上直接查看该路径".to_string())
    })
}

/// 在终端里恢复会话：服务端没有桌面终端。
fn launch_session_terminal(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        Err("Web 模式下不能打开终端；请在服务器上用 CLI 自行恢复会话".to_string())
    })
}
