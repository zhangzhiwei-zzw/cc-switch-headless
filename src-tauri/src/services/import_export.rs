//! 配置导入导出与数据库备份的业务逻辑。
//!
//! 桌面命令层（`commands/import_export.rs`）与服务端（`web/routes.rs`）共用：
//! 命令层只多一层文件对话框 / 上传下载的适配，流程本身（恢复前的互斥锁、
//! skills 表写保护、导入后的 live 重建）都在这里，避免两份实现漂移。

use serde_json::Value;

use crate::database::backup::BackupEntry;
use crate::database::Database;
use crate::error::AppError;
use crate::services::skill::skill_state_write_guard;
use crate::services::sync_protocol::sync_mutex;
use crate::services::sync_support::{
    post_sync_warning_from_result, run_post_import_sync, success_payload_with_warning,
};
use crate::store::AppState;

/// 数据库恢复期间必须与云同步互斥：先拿到全局同步锁，再开始替换数据库文件。
pub async fn run_with_database_restore_lock<T, Start, Fut>(start_operation: Start) -> T
where
    Start: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let _sync_guard = sync_mutex().lock().await;
    start_operation().await
}

/// 导出数据库为 SQL 备份文件。
pub async fn export_to_file(state: &AppState, file_path: &str) -> Result<Value, String> {
    let db = state.db.clone();
    let file_path = file_path.to_string();
    crate::host::spawn_blocking(move || {
        let target_path = std::path::PathBuf::from(&file_path);
        db.export_sql(&target_path).map_err(|e| e.to_string())?;
        Ok::<_, String>(serde_json::json!({
            "success": true,
            "message": "SQL exported successfully",
            "filePath": file_path
        }))
    })
    .await
    .map_err(|e| format!("导出配置失败: {e}"))?
}

/// 从 SQL 文件导入（恢复）配置。
pub async fn import_from_file(state: &AppState, file_path: &str) -> Result<Value, String> {
    let app_state = state.clone();
    let db = app_state.db.clone();
    let file_path = file_path.to_string();

    run_with_database_restore_lock(move || {
        crate::host::spawn_blocking(move || {
            let path_buf = std::path::PathBuf::from(&file_path);
            let backup_id = {
                // SQL 恢复会整体替换 `skills` 表：换库期间禁止本地 skills 写操作。
                let _skill_state_guard = skill_state_write_guard();
                db.import_sql(&path_buf)?
            };
            let warning = post_sync_warning_from_result(Ok(run_post_import_sync(&app_state)));
            if let Some(message) = warning.as_ref() {
                log::warn!("[Import] post-import sync warning: {message}");
            }
            Ok::<_, AppError>(success_payload_with_warning(backup_id, warning))
        })
    })
    .await
    .map_err(|e| format!("导入配置失败: {e}"))?
    .map_err(|e: AppError| e.to_string())
}

// ============================================================================
// 数据库备份文件
// ============================================================================

/// 立即创建一份数据库备份，返回备份文件名。
pub async fn create_backup(state: &AppState) -> Result<String, String> {
    let db = state.db.clone();
    crate::host::spawn_blocking(move || match db.backup_database_file()? {
        Some(path) => Ok(path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()),
        None => Err(AppError::Config(
            "Database file not found, backup skipped".to_string(),
        )),
    })
    .await
    .map_err(|e| format!("Backup failed: {e}"))?
    .map_err(|e: AppError| e.to_string())
}

/// 从备份文件恢复数据库，返回备份里记录的 id。
pub async fn restore_backup(state: &AppState, filename: &str) -> Result<String, String> {
    let app_state = state.clone();
    let db = app_state.db.clone();
    let filename = filename.to_string();

    run_with_database_restore_lock(move || {
        crate::host::spawn_blocking(move || {
            let restored = {
                let _skill_state_guard = skill_state_write_guard();
                db.restore_from_backup(&filename)?
            };
            let warning = post_sync_warning_from_result(Ok(run_post_import_sync(&app_state)));
            if let Some(message) = warning {
                // 这个命令只返回恢复的文件名，投影不完整只记日志
                log::warn!("[Restore] post-import sync warning: {message}");
            }
            Ok::<_, AppError>(restored)
        })
    })
    .await
    .map_err(|e| format!("Restore failed: {e}"))?
    .map_err(|e: AppError| e.to_string())
}

pub fn list_backups() -> Result<Vec<BackupEntry>, String> {
    Database::list_backups().map_err(|e| e.to_string())
}

pub fn rename_backup(old_filename: &str, new_name: &str) -> Result<String, String> {
    Database::rename_backup(old_filename, new_name).map_err(|e| e.to_string())
}

pub fn delete_backup(filename: &str) -> Result<(), String> {
    Database::delete_backup(filename).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::run_with_database_restore_lock;
    use crate::services::sync_protocol::sync_mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn manual_restore_starts_blocking_work_after_global_lock_acquisition() {
        let guard = sync_mutex().lock().await;
        let entered = Arc::new(AtomicBool::new(false));
        let entered_in_task = Arc::clone(&entered);
        let restore = run_with_database_restore_lock(move || {
            tokio::task::spawn_blocking(move || {
                entered_in_task.store(true, Ordering::SeqCst);
            })
        });
        tokio::pin!(restore);

        assert!(
            tokio::time::timeout(Duration::from_millis(40), restore.as_mut())
                .await
                .is_err(),
            "restore must wait while another sync operation holds the global lock"
        );
        assert!(!entered.load(Ordering::SeqCst));

        drop(guard);
        tokio::time::timeout(Duration::from_secs(1), restore.as_mut())
            .await
            .expect("restore should start after lock release")
            .expect("blocking restore task should complete");
        assert!(entered.load(Ordering::SeqCst));
    }
}
