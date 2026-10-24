//! Serve build-time gzip sidecars; never compress large assets on an async worker.
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use std::sync::LazyLock;

pub fn accepts_gzip(headers: &HeaderMap) -> bool {
    let mut explicit = None;
    let mut wildcard = false;
    for value in headers.get_all(header::ACCEPT_ENCODING) {
        let Ok(value) = value.to_str() else { continue };
        for part in value.split(',') {
            let mut fields = part.trim().split(';');
            let coding = fields.next().unwrap_or("").trim();
            let quality = fields
                .filter_map(|field| field.trim().split_once('='))
                .find(|(key, _)| key.trim().eq_ignore_ascii_case("q"))
                .map(|(_, value)| value.trim().parse::<f32>().unwrap_or(0.0))
                .unwrap_or(1.0);
            let allowed = quality > 0.0 && quality <= 1.0;
            if coding.eq_ignore_ascii_case("gzip") {
                explicit = Some(allowed);
            } else if coding == "*" {
                wildcard = allowed;
            }
        }
    }
    explicit.unwrap_or(wildcard)
}

pub fn response(path: &str, bytes: Vec<u8>, gzip: bool) -> Response {
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    if let Ok(value) =
        HeaderValue::from_str(mime_guess::from_path(path).first_or_octet_stream().as_ref())
    {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if gzip {
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    static HASHED_ASSET: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^assets/.+-[A-Za-z0-9_-]{8,}\.[A-Za-z0-9.]+$")
            .expect("static asset regex")
    });
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if HASHED_ASSET.is_match(path) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    response
}
