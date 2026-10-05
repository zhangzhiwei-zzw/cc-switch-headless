//! 用量统计命令（对应桌面版 `commands/usage.rs`）。
//!
//! 聚合查询都落在 SQLite 上，一律丢进阻塞线程池；`queryProviderUsage` 会真的
//! 发网络请求去查供应商余额/额度，是唯一有外部依赖的一条。
//!
//! 托管账号（GitHub Copilot、xAI OAuth）的额度查询需要桌面版的 OAuth 管理器，
//! 服务端只支持余额 / Coding Plan / 官方订阅额度 / 通用 JS 脚本四条路径。

use std::sync::Arc;

use serde_json::Value;

use super::routes::{deferred, parse, serializable, to_app_type, Handler, HandlerFuture};
use super::Context;
use crate::services::usage_stats::LogFilters;

pub const HANDLERS: &[(&str, Handler)] = &[
    ("get_usage_summary", get_usage_summary),
    ("get_session_usage_summary", get_session_usage_summary),
    ("get_usage_summary_by_app", get_usage_summary_by_app),
    ("get_usage_trends", get_usage_trends),
    ("get_provider_stats", get_provider_stats),
    ("get_model_stats", get_model_stats),
    ("get_request_logs", get_request_logs),
    ("get_request_detail", get_request_detail),
    ("get_model_pricing", get_model_pricing),
    ("update_model_pricing", update_model_pricing),
    ("update_model_pricing_batch", update_model_pricing_batch),
    ("delete_model_pricing", delete_model_pricing),
    ("get_models_dev_sync_config", get_models_dev_sync_config),
    ("save_models_dev_sync_config", save_models_dev_sync_config),
    (
        "record_models_dev_sync_result",
        record_models_dev_sync_result,
    ),
    ("check_provider_limits", check_provider_limits),
    ("sync_session_usage", sync_session_usage),
    ("get_session_usage_last_sync", get_session_usage_last_sync),
    ("rebuild_codex_usage", rebuild_codex_usage),
    ("get_usage_data_sources", get_usage_data_sources),
    ("queryProviderUsage", query_provider_usage),
    ("testUsageScript", test_usage_script),
    ("get_pricing_model_source", get_pricing_model_source),
    ("set_pricing_model_source", set_pricing_model_source),
];

/// 用量页那几个查询共用的过滤条件。
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageQuery {
    #[serde(default)]
    start_date: Option<i64>,
    #[serde(default)]
    end_date: Option<i64>,
    #[serde(default)]
    app_type: Option<String>,
    #[serde(default)]
    provider_name: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

fn get_usage_summary(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        let query: UsageQuery = parse(args)?;
        let state = context.require_state()?;
        deferred(move || {
            state
                .db
                .get_usage_summary(
                    query.start_date,
                    query.end_date,
                    query.app_type.as_deref(),
                    query.provider_name.as_deref(),
                    query.model.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_session_usage_summary(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            session_id: String,
        }
        let Args {
            app_type,
            session_id,
        } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            state
                .db
                .get_session_usage_summary(&app_type, &session_id)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_usage_summary_by_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        let query: UsageQuery = parse(args)?;
        let state = context.require_state()?;
        deferred(move || {
            state
                .db
                .get_usage_summary_by_app(
                    query.start_date,
                    query.end_date,
                    query.provider_name.as_deref(),
                    query.model.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_usage_trends(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        let query: UsageQuery = parse(args)?;
        let state = context.require_state()?;
        deferred(move || {
            state
                .db
                .get_daily_trends(
                    query.start_date,
                    query.end_date,
                    query.app_type.as_deref(),
                    query.provider_name.as_deref(),
                    query.model.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_provider_stats(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        let query: UsageQuery = parse(args)?;
        let state = context.require_state()?;
        deferred(move || {
            state
                .db
                .get_provider_stats(
                    query.start_date,
                    query.end_date,
                    query.app_type.as_deref(),
                    query.provider_name.as_deref(),
                    query.model.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_model_stats(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        let query: UsageQuery = parse(args)?;
        let state = context.require_state()?;
        deferred(move || {
            state
                .db
                .get_model_stats(
                    query.start_date,
                    query.end_date,
                    query.app_type.as_deref(),
                    query.provider_name.as_deref(),
                    query.model.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_request_logs(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            #[serde(default)]
            filters: LogFilters,
            page: u32,
            page_size: u32,
        }
        let Args {
            filters,
            page,
            page_size,
        } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            state
                .db
                .get_request_logs(&filters, page, page_size)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_request_detail(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            request_id: String,
        }
        let Args { request_id } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            state
                .db
                .get_request_detail(&request_id)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

// ============================================================================
// 模型定价
// ============================================================================

fn get_model_pricing(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            crate::services::model_pricing::list_model_pricing(&state.db)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

#[allow(clippy::too_many_arguments)]
fn update_model_pricing(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        use crate::services::model_pricing::ModelPricingInfo;

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            model_id: String,
            display_name: String,
            input_cost: String,
            output_cost: String,
            cache_read_cost: String,
            cache_creation_cost: String,
        }
        let Args {
            model_id,
            display_name,
            input_cost,
            output_cost,
            cache_read_cost,
            cache_creation_cost,
        } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            crate::services::model_pricing::update_model_pricing(
                &state.db,
                ModelPricingInfo {
                    model_id,
                    display_name,
                    input_cost_per_million: input_cost,
                    output_cost_per_million: output_cost,
                    cache_read_cost_per_million: cache_read_cost,
                    cache_creation_cost_per_million: cache_creation_cost,
                },
            )
            .map(|_| Value::Null)
            .map_err(|error| error.to_string())
        })
        .await
    })
}

fn update_model_pricing_batch(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            entries: Vec<crate::services::model_pricing::ModelPricingInfo>,
        }
        let Args { entries } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            crate::services::model_pricing::update_model_pricing_batch(&state.db, entries)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn delete_model_pricing(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            model_id: String,
        }
        let Args { model_id } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            crate::services::model_pricing::delete_model_pricing(&state.db, &model_id)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_models_dev_sync_config(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            crate::services::model_pricing::get_models_dev_sync_state(&state.db)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn save_models_dev_sync_config(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            config: crate::services::model_pricing::ModelsDevSyncConfig,
        }
        let Args { config } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            crate::services::model_pricing::save_models_dev_sync_config(&state.db, config)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn record_models_dev_sync_result(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            #[serde(default)]
            synced_at: Option<i64>,
            #[serde(default)]
            error: Option<String>,
        }
        let Args { synced_at, error } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            crate::services::model_pricing::record_models_dev_sync_result(
                &state.db, synced_at, error,
            )
            .map(|_| Value::Null)
            .map_err(|error| error.to_string())
        })
        .await
    })
}

fn check_provider_limits(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            app_type: String,
        }
        let Args {
            provider_id,
            app_type,
        } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            state
                .db
                .check_provider_limits(&provider_id, &app_type)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

/// 计费模式来源（本地文件 / models.dev），只是数据库里的一行设置。
fn get_pricing_model_source(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
        }
        let Args { app_type } = parse(args)?;
        let state = context.require_state()?;

        state
            .db
            .get_pricing_model_source(&app_type)
            .await
            .map_err(|error| error.to_string())
            .and_then(serializable)
    })
}

fn set_pricing_model_source(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            app_type: String,
            value: String,
        }
        let Args { app_type, value } = parse(args)?;
        let state = context.require_state()?;

        state
            .db
            .set_pricing_model_source(&app_type, &value)
            .await
            .map(|_| Value::Null)
            .map_err(|error| error.to_string())
    })
}

// ============================================================================
// 会话日志同步
// ============================================================================

fn sync_session_usage(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let db = state.db.clone();
        let _guard = crate::services::session_usage::session_sync_mutex()
            .lock()
            .await;
        deferred(move || {
            Ok(crate::services::session_usage::sync_all_unlocked(&db))
        })
        .await
    })
}

fn get_session_usage_last_sync(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        serializable(crate::services::session_usage::last_sync_completed_at())
    })
}

/// 备份数据库后，仅重建 Codex session 用量。锁覆盖 backup → reset → import
/// 整个序列，避免后台同步在清理和重导之间插入数据。
fn rebuild_codex_usage(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let db = state.db.clone();
        let _guard = crate::services::session_usage::session_sync_mutex()
            .lock()
            .await;

        let result = deferred(move || {
            db.backup_database_file()
                .map_err(|error| error.to_string())?;
            db.reset_codex_usage().map_err(|error| error.to_string())?;
            let result = crate::services::session_usage_codex::sync_codex_usage(&db)
                .map_err(|error| error.to_string());
            // reset 成功后，无论重导是否导入新行或报错，都要通知前端刷新
            crate::usage_events::notify_log_recorded();
            result
        })
        .await;

        serializable(result?)
    })
}

fn get_usage_data_sources(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            crate::services::session_usage::get_data_source_breakdown(&state.db)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

// ============================================================================
// 供应商用量 / 脚本测试
// ============================================================================

/// 与桌面版同名命令共用 `services::provider::usage::query_provider_usage`。
///
/// 差别在托管账号：Copilot 与 xAI OAuth 供应商的额度要 OAuth 管理器，
/// 服务端返回明确错误，由用量脚注的失败态（带原因与重试）展示。
fn query_provider_usage(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            app: String,
        }
        let Args { provider_id, app } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        let providers = state
            .db
            .get_all_providers(app_type.as_str())
            .map_err(|error| format!("Failed to get providers: {error}"))?;
        let provider = providers.get(&provider_id).cloned();

        let usage_script = provider
            .as_ref()
            .and_then(|p| p.meta.as_ref())
            .and_then(|m| m.usage_script.as_ref());
        let template_type = usage_script
            .and_then(|s| s.template_type.as_deref())
            .unwrap_or("");

        // 两条托管账号路径：桌面版拿 OAuth 管理器查，服务端没有。
        if template_type == crate::services::provider::usage::TEMPLATE_TYPE_GITHUB_COPILOT {
            return Err(
                "GitHub Copilot 用量需要托管账号登录，web 模式暂不支持（请在桌面版查看）"
                    .to_string(),
            );
        }
        if template_type == crate::services::provider::usage::TEMPLATE_TYPE_OFFICIAL_SUBSCRIPTION
            && provider
                .as_ref()
                .map(crate::provider::Provider::is_xai_oauth)
                .unwrap_or(false)
        {
            return Err(
                "xAI OAuth 用量需要托管账号登录，web 模式暂不支持（请在桌面版查看）".to_string(),
            );
        }

        let snapshot = crate::services::provider::usage::query_provider_usage(
            &state,
            app_type.clone(),
            provider.as_ref(),
            &provider_id,
        )
        .await?;

        // 与桌面版一致：成功的快照进缓存并广播，别的前端组件据此刷新。
        crate::event_sink::emit(
            "usage-cache-updated",
            serde_json::json!({
                "kind": "script",
                "appType": app_type.as_str(),
                "providerId": &provider_id,
                "data": &snapshot,
            }),
        );
        state
            .usage_cache
            .put_script(app_type, provider_id, snapshot.clone());

        serializable(snapshot)
    })
}

fn test_usage_script(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            provider_id: String,
            app: String,
            script_code: String,
            #[serde(default)]
            timeout: Option<u64>,
            #[serde(default)]
            api_key: Option<String>,
            #[serde(default)]
            base_url: Option<String>,
            #[serde(default)]
            access_token: Option<String>,
            #[serde(default)]
            user_id: Option<String>,
            #[serde(default)]
            template_type: Option<String>,
        }
        let Args {
            provider_id,
            app,
            script_code,
            timeout,
            api_key,
            base_url,
            access_token,
            user_id,
            template_type,
        } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        crate::services::provider::usage::test_usage_script(
            &state,
            app_type,
            &provider_id,
            &script_code,
            timeout.unwrap_or(10),
            api_key.as_deref(),
            base_url.as_deref(),
            access_token.as_deref(),
            user_id.as_deref(),
            template_type.as_deref(),
        )
        .await
        .map_err(|error| error.to_string())
        .and_then(serializable)
    })
}
