use crate::auth::AuthUser;
use crate::config::AppConfig;
use crate::db::Database;
use crate::services::pxe_generator::PxeGenerator;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::path::PathBuf;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

#[derive(Clone)]
pub struct PxeState {
    pub db: Database,
    pub config: AppConfig,
    pub iso_dir: PathBuf,
    pub tftp_dir: PathBuf,
}

pub async fn get_boot_ipxe(
    State(state): State<PxeState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("client");

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("iPXE");

    let server_ip = state.config.get_server_ip();
    let settings = state
        .db
        .get_settings(&server_ip, state.config.http_port, state.config.tftp_port)
        .unwrap();

    let isos = state.db.list_isos().unwrap_or_default();

    // Log client boot request
    let platform = if user_agent.contains("UEFI") {
        Some("iPXE (UEFI)")
    } else if user_agent.contains("iPXE") {
        Some("iPXE (BIOS)")
    } else {
        Some("PXE Client")
    };
    state
        .db
        .log_client_access(client_ip, user_agent, "/boot.ipxe", platform)
        .ok();

    // Determine base URL dynamically (supports public domains like pxe.jskable.com and reverse proxies)
    let custom_base_url = if let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) {
        let proto = if headers.get("x-forwarded-proto").and_then(|h| h.to_str().ok()) == Some("https") {
            "https"
        } else {
            "http"
        };
        Some(format!("{}://{}", proto, host))
    } else {
        None
    };

    let script = PxeGenerator::generate_ipxe_script(
        &settings,
        &isos,
        &settings.server_ip,
        settings.http_port,
        custom_base_url.as_deref(),
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        script,
    )
}

pub async fn get_grub_cfg(
    State(state): State<PxeState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("client");

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("GRUB");

    let server_ip = state.config.get_server_ip();
    let settings = state
        .db
        .get_settings(&server_ip, state.config.http_port, state.config.tftp_port)
        .unwrap();

    let isos = state.db.list_isos().unwrap_or_default();

    state
        .db
        .log_client_access(client_ip, user_agent, "/boot/grub/grub.cfg", Some("GRUB (Secure Boot)"))
        .ok();

    let cfg = PxeGenerator::generate_grub_cfg(
        &settings,
        &isos,
        &settings.server_ip,
        settings.http_port,
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        cfg,
    )
}

pub async fn serve_iso(
    State(state): State<PxeState>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_path = state.iso_dir.join(&filename);
    if !file_path.exists() || !file_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("client");
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("HTTP Client");

    state
        .db
        .log_client_access(
            client_ip,
            user_agent,
            &format!("/iso/{}", filename),
            Some("ISO Stream"),
        )
        .ok();

    let mut file = File::open(&file_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metadata = file.metadata().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_len = metadata.len();

    // Check for HTTP Range Header (crucial for sanboot and ISO chunk booting!)
    if let Some(range_header) = headers.get(header::RANGE).and_then(|h| h.to_str().ok()) {
        if let Some(range) = parse_range_header(range_header, total_len) {
            use std::io::SeekFrom;
            use tokio::io::AsyncSeekExt;
            if file.seek(SeekFrom::Start(range.start)).await.is_ok() {
                let length = range.end - range.start + 1;
                let limited = tokio::io::AsyncReadExt::take(file, length);
                let stream = ReaderStream::new(limited);
                let body = Body::from_stream(stream);

                let content_range = format!("bytes {}-{}/{}", range.start, range.end, total_len);
                return Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, length.to_string())
                    .header(header::CONTENT_RANGE, content_range)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .body(body)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
            } else {
                return Err(StatusCode::RANGE_NOT_SATISFIABLE);
            }
        }
    }

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, total_len.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn serve_tftp_file(
    State(state): State<PxeState>,
    Path(filename): Path<String>,
) -> Result<Response, StatusCode> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_path = state.tftp_dir.join(&filename);
    if !file_path.exists() || !file_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let file = File::open(&file_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metadata = file.metadata().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, metadata.len().to_string())
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_pxe_preview(
    _auth: AuthUser,
    State(state): State<PxeState>,
) -> Json<serde_json::Value> {
    let server_ip = state.config.get_server_ip();
    let settings = state
        .db
        .get_settings(&server_ip, state.config.http_port, state.config.tftp_port)
        .unwrap();
    let isos = state.db.list_isos().unwrap_or_default();

    let ipxe = PxeGenerator::generate_ipxe_script(
        &settings,
        &isos,
        &settings.server_ip,
        settings.http_port,
        None,
    );

    let grub = PxeGenerator::generate_grub_cfg(
        &settings,
        &isos,
        &settings.server_ip,
        settings.http_port,
    );

    Json(serde_json::json!({
        "ipxe": ipxe,
        "grub": grub,
        "server_ip": settings.server_ip,
        "http_port": settings.http_port
    }))
}

struct ByteRange {
    start: u64,
    end: u64,
}

fn parse_range_header(header_val: &str, total_len: u64) -> Option<ByteRange> {
    if !header_val.starts_with("bytes=") {
        return None;
    }
    let range_str = &header_val[6..];
    let mut parts = range_str.split('-');
    let start_str = parts.next()?;
    let end_str = parts.next()?;

    let start = start_str.parse::<u64>().ok()?;
    let end = if end_str.is_empty() {
        total_len - 1
    } else {
        end_str.parse::<u64>().ok()?
    };

    if start <= end && end < total_len {
        Some(ByteRange { start, end })
    } else {
        None
    }
}
