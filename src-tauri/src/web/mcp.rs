//! MCP 管理命令（对应桌面版 `commands/mcp.rs`）。
//!
//! 命令体在桌面版里就是「解析参数 → 调 `McpService` / `claude_mcp`」，
//! 这里照搬同一层封装；业务逻辑仍然只有 `services/mcp.rs` 一份。
//! 唯一一处有真实逻辑的 `upsert_mcp_server_in_config`（旧接口转统一结构）
//! 已下沉到 `McpService::upsert_from_legacy`，与桌面版共用。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use super::routes::{deferred, parse, serializable, to_app_type, Handler, HandlerFuture};
use super::Context;
use crate::services::McpService;

/// 命令名 → 处理函数。`routes.rs` 把它整个并进总表。
pub const HANDLERS: &[(&str, Handler)] = &[
    ("get_claude_mcp_status", get_claude_mcp_status),
    ("read_claude_mcp_config", read_claude_mcp_config),
    ("upsert_claude_mcp_server", upsert_claude_mcp_server),
    ("delete_claude_mcp_server", delete_claude_mcp_server),
    ("validate_mcp_command", validate_mcp_command),
    ("get_mcp_config", get_mcp_config),
    (
        "upsert_mcp_server_in_config",
        upsert_mcp_server_in_config,
    ),
    ("delete_mcp_server_in_config", delete_mcp_server_in_config),
    ("set_mcp_enabled", set_mcp_enabled),
    ("get_mcp_servers", get_mcp_servers),
    ("upsert_mcp_server", upsert_mcp_server),
    ("delete_mcp_server", delete_mcp_server),
    ("toggle_mcp_app", toggle_mcp_app),
    ("import_mcp_from_apps", import_mcp_from_apps),
    ("resync_mcp_to_apps", resync_mcp_to_apps),
];

// ============================================================================
// 兼容旧接口：直接读写 ~/.claude.json 的 mcpServers
// ============================================================================

fn get_claude_mcp_status(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    deferred(|| crate::claude_mcp::get_mcp_status().map_err(|error| error.to_string()))
}

fn read_claude_mcp_config(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    deferred(|| crate::claude_mcp::read_mcp_json().map_err(|error| error.to_string()))
}

fn upsert_claude_mcp_server(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            id: String,
            spec: Value,
        }
        let Args { id, spec } = parse(args)?;
        deferred(move || {
            crate::claude_mcp::upsert_mcp_server(&id, spec).map_err(|error| error.to_string())
        })
        .await
    })
}

fn delete_claude_mcp_server(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            id: String,
        }
        let Args { id } = parse(args)?;
        deferred(move || {
            crate::claude_mcp::delete_mcp_server(&id).map_err(|error| error.to_string())
        })
        .await
    })
}

/// 只查 PATH，不执行命令——服务端跑在无桌面环境里也成立。
fn validate_mcp_command(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            cmd: String,
        }
        let Args { cmd } = parse(args)?;
        deferred(move || {
            crate::claude_mcp::validate_command_in_path(&cmd).map_err(|error| error.to_string())
        })
        .await
    })
}

// ============================================================================
// 统一结构（v3.7.0+）
// ============================================================================

#[derive(serde::Serialize)]
struct McpConfigResponse {
    config_path: String,
    servers: HashMap<String, Value>,
}

#[allow(deprecated)] // 兼容层命令，内部调用已废弃的 Service 方法
fn get_mcp_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        let config_path = crate::config::get_app_config_path()
            .to_string_lossy()
            .to_string();
        let servers = McpService::get_servers(&state, app_type).map_err(|e| e.to_string())?;
        serializable(McpConfigResponse {
            config_path,
            servers,
        })
    })
}

fn upsert_mcp_server_in_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app: String,
            id: String,
            spec: Value,
            #[serde(default)]
            sync_other_side: Option<bool>,
        }
        let Args {
            app,
            id,
            spec,
            sync_other_side,
        } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            McpService::upsert_from_legacy(
                &state,
                &app_type,
                &id,
                spec,
                sync_other_side.unwrap_or(false),
            )
            .map(|_| true)
            .map_err(|error| error.to_string())
        })
        .await
    })
}

/// `app` 参数在统一结构里已无意义，保留只为兼容前端调用。
fn delete_mcp_server_in_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            #[allow(dead_code)]
            #[serde(default)]
            app: Option<String>,
            id: String,
        }
        let Args { id, .. } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            McpService::delete_server(&state, &id).map_err(|error| error.to_string())
        })
        .await
    })
}

#[allow(deprecated)] // 兼容层命令，内部调用已废弃的 Service 方法
fn set_mcp_enabled(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
            enabled: bool,
        }
        let Args { app, id, enabled } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            McpService::set_enabled(&state, app_type, &id, enabled)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_mcp_servers(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            McpService::get_all_servers(&state).map_err(|error| error.to_string())
        })
        .await
    })
}

fn upsert_mcp_server(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            server: crate::app_config::McpServer,
        }
        let Args { server } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            McpService::upsert_server(&state, server)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn delete_mcp_server(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            id: String,
        }
        let Args { id } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            McpService::delete_server(&state, &id).map_err(|error| error.to_string())
        })
        .await
    })
}

fn toggle_mcp_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            server_id: String,
            app: String,
            enabled: bool,
        }
        let Args {
            server_id,
            app,
            enabled,
        } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            McpService::toggle_app(&state, &server_id, app_type, enabled)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn import_mcp_from_apps(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            McpService::import_from_all_apps(&state).map_err(|error| error.to_string())
        })
        .await
    })
}

/// 按数据库里的开关把 MCP 重新写进各应用的 live 配置，逐应用返回结果。
///
/// 每个应用先拿它的切换锁再写，和切换供应商互斥；单个应用失败不影响其余应用。
fn resync_mcp_to_apps(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            #[serde(default)]
            apps: Option<Vec<String>>,
        }
        let Args { apps } = parse(args)?;
        let state = context.require_state()?;

        let targets = McpService::resync_targets(apps.as_deref()).map_err(|e| e.to_string())?;
        let mut outcomes = Vec::with_capacity(targets.len());
        for app in targets {
            let _guard = state.proxy_service.lock_switch_for_app(app.as_str()).await;
            outcomes.push(McpService::resync_app(&state, &app));
        }
        serializable(outcomes)
    })
}
