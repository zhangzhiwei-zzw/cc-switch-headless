//! 故障转移切换模块
//!
//! 处理故障转移成功后的供应商切换逻辑，包括：
//! - 去重控制（避免多个请求同时触发）
//! - 托盘菜单更新（仅桌面版）
//! - 前端事件发射（经 `event_sink`，两种构建一致）

use crate::error::AppError;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 故障转移切换管理器
///
/// 负责处理故障转移成功后的供应商切换，确保 UI 能够直观反映当前使用的供应商。
#[derive(Clone, Default)]
pub struct FailoverSwitchManager {
    /// 正在处理中的切换（key = "app_type:provider_id"）
    pending_switches: Arc<RwLock<HashSet<String>>>,
}

impl FailoverSwitchManager {
    pub fn new() -> Self {
        Self {
            pending_switches: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 尝试执行故障转移切换
    ///
    /// 如果相同的切换已在进行中，则跳过；否则执行切换逻辑。
    ///
    /// # Returns
    /// - `Ok(true)` - 切换成功执行
    /// - `Ok(false)` - 切换已在进行中，或当前不具备切换条件
    /// - `Err(e)` - 切换过程中发生错误
    pub async fn try_switch(
        &self,
        app_type: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<bool, AppError> {
        let switch_key = format!("{app_type}:{provider_id}");

        // 去重检查：如果相同切换已在进行中，跳过
        {
            let mut pending = self.pending_switches.write().await;
            if pending.contains(&switch_key) {
                log::debug!("[Failover] 切换已在进行中，跳过: {app_type} -> {provider_id}");
                return Ok(false);
            }
            pending.insert(switch_key.clone());
        }

        // 执行切换（确保最后清理 pending 标记）
        let result = self.do_switch(app_type, provider_id, provider_name).await;

        // 清理 pending 标记
        {
            let mut pending = self.pending_switches.write().await;
            pending.remove(&switch_key);
        }

        result
    }

    async fn do_switch(
        &self,
        app_type: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<bool, AppError> {
        // 只有处于代理模式的应用才允许执行故障转移切换
        let Ok(app_enum) = app_type.parse::<crate::app_config::AppType>() else {
            return Ok(false);
        };
        if !crate::mode::current::is_proxy(&app_enum) {
            log::debug!("[Failover] {app_type} 不在路由模式，跳过切换");
            return Ok(false);
        }

        // AppState 由启动流程注册（桌面版 setup / 服务端 bootstrap）。
        let Some(app_state) = crate::store::current() else {
            log::warn!("[Failover] AppState 尚未注册，跳过切换");
            return Ok(false);
        };

        log::info!("[FO-001] 切换: {app_type} → {provider_name}");

        // 只换代理的路由，不写客户端文件。
        let switched =
            crate::mode::controller::record_failover_route(&app_state, &app_enum, provider_id)
                .await
                .map_err(AppError::Message)?;

        if !switched {
            return Ok(false);
        }

        // 托盘菜单只存在于桌面版；服务端没有可刷新的托盘。
        #[cfg(feature = "desktop")]
        if let Some(handle) = crate::host::handle() {
            if let Ok(new_menu) = crate::tray::create_tray_menu(&handle, &app_state) {
                if let Some(tray) = handle.tray_by_id(crate::tray::TRAY_ID) {
                    if let Err(error) = tray.set_menu(Some(new_menu)) {
                        log::error!("[Failover] 更新托盘菜单失败: {error}");
                    }
                }
            }
        }

        // 发射事件到前端（两种构建同名同 payload）
        crate::event_sink::emit(
            "provider-switched",
            serde_json::json!({
                "appType": app_type,
                "providerId": provider_id,
                "source": "failover"  // 标识来源是故障转移
            }),
        );

        Ok(true)
    }
}
