//! Stream transcript chunks without constructing one huge HTTP JSON response.
use super::Context;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
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

pub async fn stream(State(context): State<Arc<Context>>, Json(args): Json<Request>) -> Response {
    let mut shutdown = context.shutdown.subscribe();
    let mut generation = context.auth_generation.subscribe();
    let stream = async_stream::stream! {
        let loaded = tokio::select! {
            biased;
            _ = shutdown.changed() => return,
            _ = generation.changed() => return,
            result = tokio::task::spawn_blocking(move || crate::session_manager::load_transcript(&args.provider_id, &args.source_path)) => result,
        };
        let loaded = match loaded {
            Ok(Ok(loaded)) => loaded,
            _ => { yield Ok::<_, Infallible>(bytes::Bytes::from("{\"type\":\"error\",\"message\":\"会话读取失败，请检查文件与服务端日志\"}\n")); return; }
        };
        let header = json!({"type":"header", "total":loaded.transcript.messages.len(), "turns":loaded.transcript.turns,
            "cached":loaded.cached, "parseMs":loaded.parse_ms});
        yield Ok(bytes::Bytes::from(format!("{header}\n")));
        for (batch, messages) in loaded.transcript.messages.chunks(100).enumerate() {
            if *shutdown.borrow() || generation.has_changed().unwrap_or(true) { return; }
            let chunk = json!({"type":"messages","start":batch * 100,"messages":messages});
            yield Ok(bytes::Bytes::from(format!("{chunk}\n")));
            tokio::task::yield_now().await;
        }
        yield Ok(bytes::Bytes::from(format!("{}\n",json!({"type":"done","payloadBytes":loaded.transcript.approx_bytes}))));
    };
    let mut response = axum::body::Body::from_stream(stream).into_response();
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
