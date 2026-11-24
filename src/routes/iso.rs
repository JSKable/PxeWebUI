use crate::auth::AuthUser;
use crate::db::Database;
use crate::models::{Iso, UpdateIsoRequest};
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::info;

#[derive(Clone)]
pub struct IsoState {
    pub db: Database,
    pub iso_dir: PathBuf,
}

pub async fn list_isos(
    _auth: AuthUser,
    State(state): State<IsoState>,
) -> Result<Json<Vec<Iso>>, (StatusCode, Json<serde_json::Value>)> {
    let isos = state.db.list_isos().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;
    Ok(Json(isos))
}

pub async fn get_iso(
    _auth: AuthUser,
    State(state): State<IsoState>,
    Path(id): Path<String>,
) -> Result<Json<Iso>, (StatusCode, Json<serde_json::Value>)> {
    let iso = state
        .db
        .get_iso(&id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "ISO not found" })),
            )
        })?;

    Ok(Json(iso))
}

pub async fn update_iso(
    _auth: AuthUser,
    State(state): State<IsoState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateIsoRequest>,
) -> Result<Json<Iso>, (StatusCode, Json<serde_json::Value>)> {
    let mut iso = state
        .db
        .get_iso(&id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "ISO not found" })),
            )
        })?;

    if let Some(title) = payload.title {
        iso.title = title;
    }
    if let Some(desc) = payload.description {
        iso.description = desc;
    }
    if let Some(cat) = payload.category {
        iso.category = cat;
    }
    if let Some(boot) = payload.boot_mode {
        iso.boot_mode = boot;
    }
    if let Some(custom) = payload.custom_ipxe {
        iso.custom_ipxe = Some(custom);
    }
    if let Some(kargs) = payload.kernel_args {
        iso.kernel_args = Some(kargs);
    }
    if let Some(bios) = payload.arch_bios {
        iso.arch_bios = bios;
    }
    if let Some(uefi64) = payload.arch_uefi64 {
        iso.arch_uefi64 = uefi64;
    }
    if let Some(uefi32) = payload.arch_uefi32 {
        iso.arch_uefi32 = uefi32;
    }
    if let Some(arm64) = payload.arch_arm64 {
        iso.arch_arm64 = arm64;
    }
    if let Some(sb) = payload.secure_boot_supported {
        iso.secure_boot_supported = sb;
    }
    if let Some(def) = payload.is_default {
        iso.is_default = def;
    }
    if let Some(en) = payload.is_enabled {
        iso.is_enabled = en;
    }

    iso.updated_at = chrono::Utc::now().to_rfc3339();

    state.db.update_iso(&iso).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(iso))
}

pub async fn delete_iso(
    _auth: AuthUser,
    State(state): State<IsoState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let filename_opt = state.db.delete_iso(&id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    if let Some(filename) = filename_opt {
        let file_path = state.iso_dir.join(&filename);
        if file_path.exists() {
            tokio::fs::remove_file(&file_path).await.ok();
            info!("Deleted ISO file from disk: {:?}", file_path);
        }
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn upload_iso(
    _auth: AuthUser,
    State(state): State<IsoState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Iso>), (StatusCode, Json<serde_json::Value>)> {
    let mut filename = String::new();
    let mut title = String::new();
    let mut description = String::new();
    let mut category = "linux".to_string();
    let mut boot_mode = "sanboot".to_string();
    let mut secure_boot = false;
    let mut file_size: u64 = 0;

    std::fs::create_dir_all(&state.iso_dir).ok();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();

        match name.as_str() {
            "title" => {
                if let Ok(text) = field.text().await {
                    title = text;
                }
            }
            "description" => {
                if let Ok(text) = field.text().await {
                    description = text;
                }
            }
            "category" => {
                if let Ok(text) = field.text().await {
                    category = text;
                }
            }
            "boot_mode" => {
                if let Ok(text) = field.text().await {
                    boot_mode = text;
                }
            }
            "secure_boot_supported" => {
                if let Ok(text) = field.text().await {
                    secure_boot = text == "true" || text == "1";
                }
            }
            "file" => {
                let original_name = field
                    .file_name()
                    .unwrap_or("uploaded.iso")
                    .to_string();

                filename = original_name.replace(['/', '\\'], "_");
                if !filename.ends_with(".iso") && !filename.ends_with(".img") {
                    filename.push_str(".iso");
                }

                let dest_path = state.iso_dir.join(&filename);
                let mut out_file = File::create(&dest_path).await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": format!("Cannot create file: {}", e) })),
                    )
                })?;

                while let Ok(Some(chunk)) = field.chunk().await {
                    file_size += chunk.len() as u64;
                    out_file.write_all(&chunk).await.map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "error": format!("Write error: {}", e) })),
                        )
                    })?;
                }
                out_file.flush().await.ok();
            }
            _ => {}
        }
    }

    if filename.is_empty() || file_size == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "No file uploaded or file is empty" })),
        ));
    }

    if title.is_empty() {
        title = filename
            .trim_end_matches(".iso")
            .trim_end_matches(".img")
            .replace(['-', '_'], " ");
    }

    let now = chrono::Utc::now().to_rfc3339();
    let new_iso = Iso {
        id: uuid::Uuid::new_v4().to_string(),
        filename,
        title,
        description,
        category,
        file_size: file_size as i64,
        boot_mode,
        custom_ipxe: None,
        kernel_args: None,
        arch_bios: true,
        arch_uefi64: true,
        arch_uefi32: false,
        arch_arm64: false,
        secure_boot_supported: secure_boot,
        is_default: false,
        is_enabled: true,
        checksum_sha256: None,
        created_at: now.clone(),
        updated_at: now,
    };

    state.db.insert_iso(&new_iso).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(new_iso)))
}
