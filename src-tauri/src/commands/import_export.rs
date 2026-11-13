#![allow(non_snake_case)]

use serde_json::{json, Value};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::database::backup::BackupEntry;
use crate::error::AppError;
use crate::services::provider::ProviderService;
use crate::store::AppState;

// ─── File import/export ──────────────────────────────────────

/// 导出数据库为 SQL 备份
#[tauri::command]
pub async fn export_config_to_file(
    #[allow(non_snake_case)] filePath: String,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    crate::services::import_export::export_to_file(state.inner(), &filePath).await
}

/// 从 SQL 备份导入数据库
#[tauri::command]
pub async fn import_config_from_file(
    #[allow(non_snake_case)] filePath: String,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    crate::services::import_export::import_from_file(state.inner(), &filePath).await
}

#[tauri::command]
pub async fn sync_current_providers_live(state: State<'_, AppState>) -> Result<Value, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let app_state = AppState::new(db);
        ProviderService::sync_current_to_live(&app_state)?;
        Ok::<_, AppError>(json!({
            "success": true,
            "message": "Live configuration synchronized"
        }))
    })
    .await
    .map_err(|e| format!("同步当前供应商失败: {e}"))?
    .map_err(|e: AppError| e.to_string())
}

// ─── File dialogs ────────────────────────────────────────────

#[tauri::command]
pub async fn save_file_dialog<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    #[allow(non_snake_case)] defaultName: String,
) -> Result<Option<String>, String> {
    let dialog = app.dialog();
    let result = dialog
        .file()
        .add_filter("SQL", &["sql"])
        .set_file_name(&defaultName)
        .blocking_save_file();

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn open_file_dialog<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    let dialog = app.dialog();
    let result = dialog
        .file()
        .add_filter("SQL", &["sql"])
        .blocking_pick_file();

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn open_zip_file_dialog<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    let dialog = app.dialog();
    let result = dialog
        .file()
        .add_filter("ZIP / Skill", &["zip", "skill"])
        .blocking_pick_file();

    Ok(result.map(|p| p.to_string()))
}

// ─── Database backup management ─────────────────────────────

/// Manually create a database backup
#[tauri::command]
pub async fn create_db_backup(state: State<'_, AppState>) -> Result<String, String> {
    crate::services::import_export::create_backup(state.inner()).await
}

/// List all database backup files
#[tauri::command]
pub async fn list_db_backups() -> Result<Vec<BackupEntry>, String> {
    tauri::async_runtime::spawn_blocking(crate::services::import_export::list_backups)
        .await
        .map_err(|e| e.to_string())?
}

/// Restore database from a backup file
#[tauri::command]
pub async fn restore_db_backup(
    state: State<'_, AppState>,
    filename: String,
) -> Result<String, String> {
    crate::services::import_export::restore_backup(state.inner(), &filename).await
}

/// Rename a database backup file
#[tauri::command]
pub async fn rename_db_backup(
    #[allow(non_snake_case)] oldFilename: String,
    #[allow(non_snake_case)] newName: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::services::import_export::rename_backup(&oldFilename, &newName)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a database backup file
#[tauri::command]
pub async fn delete_db_backup(filename: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::services::import_export::delete_backup(&filename)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ─── Backup storage overview ─────────────────────────────────

/// 列出本机各类备份的位置与大小
#[tauri::command]
pub async fn list_backup_locations(
) -> Result<Vec<crate::services::backup_storage::BackupLocation>, String> {
    tauri::async_runtime::spawn_blocking(crate::services::backup_storage::list_locations)
        .await
        .map_err(|e| e.to_string())
}

/// 删除一类备份（只认类别 id），返回释放的字节数
#[tauri::command]
pub async fn delete_backup_location(id: String) -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::services::backup_storage::delete_location(&id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 在文件管理器中显示一类备份所在的目录
#[tauri::command]
pub async fn reveal_backup_location(id: String) -> Result<bool, String> {
    let dir = crate::services::backup_storage::location_dir(&id).map_err(|e| e.to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(&dir).map_err(|e| format!("打开文件夹失败: {e}"))?;
    Ok(true)
}
