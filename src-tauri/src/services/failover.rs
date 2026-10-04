//! 故障转移队列的业务逻辑。
//!
//! 桌面命令层（`commands/failover.rs`）与服务端（`web/routes.rs`）共用这里的实现：
//! 命令层只多做一件事——刷新托盘菜单；其余（校验、入队/出队、开启时把 P1 切上去、
//! 失败回滚）都在这里，避免两份实现漂移。

use std::str::FromStr;

use crate::app_config::AppType;
use crate::database::FailoverQueueItem;
use crate::provider::Provider;
use crate::store::AppState;

/// 只有支持本地路由的应用才有故障转移。
pub fn require_failover_app(app_type: &str) -> Result<(), String> {
    let app = AppType::from_str(app_type).map_err(|error| format!("无效的应用类型: {error}"))?;
    if !app.supports_local_proxy() {
        return Err(format!("{} 不支持故障转移", app.as_str()));
    }
    Ok(())
}

/// 校验供应商存在且支持故障转移（Codex 官方账号卡不支持）。
pub fn require_failover_provider(
    db: &crate::database::Database,
    app_type: &str,
    provider_id: &str,
) -> Result<Provider, String> {
    let provider = db
        .get_provider_by_id(provider_id, app_type)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("供应商不存在: {provider_id}"))?;
    if !crate::proxy::provider_router::provider_supports_failover(app_type, &provider) {
        return Err("Codex Official 账号卡不支持自动故障转移".to_string());
    }
    Ok(provider)
}

// ============================================================================
// 队列
// ============================================================================

pub async fn get_queue(state: &AppState, app_type: &str) -> Result<Vec<FailoverQueueItem>, String> {
    require_failover_app(app_type)?;
    let queue = state
        .db
        .get_failover_queue(app_type)
        .map_err(|e| e.to_string())?;
    if app_type != "codex" {
        return Ok(queue);
    }

    // Codex 官方账号卡留在队列里没有意义，读取时过滤掉
    let providers = state
        .db
        .get_all_providers(app_type)
        .map_err(|e| e.to_string())?;
    Ok(filter_supported(app_type, queue, &providers))
}

pub async fn get_available_providers(
    state: &AppState,
    app_type: &str,
) -> Result<Vec<Provider>, String> {
    require_failover_app(app_type)?;
    let providers = state
        .db
        .get_available_providers_for_failover(app_type)
        .map_err(|e| e.to_string())?;
    Ok(providers
        .into_iter()
        .filter(|provider| {
            crate::proxy::provider_router::provider_supports_failover(app_type, provider)
        })
        .collect())
}

pub async fn add(state: &AppState, app_type: &str, provider_id: &str) -> Result<(), String> {
    require_failover_app(app_type)?;
    require_failover_provider(&state.db, app_type, provider_id)?;
    state
        .db
        .add_to_failover_queue(app_type, provider_id)
        .map_err(|e| e.to_string())
}

pub async fn remove(state: &AppState, app_type: &str, provider_id: &str) -> Result<(), String> {
    require_failover_app(app_type)?;
    state
        .db
        .remove_from_failover_queue(app_type, provider_id)
        .map_err(|e| e.to_string())
}

pub async fn auto_failover_enabled(state: &AppState, app_type: &str) -> Result<bool, String> {
    require_failover_app(app_type)?;
    state
        .db
        .get_proxy_config_for_app(app_type)
        .await
        .map(|config| config.auto_failover_enabled)
        .map_err(|e| e.to_string())
}

/// 设置自动故障转移开关。
///
/// 开启时：队列为空会把当前供应商自动加为 P1（避免"必须先加队列才能开启"的死锁），
/// 然后**先切到 P1 再写开关**——P1 切不过去（例如官方卡）时不留"开关已开但没切"的脏状态，
/// 自动加入的 P1 也会回滚。
///
/// 返回开启时的 P1 供应商 ID（调用方据此发 `provider-switched` 事件）；关闭时返回 `None`。
pub async fn set_auto_failover_enabled(
    state: &AppState,
    app_type: &str,
    enabled: bool,
) -> Result<Option<String>, String> {
    require_failover_app(app_type)?;
    let app_enum =
        AppType::from_str(app_type).map_err(|_| format!("无效的应用类型: {app_type}"))?;
    log::info!(
        "[Failover] Setting auto_failover_enabled: app_type='{app_type}', enabled={enabled}"
    );

    let mut config = state
        .db
        .get_proxy_config_for_app(app_type)
        .await
        .map_err(|e| e.to_string())?;

    if enabled && !crate::mode::current::is_proxy(&app_enum) {
        return Err("需要先让该应用进入路由模式，再开启故障转移".to_string());
    }

    let mut auto_added_provider_id: Option<String> = None;
    let p1_provider_id = if enabled {
        let all_providers = state
            .db
            .get_all_providers(app_type)
            .map_err(|e| e.to_string())?;
        let mut queue = filter_supported(
            app_type,
            state
                .db
                .get_failover_queue(app_type)
                .map_err(|e| e.to_string())?,
            &all_providers,
        );

        if queue.is_empty() {
            let current_id = crate::mode::current::provider_for(
                &state.db,
                &app_enum,
                crate::mode::current::Purpose::InUse,
            )
            .map_err(|e| e.to_string())?;

            let Some(current_id) = current_id else {
                return Err("故障转移队列为空，且未设置当前供应商，无法开启故障转移".to_string());
            };

            require_failover_provider(&state.db, app_type, &current_id)?;

            state
                .db
                .add_to_failover_queue(app_type, &current_id)
                .map_err(|e| e.to_string())?;
            auto_added_provider_id = Some(current_id);

            queue = filter_supported(
                app_type,
                state
                    .db
                    .get_failover_queue(app_type)
                    .map_err(|e| e.to_string())?,
                &all_providers,
            );
        }

        queue
            .first()
            .map(|item| item.provider_id.clone())
            .ok_or_else(|| "故障转移队列为空，无法开启故障转移".to_string())?
    } else {
        String::new()
    };

    if enabled {
        // Stack 模式不做故障转移，切换会在锁内被拒绝（见 `switch_route_for_failover`）。
        if let Err(error) =
            crate::mode::controller::switch_route_for_failover(state, &app_enum, &p1_provider_id)
                .await
        {
            if let Some(provider_id) = auto_added_provider_id {
                let _ = state.db.remove_from_failover_queue(app_type, &provider_id);
            }
            return Err(error);
        }
    }

    // `enabled` 是模式的镜像，保持与模式一致
    config.auto_failover_enabled = enabled;
    config.enabled = crate::mode::current::is_proxy(&app_enum);

    state
        .db
        .update_proxy_config_for_app(config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(enabled.then_some(p1_provider_id))
}

fn filter_supported(
    app_type: &str,
    queue: Vec<FailoverQueueItem>,
    providers: &indexmap::IndexMap<String, Provider>,
) -> Vec<FailoverQueueItem> {
    queue
        .into_iter()
        .filter(|item| {
            providers.get(&item.provider_id).is_some_and(|provider| {
                crate::proxy::provider_router::provider_supports_failover(app_type, provider)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{require_failover_app, require_failover_provider};
    use crate::database::Database;
    use crate::provider::{AuthBinding, AuthBindingSource, Provider, ProviderMeta};
    use serde_json::json;

    #[test]
    fn failover_rejects_apps_without_a_proxy_data_plane() {
        assert!(require_failover_app("claude").is_ok());
        assert!(require_failover_app("pi").is_err());
    }

    #[test]
    fn failover_rejects_codex_official_account_cards() {
        let db = Database::memory().expect("memory db");
        let mut official = Provider::with_id(
            "official-a".to_string(),
            "OpenAI Official".to_string(),
            json!({ "auth": {}, "config": "" }),
            None,
        );
        official.category = Some("official".to_string());
        official.meta = Some(ProviderMeta {
            auth_binding: Some(AuthBinding {
                source: AuthBindingSource::ManagedAccount,
                auth_provider: Some("codex_oauth".to_string()),
                account_id: Some("account-a".to_string()),
            }),
            ..Default::default()
        });
        db.save_provider("codex", &official).expect("save official");

        assert!(require_failover_provider(&db, "codex", &official.id).is_err());
    }
}
