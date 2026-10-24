//! Stream transcript chunks without constructing one huge HTTP JSON response.
use super::{sessions, Context};
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    provider_id: String,
    source_path: String,
}

pub async fn stream(
    State(context): State<Arc<Context>>,
    request_id: Option<Extension<super::diagnostics::RequestId>>,
    Json(args): Json<Request>,
) -> Response {
    let id = request_id
        .map(|Extension(value)| value.0)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let permit = match sessions::acquire() {
        Ok(permit) => permit,
        Err(error) => {
            let mut response = (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"ok":false,"error":error,"code":"E_BUSY","requestId":id})),
            )
                .into_response();
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
            response
                .extensions_mut()
                .insert(super::diagnostics::Outcome {
                    command: "session_transcript",
                    code: Some("E_BUSY"),
                });
            return response;
        }
    };
    let worker_permit = permit.clone();
    let mut shutdown = context.shutdown.subscribe();
    let mut generation = context.auth_generation.subscribe();
    let stream = async_stream::stream! {
        // Both the worker and the response own the slot: disconnects cannot admit extra parsers.
        let _permit = permit;
        let loaded = tokio::select! {
            biased;
            _ = shutdown.changed() => return,
            _ = generation.changed() => return,
            result = tokio::task::spawn_blocking(move || {
                let _permit = worker_permit;
                sessions::load(&args.provider_id, &args.source_path)
            }) => result,
        };
        let loaded = match loaded {
            Ok(Ok(loaded)) => loaded,
            error => {
                let code = match &error {
                    Ok(Err(error)) if error.starts_with("E_SESSION_LIMIT:") => "E_SESSION_LIMIT",
                    _ => "E_SESSION_READ",
                };
                log::warn!("web session request_id={id} code={code}");
                let message = if code == "E_SESSION_LIMIT" {
                    "会话超过 Web 读取限额（源 64 MiB、结果 32 MiB、单条消息 2 MiB），请在 CLI 中查看"
                } else { "会话读取失败，请检查文件与服务端日志" };
                yield Ok::<_, Infallible>(bytes::Bytes::from(format!("{}\n", json!({"type":"error","message":message,"code":code,"requestId":id}))));
                return;
            }
        };
        let header = json!({"type":"header", "total":loaded.transcript.messages.len(), "turns":loaded.transcript.turns,
            "cached":loaded.cached, "parseMs":loaded.parse_ms});
        yield Ok(bytes::Bytes::from(format!("{header}\n")));
        for (start, end) in crate::session_manager::cache::chunk_ranges(&loaded.transcript.message_bytes) {
            if *shutdown.borrow() || generation.has_changed().unwrap_or(true) { return; }
            let chunk = json!({"type":"messages","start":start,"messages":&loaded.transcript.messages[start..end]});
            yield Ok(bytes::Bytes::from(format!("{chunk}\n")));
            tokio::task::yield_now().await;
        }
        yield Ok(bytes::Bytes::from(format!("{}\n",json!({"type":"done","payloadBytes":loaded.transcript.approx_bytes}))));
    };
    let mut response = axum::body::Body::from_stream(stream).into_response();
    response
        .extensions_mut()
        .insert(super::diagnostics::Outcome {
            command: "session_transcript",
            code: None,
        });
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/x-ndjson"),
    );
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
