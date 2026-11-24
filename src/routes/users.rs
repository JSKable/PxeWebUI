use crate::auth::AuthUser;
use crate::db::Database;
use crate::models::{CreateUserRequest, UpdateUserRequest, User};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

pub async fn list_users(
    _auth: AuthUser,
    State(db): State<Database>,
) -> Result<Json<Vec<User>>, (StatusCode, Json<serde_json::Value>)> {
    let users = db.list_users().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;
    Ok(Json(users))
}

pub async fn create_user(
    AuthUser(current_user): AuthUser,
    State(db): State<Database>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>), (StatusCode, Json<serde_json::Value>)> {
    if current_user.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Only administrators can create users" })),
        ));
    }

    if payload.username.trim().is_empty() || payload.password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Username cannot be empty and password must be at least 6 characters" })),
        ));
    }

    let role = payload.role.unwrap_or_else(|| "operator".to_string());
    let new_user = db
        .create_user(payload.username.trim(), &payload.password, &role)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Failed to create user: {}", e) })),
            )
        })?;

    Ok((StatusCode::CREATED, Json(new_user)))
}

pub async fn update_user(
    AuthUser(current_user): AuthUser,
    State(db): State<Database>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Only admin can change other users, but user can change their own password
    if current_user.role != "admin" && current_user.id != id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Permission denied" })),
        ));
    }

    let role_to_set = if current_user.role == "admin" {
        payload.role.as_deref()
    } else {
        None
    };

    db.update_user(id, payload.password.as_deref(), role_to_set)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn delete_user(
    AuthUser(current_user): AuthUser,
    State(db): State<Database>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if current_user.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Only administrators can delete users" })),
        ));
    }

    if current_user.id == id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Cannot delete your own account" })),
        ));
    }

    let deleted = db.delete_user(id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    if !deleted {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Cannot delete user (might be the last remaining admin)" })),
        ));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}
