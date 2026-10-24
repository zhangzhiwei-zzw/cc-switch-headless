//! Correlate failures without logging tokens, request arguments, query strings or bodies.
use axum::{
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use std::time::Instant;

#[derive(Clone)]
pub struct RequestId(pub String);

#[derive(Clone)]
pub struct Outcome {
    pub command: &'static str,
    pub code: Option<&'static str>,
}

pub async fn trace_request(mut request: Request<axum::body::Body>, next: Next) -> Response {
    let id = uuid::Uuid::new_v4().to_string();
    request.extensions_mut().insert(RequestId(id.clone()));
    let method = match request.method().as_str() {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => {
            request.method().to_string()
        }
        _ => "OTHER".to_string(),
    };
    let path = request.uri().path().to_string();
    let started = Instant::now();
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&id).expect("UUID header"),
    );
    if path.starts_with("/api/") || path == "/auth" {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
        let outcome = response.extensions().get::<Outcome>();
        let command = outcome.map(|value| value.command).unwrap_or("-");
        let code = outcome.and_then(|value| value.code).unwrap_or("-");
        // Do not log arbitrary paths: even URL paths can contain sensitive user input.
        let endpoint = match path.as_str() {
            "/api/invoke"
            | "/api/events"
            | "/api/session-transcript"
            | "/api/upload"
            | "/api/download"
            | "/api/rotate-token"
            | "/api/env"
            | "/api/capabilities"
            | "/api/commands"
            | "/auth" => path.as_str(),
            _ => "/api/<unknown>",
        };
        let status = response.status().as_u16();
        let elapsed = started.elapsed().as_millis();
        if status >= 400 || code != "-" {
            log::warn!("web request_id={id} method={method} endpoint={endpoint} command={command} status={status} code={code} elapsed_ms={elapsed}");
        } else {
            log::info!("web request_id={id} method={method} endpoint={endpoint} command={command} status={status} code={code} elapsed_ms={elapsed}");
        }
    }
    response
}
