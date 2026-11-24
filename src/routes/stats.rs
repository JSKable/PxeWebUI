use crate::auth::AuthUser;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{ClientLog, SystemStats};
use crate::services::downloader::DownloadManager;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::{Disks, System};

#[derive(Clone)]
pub struct StatsState {
    pub db: Database,
    pub config: AppConfig,
    pub downloader: DownloadManager,
    pub start_time: Instant,
    pub sysinfo: Arc<Mutex<System>>,
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
}

pub async fn get_stats(
    _auth: AuthUser,
    State(state): State<StatsState>,
) -> Result<Json<SystemStats>, (StatusCode, Json<serde_json::Value>)> {
    let mut sys = state.sysinfo.lock().unwrap();
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let ram_used_mb = sys.used_memory() / (1024 * 1024);
    let ram_total_mb = sys.total_memory() / (1024 * 1024);

    let mut disk_free_gb = 0.0;
    let mut disk_total_gb = 0.0;

    // Find the disk volume hosting the ISO directory
    let canonical_iso_dir = state
        .config
        .iso_dir
        .canonicalize()
        .unwrap_or_else(|_| state.config.iso_dir.clone());

    let disks = Disks::new_with_refreshed_list();
    let mut found_disk = false;
    for disk in &disks {
        if canonical_iso_dir.starts_with(disk.mount_point()) {
            disk_free_gb = (disk.available_space() as f64) / (1024.0 * 1024.0 * 1024.0);
            disk_total_gb = (disk.total_space() as f64) / (1024.0 * 1024.0 * 1024.0);
            found_disk = true;
            break;
        }
    }

    if !found_disk {
        if let Some(disk) = disks.first() {
            disk_free_gb = (disk.available_space() as f64) / (1024.0 * 1024.0 * 1024.0);
            disk_total_gb = (disk.total_space() as f64) / (1024.0 * 1024.0 * 1024.0);
        }
    }

    let isos = state.db.list_isos().unwrap_or_default();
    let total_isos = isos.len();

    let active_downloads = state
        .downloader
        .list_tasks()
        .iter()
        .filter(|t| t.status == "downloading")
        .count();

    let uptime = state.start_time.elapsed().as_secs();

    Ok(Json(SystemStats {
        cpu_usage,
        ram_used_mb,
        ram_total_mb,
        disk_free_gb,
        disk_total_gb,
        total_isos,
        active_downloads,
        server_ip: state.config.get_server_ip(),
        http_port: state.config.http_port,
        uptime_seconds: uptime,
    }))
}

pub async fn get_logs(
    _auth: AuthUser,
    State(state): State<StatsState>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<Vec<ClientLog>>, (StatusCode, Json<serde_json::Value>)> {
    let limit = query.limit.unwrap_or(100).min(500);
    let logs = state.db.list_recent_logs(limit).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;
    Ok(Json(logs))
}
