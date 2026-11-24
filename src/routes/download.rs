use crate::auth::AuthUser;
use crate::models::{DownloadTask, IsoPreset, StartDownloadRequest};
use crate::services::downloader::DownloadManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

pub async fn list_downloads(
    _auth: AuthUser,
    State(downloader): State<DownloadManager>,
) -> Json<Vec<DownloadTask>> {
    Json(downloader.list_tasks())
}

pub async fn start_download(
    _auth: AuthUser,
    State(downloader): State<DownloadManager>,
    Json(payload): Json<StartDownloadRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if payload.url.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "URL cannot be empty" })),
        ));
    }

    let id = downloader.start_download(payload);
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "task_id": id, "status": "started" })),
    ))
}

pub async fn cancel_download(
    _auth: AuthUser,
    State(downloader): State<DownloadManager>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let cancelled = downloader.cancel_task(&id);
    if cancelled {
        Ok(Json(serde_json::json!({ "success": true })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Task not found or not active" })),
        ))
    }
}

pub async fn get_presets(_auth: AuthUser) -> Json<Vec<IsoPreset>> {
    Json(DownloadManager::get_presets())
}
