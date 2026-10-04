//! `/api/invoke` 命令分发表。
//!
//! 桌面版的 302 个 `#[tauri::command]` 定义在 `commands/` 里，而那个模块只在
//! `desktop` feature 下编译。服务端因此**不复用命令函数**，而是把命令体里那层
//! 薄封装（解析参数 → 调 service 层）在这里重写一遍——业务逻辑仍然只有一份，
//! 在 `services/` 里。
//!
//! PoC 阶段只实现「启动 + 供应商页」所需的命令。未实现的命令返回
//! `E_NOT_IMPLEMENTED:<cmd>`，前端会以普通错误提示，不会白屏。

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
type Handler = fn(Arc<Context>, Value) -> HandlerFuture;

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

        // ---- 代理状态（服务端不跑本地代理，但界面要能渲染）----
        table.insert("get_proxy_status", get_proxy_status as Handler);
        table.insert(
            "get_proxy_takeover_status",
            get_proxy_takeover_status as Handler,
        );

        table
    })
}

// ============================================================================
// 工具函数
// ============================================================================

fn serializable<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn parse<T: DeserializeOwned>(raw: Value) -> Result<T, String> {
    serde_json::from_value(raw).map_err(|error| format!("参数解析失败: {error}"))
}

/// 在阻塞线程池里跑同步的 service 调用（SQLite、文件读写都在其中）。
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    crate::host::spawn_blocking(work)
        .await
        .map_err(|error| format!("后台任务异常退出: {error}"))?
}

fn to_app_type(app: &str) -> Result<AppType, String> {
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
