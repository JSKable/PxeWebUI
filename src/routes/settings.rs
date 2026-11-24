use crate::auth::AuthUser;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::Settings;
use crate::services::dnsmasq::DnsmasqManager;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;

#[derive(Clone)]
pub struct SettingsState {
    pub db: Database,
    pub config: AppConfig,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub server_ip: Option<String>,
    pub boot_timeout: Option<u32>,
    pub default_boot_action: Option<String>,
    pub proxydhcp_enabled: Option<bool>,
    pub menu_title: Option<String>,
    pub ipxe_banner: Option<String>,
}

pub async fn get_settings(
    _auth: AuthUser,
    State(state): State<SettingsState>,
) -> Result<Json<Settings>, (StatusCode, Json<serde_json::Value>)> {
    let server_ip = state.config.get_server_ip();
    let settings = state
        .db
        .get_settings(&server_ip, state.config.http_port, state.config.tftp_port)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;
    Ok(Json(settings))
}

pub async fn update_settings(
    AuthUser(user): AuthUser,
    State(state): State<SettingsState>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<Settings>, (StatusCode, Json<serde_json::Value>)> {
    if user.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Only administrators can change system settings" })),
        ));
    }

    if let Some(ref ip) = payload.server_ip {
        state.db.update_setting("server_ip", ip).ok();
    }
    if let Some(timeout) = payload.boot_timeout {
        state.db.update_setting("boot_timeout", &timeout.to_string()).ok();
    }
    if let Some(ref action) = payload.default_boot_action {
        state.db.update_setting("default_boot_action", action).ok();
    }
    if let Some(proxydhcp) = payload.proxydhcp_enabled {
        state.db.update_setting("proxydhcp_enabled", if proxydhcp { "true" } else { "false" }).ok();
    }
    if let Some(ref title) = payload.menu_title {
        state.db.update_setting("menu_title", title).ok();
    }
    if let Some(ref banner) = payload.ipxe_banner {
        state.db.update_setting("ipxe_banner", banner).ok();
    }

    let server_ip = state.config.get_server_ip();
    let updated = state
        .db
        .get_settings(&server_ip, state.config.http_port, state.config.tftp_port)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    // Regenerate dnsmasq configuration file if path exists
    let dnsmasq_conf = state.config.data_dir.join("dnsmasq.conf");
    let _ = DnsmasqManager::write_config_file(
        &dnsmasq_conf,
        &updated.server_ip,
        &state.config.tftp_dir,
        updated.http_port,
        updated.proxydhcp_enabled,
    );

    Ok(Json(updated))
}
