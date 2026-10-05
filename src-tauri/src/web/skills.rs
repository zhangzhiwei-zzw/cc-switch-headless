//! Skills 管理命令（对应桌面版 `commands/skill.rs`）。
//!
//! 业务逻辑全在 `services/skill.rs`，这里只做参数解析与调用。两处与浏览器有关：
//!
//! - `install_skills_from_zip` 收的是**服务端路径**；前端 shim 已经把「选文件」
//!   换成浏览器 `<input type=file>` + `POST /api/upload`，所以这里拿到的是真的
//!   落地文件。
//! - `open_cc_switch_skills_dir` 要让服务端弹文件管理器，浏览器里没有对应物，
//!   由能力表 `openInFileManager` 让前端隐藏入口；真被调到就返回明确错误。

use std::sync::Arc;

use serde_json::Value;

use super::routes::{deferred, parse, serializable, to_app_type, Handler, HandlerFuture};
use super::Context;
use crate::services::skill::{
    SkillService, SkillStorageLocation, SkillUninstallResult,
};

pub const HANDLERS: &[(&str, Handler)] = &[
    ("get_installed_skills", get_installed_skills),
    ("get_skill_backups", get_skill_backups),
    ("delete_skill_backup", delete_skill_backup),
    ("install_skill_unified", install_skill_unified),
    ("uninstall_skill_unified", uninstall_skill_unified),
    ("restore_skill_backup", restore_skill_backup),
    ("toggle_skill_app", toggle_skill_app),
    ("scan_unmanaged_skills", scan_unmanaged_skills),
    ("import_skills_from_apps", import_skills_from_apps),
    ("discover_available_skills", discover_available_skills),
    ("check_skill_updates", check_skill_updates),
    ("resync_skills_to_apps", resync_skills_to_apps),
    ("update_skill", update_skill),
    ("migrate_skill_storage", migrate_skill_storage),
    ("get_cc_switch_skills_dir", get_cc_switch_skills_dir),
    ("open_cc_switch_skills_dir", open_cc_switch_skills_dir),
    ("search_skills_sh", search_skills_sh),
    // 兼容旧 API
    ("get_skills", get_skills),
    ("get_skills_for_app", get_skills_for_app),
    ("install_skill", install_skill),
    ("install_skill_for_app", install_skill_for_app),
    ("uninstall_skill", uninstall_skill),
    ("uninstall_skill_for_app", uninstall_skill_for_app),
    // 仓库管理
    ("get_skill_repos", get_skill_repos),
    ("add_skill_repo", add_skill_repo),
    ("remove_skill_repo", remove_skill_repo),
    ("install_skills_from_zip", install_skills_from_zip),
];

// ============================================================================
// 已安装的 Skills
// ============================================================================

fn get_installed_skills(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            SkillService::get_all_installed(&state.db).map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_skill_backups(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    deferred(|| SkillService::list_backups().map_err(|error| error.to_string()))
}

fn delete_skill_backup(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            backup_id: String,
        }
        let Args { backup_id } = parse(args)?;
        deferred(move || {
            SkillService::delete_backup(&backup_id)
                .map(|_| true)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn install_skill_unified(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            skill: crate::services::skill::DiscoverableSkill,
            current_app: String,
        }
        let Args { skill, current_app } = parse(args)?;
        let app_type = to_app_type(&current_app)?;
        let state = context.require_state()?;

        SkillService::new()
            .install(&state.db, &skill, &app_type)
            .await
            .map_err(|error| error.to_string())
            .and_then(serializable)
    })
}

fn uninstall_skill_unified(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            id: String,
        }
        let Args { id } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            SkillService::uninstall(&state.db, &id).map_err(|error| error.to_string())
        })
        .await
    })
}

fn restore_skill_backup(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            backup_id: String,
            current_app: String,
        }
        let Args {
            backup_id,
            current_app,
        } = parse(args)?;
        let app_type = to_app_type(&current_app)?;
        let state = context.require_state()?;

        deferred(move || {
            SkillService::restore_from_backup(&state.db, &backup_id, &app_type)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn toggle_skill_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            id: String,
            app: String,
            enabled: bool,
        }
        let Args { id, app, enabled } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            SkillService::toggle_app(&state.db, &id, &app_type, enabled)
                .map(|_| true)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn scan_unmanaged_skills(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            SkillService::scan_unmanaged(&state.db).map_err(|error| error.to_string())
        })
        .await
    })
}

fn import_skills_from_apps(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            imports: Vec<crate::services::skill::ImportSkillSelection>,
        }
        let Args { imports } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            SkillService::import_from_apps(&state.db, imports).map_err(|error| error.to_string())
        })
        .await
    })
}

// ============================================================================
// 发现 / 更新（要走网络，直接 await）
// ============================================================================

fn discover_available_skills(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let repos = state
            .db
            .get_skill_repos()
            .map_err(|error| error.to_string())?;
        SkillService::new()
            .discover_available_report(repos)
            .await
            .map_err(|error| error.to_string())
            .and_then(serializable)
    })
}

fn check_skill_updates(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        SkillService::new()
            .check_updates_report(&state.db)
            .await
            .map_err(|error| error.to_string())
            .and_then(serializable)
    })
}

fn update_skill(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            id: String,
        }
        let Args { id } = parse(args)?;
        let state = context.require_state()?;

        SkillService::new()
            .update_skill(&state.db, &id)
            .await
            .map_err(|error| error.to_string())
            .and_then(serializable)
    })
}

/// 按数据库里的开关把 Skill 重新投影到各应用目录，逐应用返回结果。
fn resync_skills_to_apps(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        let db = state.db.clone();
        crate::host::spawn_blocking(move || SkillService::resync_all_apps(&db))
            .await
            .map_err(|error| format!("重新同步 Skill 失败: {error}"))
            .and_then(serializable)
    })
}

fn migrate_skill_storage(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            target: SkillStorageLocation,
        }
        let Args { target } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            SkillService::migrate_storage(&state.db, target).map_err(|error| error.to_string())
        })
        .await
    })
}

// ============================================================================
// 存储位置
// ============================================================================

/// CC Switch 目录下放 Skill 主副本的位置（改过配置目录就是改后的）。
fn cc_switch_skills_dir() -> std::path::PathBuf {
    crate::config::get_app_config_dir().join("skills")
}

fn get_cc_switch_skills_dir(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move { serializable(cc_switch_skills_dir().to_string_lossy().to_string()) })
}

/// 服务端能建目录，但没法替**用户**弹出文件管理器——入口由能力表隐藏。
fn open_cc_switch_skills_dir(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let dir = cc_switch_skills_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|error| format!("创建目录失败: {error}"))?;
        }
        Err(format!(
            "服务端没有文件管理器，目录在 {}",
            dir.to_string_lossy()
        ))
    })
}

// ============================================================================
// 搜索 / 兼容旧 API / 仓库管理
// ============================================================================

fn search_skills_sh(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            query: String,
            limit: usize,
            offset: usize,
        }
        let Args {
            query,
            limit,
            offset,
        } = parse(args)?;
        SkillService::search_skills_sh(&query, limit, offset)
            .await
            .map_err(|error| error.to_string())
            .and_then(serializable)
    })
}

/// 列出「可发现 + 已安装」，两种构建共用同一份 `SkillService::list_skills`。
async fn list_skills(context: Arc<Context>) -> Result<Value, String> {
    let state = context.require_state()?;
    let repos = state
        .db
        .get_skill_repos()
        .map_err(|error| error.to_string())?;
    let skills = SkillService::new()
        .list_skills(repos, &state.db)
        .await
        .map_err(|error| error.to_string())?;
    serializable(skills)
}

fn get_skills(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(list_skills(context))
}

/// 新版本不再区分应用，`app` 只用于校验参数合法性。
fn get_skills_for_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        to_app_type(&app)?;
        list_skills(context).await
    })
}

fn install_skill(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            directory: String,
        }
        let Args { directory } = parse(args)?;
        install_for_app(context, "claude".to_string(), directory).await
    })
}

fn install_skill_for_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            directory: String,
        }
        let Args { app, directory } = parse(args)?;
        install_for_app(context, app, directory).await
    })
}

/// 旧接口：按目录名在仓库里找到 Skill 再安装。
async fn install_for_app(
    context: Arc<Context>,
    app: String,
    directory: String,
) -> Result<Value, String> {
    let app_type = to_app_type(&app)?;
    let state = context.require_state()?;

    let repos = state
        .db
        .get_skill_repos()
        .map_err(|error| error.to_string())?;
    let skills = SkillService::new()
        .discover_available(repos)
        .await
        .map_err(|error| error.to_string())?;

    let skill = skills
        .into_iter()
        .find(|skill| {
            let install_name = std::path::Path::new(&skill.directory)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| skill.directory.clone());
            install_name.eq_ignore_ascii_case(&directory)
                || skill.directory.eq_ignore_ascii_case(&directory)
        })
        .ok_or_else(|| {
            crate::error::format_skill_error(
                "SKILL_NOT_FOUND",
                &[("directory", &directory)],
                Some("checkRepoUrl"),
            )
        })?;

    SkillService::new()
        .install(&state.db, &skill, &app_type)
        .await
        .map_err(|error| error.to_string())?;

    Ok(Value::Bool(true))
}

fn uninstall_skill(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            directory: String,
        }
        let Args { directory } = parse(args)?;
        uninstall_for_app(context, "claude".to_string(), directory).await
    })
}

fn uninstall_skill_for_app(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            directory: String,
        }
        let Args { app, directory } = parse(args)?;
        uninstall_for_app(context, app, directory).await
    })
}

async fn uninstall_for_app(
    context: Arc<Context>,
    app: String,
    directory: String,
) -> Result<Value, String> {
    to_app_type(&app)?;
    let state = context.require_state()?;

    let skills = SkillService::get_all_installed(&state.db).map_err(|e| e.to_string())?;
    let skill = skills
        .into_iter()
        .find(|skill| skill.directory.eq_ignore_ascii_case(&directory))
        .ok_or_else(|| format!("未找到已安装的 Skill: {directory}"))?;

    let result: SkillUninstallResult =
        SkillService::uninstall(&state.db, &skill.id).map_err(|e| e.to_string())?;
    serializable(result)
}

fn get_skill_repos(context: Arc<Context>, _args: Value) -> HandlerFuture {
    Box::pin(async move {
        let state = context.require_state()?;
        deferred(move || {
            state
                .db
                .get_skill_repos()
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn add_skill_repo(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            repo: crate::services::skill::SkillRepo,
        }
        let Args { repo } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            // 整个结构体由前端反序列化而来，owner/name/branch 会被拼进归档下载
            // URL。主防线在 download_repo，这里让非法值当场报错而不是沉淀进表。
            SkillService::validate_repo_ref(&repo.owner, &repo.name, &repo.branch)
                .map_err(|error| error.to_string())?;
            state
                .db
                .save_skill_repo(&repo)
                .map(|_| true)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn remove_skill_repo(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            owner: String,
            name: String,
        }
        let Args { owner, name } = parse(args)?;
        let state = context.require_state()?;

        deferred(move || {
            state
                .db
                .delete_skill_repo(&owner, &name)
                .map(|_| true)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

/// 从 ZIP 安装；`filePath` 是前端上传后服务端上的真实路径。
fn install_skills_from_zip(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            file_path: String,
            current_app: String,
        }
        let Args {
            file_path,
            current_app,
        } = parse(args)?;
        let app_type = to_app_type(&current_app)?;
        let state = context.require_state()?;

        deferred(move || {
            SkillService::install_from_zip(&state.db, std::path::Path::new(&file_path), &app_type)
                .map_err(|error| error.to_string())
        })
        .await
    })
}
