//! Prompts 管理命令（对应桌面版 `commands/prompt.rs`）。
//!
//! 全部是 `PromptService` / `PiPromptFileService` 的薄封装，两个构建共用同一份
//! 业务逻辑。`import_prompt_from_file` 读的是**服务端**磁盘上的提示词文件，
//! 不需要浏览器参与。

use std::sync::Arc;

use serde_json::Value;

use super::routes::{deferred, parse, serializable, to_app_type, Handler, HandlerFuture};
use super::Context;
use crate::services::pi_prompt_files::{
    PiPromptFileKind, PiPromptFileService, PiPromptTemplateService,
};
use crate::services::prompt::PromptService;

pub const HANDLERS: &[(&str, Handler)] = &[
    ("get_prompts", get_prompts),
    ("upsert_prompt", upsert_prompt),
    ("delete_prompt", delete_prompt),
    ("enable_prompt", enable_prompt),
    ("import_prompt_from_file", import_prompt_from_file),
    (
        "get_current_prompt_file_content",
        get_current_prompt_file_content,
    ),
    ("get_prompt_file_location", get_prompt_file_location),
    ("get_pi_prompt_file", get_pi_prompt_file),
    ("replace_pi_prompt_file", replace_pi_prompt_file),
    ("delete_pi_prompt_file", delete_pi_prompt_file),
    ("list_pi_prompt_templates", list_pi_prompt_templates),
    ("upsert_pi_prompt_template", upsert_pi_prompt_template),
    ("delete_pi_prompt_template", delete_pi_prompt_template),
];

// ============================================================================
// 通用提示词库
// ============================================================================

fn get_prompts(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            PromptService::get_prompts(&state, app_type).map_err(|error| error.to_string())
        })
        .await
    })
}

fn upsert_prompt(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
            prompt: crate::prompt::Prompt,
        }
        let Args { app, id, prompt } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            PromptService::upsert_prompt(&state, app_type, &id, prompt)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn delete_prompt(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
        }
        let Args { app, id } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            PromptService::delete_prompt(&state, app_type, &id)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn enable_prompt(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
            id: String,
        }
        let Args { app, id } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            PromptService::enable_prompt(&state, app_type, &id)
                .map(|_| Value::Null)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

/// 从服务端已有的提示词文件导入一条记录。
fn import_prompt_from_file(context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let app_type = to_app_type(&app)?;
        let state = context.require_state()?;

        deferred(move || {
            PromptService::import_from_file(&state, app_type).map_err(|error| error.to_string())
        })
        .await
    })
}

fn get_current_prompt_file_content(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let app_type = to_app_type(&app)?;

        deferred(move || {
            PromptService::get_current_file_content(app_type).map_err(|error| error.to_string())
        })
        .await
    })
}

/// 提示词目标文件的位置：`path` 是完整路径（复制用），`displayPath` 把主目录写成 `~`。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptFileLocation {
    path: String,
    display_path: String,
}

fn get_prompt_file_location(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            app: String,
        }
        let Args { app } = parse(args)?;
        let app_type = to_app_type(&app)?;

        let path = crate::prompt_files::prompt_file_path(&app_type).map_err(|e| e.to_string())?;
        serializable(PromptFileLocation {
            path: path.to_string_lossy().to_string(),
            display_path: crate::prompt_files::display_path(&path),
        })
    })
}

// ============================================================================
// Pi 的原生提示词文件与模板
// ============================================================================

fn get_pi_prompt_file(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: PiPromptFileKind,
        }
        let Args { kind } = parse(args)?;
        deferred(move || PiPromptFileService::read(kind).map_err(|error| error.to_string())).await
    })
}

fn replace_pi_prompt_file(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            kind: PiPromptFileKind,
            expected_revision: String,
            content: String,
        }
        let Args {
            kind,
            expected_revision,
            content,
        } = parse(args)?;
        deferred(move || {
            PiPromptFileService::replace(kind, &expected_revision, &content)
                .map_err(|error| error.to_string())
        })
        .await
    })
}

fn delete_pi_prompt_file(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            kind: PiPromptFileKind,
            expected_revision: String,
        }
        let Args {
            kind,
            expected_revision,
        } = parse(args)?;
        deferred(move || {
            PiPromptFileService::delete(kind, &expected_revision).map_err(|error| error.to_string())
        })
        .await
    })
}

fn list_pi_prompt_templates(_context: Arc<Context>, _args: Value) -> HandlerFuture {
    deferred(|| PiPromptTemplateService::list().map_err(|error| error.to_string()))
}

fn upsert_pi_prompt_template(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            slug: String,
            #[serde(default)]
            original_slug: Option<String>,
            expected_revision: String,
            content: String,
        }
        let Args {
            slug,
            original_slug,
            expected_revision,
            content,
        } = parse(args)?;
        deferred(move || {
            PiPromptTemplateService::upsert(
                &slug,
                original_slug.as_deref(),
                &expected_revision,
                &content,
            )
            .map_err(|error| error.to_string())
        })
        .await
    })
}

fn delete_pi_prompt_template(_context: Arc<Context>, args: Value) -> HandlerFuture {
    Box::pin(async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            slug: String,
            expected_revision: String,
        }
        let Args {
            slug,
            expected_revision,
        } = parse(args)?;
        deferred(move || {
            PiPromptTemplateService::delete(&slug, &expected_revision)
                .map_err(|error| error.to_string())
        })
        .await
    })
}
