use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticAssets;

pub async fn static_handler(path: Option<String>) -> impl IntoResponse {
    let raw_path = path.unwrap_or_else(|| "index.html".to_string());
    let trimmed = raw_path.trim_start_matches('/');
    let target_path = if trimmed.is_empty() {
        "index.html"
    } else {
        trimmed
    };

    if let Some(content) = StaticAssets::get(target_path) {
        let mime = mime_guess::from_path(target_path).first_or_octet_stream();
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .body(Body::from(content.data))
            .unwrap()
    } else if let Some(content) = StaticAssets::get("index.html") {
        // SPA Fallback
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(content.data))
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap()
    }
}
