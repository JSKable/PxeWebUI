mod assets;
mod auth;
mod config;
mod db;
mod models;
mod routes;
mod services;

use assets::static_handler;
use auth::SessionManager;
use clap::Parser;
use config::AppConfig;
use db::Database;
use routes::{
    auth::{login, logout, me, AuthState},
    download::{cancel_download, get_presets, list_downloads, start_download},
    iso::{delete_iso, get_iso, list_isos, update_iso, upload_iso, IsoState},
    pxe::{get_boot_ipxe, get_grub_cfg, get_pxe_preview, serve_iso, serve_tftp_file, PxeState},
    settings::{get_settings, update_settings, SettingsState},
    stats::{get_logs, get_stats, StatsState},
    users::{create_user, delete_user, list_users, update_user},
};
use services::{dnsmasq::DnsmasqManager, downloader::DownloadManager};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::System;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::DefaultBodyLimit;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub session_mgr: SessionManager,
    pub config: AppConfig,
    pub downloader: DownloadManager,
    pub start_time: Instant,
    pub sysinfo: Arc<Mutex<System>>,
}

// Implement FromRef for all sub-states
impl axum::extract::FromRef<AppState> for Database {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl axum::extract::FromRef<AppState> for SessionManager {
    fn from_ref(state: &AppState) -> Self {
        state.session_mgr.clone()
    }
}

impl axum::extract::FromRef<AppState> for AuthState {
    fn from_ref(state: &AppState) -> Self {
        AuthState {
            db: state.db.clone(),
            session_mgr: state.session_mgr.clone(),
        }
    }
}

impl axum::extract::FromRef<AppState> for IsoState {
    fn from_ref(state: &AppState) -> Self {
        IsoState {
            db: state.db.clone(),
            iso_dir: state.config.iso_dir.clone(),
        }
    }
}

impl axum::extract::FromRef<AppState> for DownloadManager {
    fn from_ref(state: &AppState) -> Self {
        state.downloader.clone()
    }
}

impl axum::extract::FromRef<AppState> for PxeState {
    fn from_ref(state: &AppState) -> Self {
        PxeState {
            db: state.db.clone(),
            config: state.config.clone(),
            iso_dir: state.config.iso_dir.clone(),
            tftp_dir: state.config.tftp_dir.clone(),
        }
    }
}

impl axum::extract::FromRef<AppState> for SettingsState {
    fn from_ref(state: &AppState) -> Self {
        SettingsState {
            db: state.db.clone(),
            config: state.config.clone(),
        }
    }
}

impl axum::extract::FromRef<AppState> for StatsState {
    fn from_ref(state: &AppState) -> Self {
        StatsState {
            db: state.db.clone(),
            config: state.config.clone(),
            downloader: state.downloader.clone(),
            start_time: state.start_time,
            sysinfo: state.sysinfo.clone(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pxewebui=debug".into()),
        )
        .init();

    let config = AppConfig::parse();

    info!("Starting PxeWebUI Server v{}", env!("CARGO_PKG_VERSION"));
    info!("Data directory: {:?}", config.data_dir);
    info!("ISO directory:  {:?}", config.iso_dir);
    info!("TFTP directory: {:?}", config.tftp_dir);

    // Ensure required directories exist
    std::fs::create_dir_all(&config.data_dir)?;
    std::fs::create_dir_all(&config.iso_dir)?;
    std::fs::create_dir_all(&config.tftp_dir)?;

    // Initialize database
    let db_path = config.data_dir.join("pxewebui.db");
    let db = Database::new(db_path)?;

    let server_ip = config.get_server_ip();
    info!("Detected / Configured Server IP: {}", server_ip);

    // Initial dnsmasq configuration sync
    let settings = db.get_settings(&server_ip, config.http_port, config.tftp_port)?;
    let dnsmasq_conf_path = config.data_dir.join("dnsmasq.conf");
    let _ = DnsmasqManager::write_config_file(
        &dnsmasq_conf_path,
        &settings.server_ip,
        &config.tftp_dir,
        settings.http_port,
        settings.proxydhcp_enabled,
    );

    let session_mgr = SessionManager::new();
    let downloader = DownloadManager::new(config.iso_dir.clone(), db.clone());
    let sysinfo = Arc::new(Mutex::new(System::new_all()));

    let app_state = AppState {
        db,
        session_mgr,
        config: config.clone(),
        downloader,
        start_time: Instant::now(),
        sysinfo,
    };

    // Build router
    use axum::routing::{delete, get, post, put};
    let app = axum::Router::new()
        // Public PXE & Boot Endpoints
        .route("/boot.ipxe", get(get_boot_ipxe))
        .route("/boot/grub/grub.cfg", get(get_grub_cfg))
        .route("/iso/:filename", get(serve_iso))
        .route("/tftp/:filename", get(serve_tftp_file))
        // Auth API
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        // User Management API
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", put(update_user).delete(delete_user))
        // ISO Management API
        .route("/api/isos", get(list_isos))
        .route("/api/isos/:id", get(get_iso).put(update_iso).delete(delete_iso))
        .route("/api/isos/upload", post(upload_iso))
        // Downloads API
        .route("/api/downloads", get(list_downloads).post(start_download))
        .route("/api/downloads/:id", delete(cancel_download))
        .route("/api/downloads/presets", get(get_presets))
        // Settings API
        .route("/api/settings", get(get_settings).put(update_settings))
        // Stats & Client Logs
        .route("/api/stats", get(get_stats))
        .route("/api/logs", get(get_logs))
        // PXE script preview
        .route("/api/pxe/preview", get(get_pxe_preview))
        // Static assets fallback
        .route("/", get(|| static_handler(None)))
        .route("/*path", get(|axum::extract::Path(path): axum::extract::Path<String>| static_handler(Some(path))))
        // Support large ISO file uploads (50 GB limit)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(app_state);

    let bind_addr: SocketAddr = config.bind_addr.parse().expect("Invalid bind address");
    info!("PxeWebUI HTTP Server listening on http://{}", bind_addr);
    info!("Access Web UI: http://{}:{}", server_ip, config.http_port);
    info!("PXE iPXE URL:  http://{}:{}/boot.ipxe", server_ip, config.http_port);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}
