//! 故障转移队列命令
//!
//! 管理代理模式下的故障转移队列（基于 providers 表的 in_failover_queue 字段）。
//! 业务逻辑在 `services::failover`（服务端 web 模式共用同一份实现），
//! 这里只负责参数拆包、事件与托盘刷新。

use crate::database::FailoverQueueItem;
use crate::provider::Provider;
use crate::store::AppState;
use tauri::{AppHandle, State};

/// 获取故障转移队列
#[tauri::command]
pub async fn get_failover_queue(
    state: State<'_, AppState>,
    app_type: String,
) -> Result<Vec<FailoverQueueItem>, String> {
    crate::services::failover::get_queue(state.inner(), &app_type).await
}

/// 获取可添加到故障转移队列的供应商（不在队列中的）
#[tauri::command]
pub async fn get_available_providers_for_failover(
    state: State<'_, AppState>,
    app_type: String,
) -> Result<Vec<Provider>, String> {
    crate::services::failover::get_available_providers(state.inner(), &app_type).await
}

/// 添加供应商到故障转移队列。托盘的故障转移子菜单只列队列成员：队列一变就重建。
#[tauri::command]
pub async fn add_to_failover_queue(
    app: AppHandle,
    state: State<'_, AppState>,
    app_type: String,
    provider_id: String,
) -> Result<(), String> {
    crate::services::failover::add(state.inner(), &app_type, &provider_id).await?;
    crate::tray::refresh_tray_menu(&app);
    Ok(())
}

/// 从故障转移队列移除供应商
#[tauri::command]
pub async fn remove_from_failover_queue(
    app: AppHandle,
    state: State<'_, AppState>,
    app_type: String,
    provider_id: String,
) -> Result<(), String> {
    crate::services::failover::remove(state.inner(), &app_type, &provider_id).await?;
    crate::tray::refresh_tray_menu(&app);
    Ok(())
}

/// 获取指定应用的自动故障转移开关状态
#[tauri::command]
pub async fn get_auto_failover_enabled(
    state: State<'_, AppState>,
    app_type: String,
) -> Result<bool, String> {
    crate::services::failover::auto_failover_enabled(state.inner(), &app_type).await
}

/// 设置指定应用的自动故障转移开关状态（写入 proxy_config 表）
///
/// 注意：关闭故障转移时不会清除队列，队列内容会保留供下次开启时使用
#[tauri::command]
pub async fn set_auto_failover_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    app_type: String,
    enabled: bool,
) -> Result<(), String> {
    let p1_provider_id =
        crate::services::failover::set_auto_failover_enabled(state.inner(), &app_type, enabled)
            .await?;

    if let Some(provider_id) = p1_provider_id {
        // 让前端刷新当前供应商（与托盘/服务端同名事件）
        crate::event_sink::emit(
            "provider-switched",
            serde_json::json!({
                "appType": app_type,
                "providerId": provider_id,
                "source": "failoverEnabled"
            }),
        );
    }

    crate::tray::refresh_tray_menu(&app);
    Ok(())
}
