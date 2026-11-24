use crate::auth::{AuthUser, SessionManager};
use crate::db::Database;
use crate::models::User;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AuthState {
    pub db: Database,
    pub session_mgr: SessionManager,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

pub async fn login(
    State(state): State<AuthState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let user_opt = state
        .db
        .get_user_by_username(&payload.username)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    if let Some(user) = user_opt {
        if bcrypt::verify(&payload.password, &user.password_hash).unwrap_or(false) {
            let token = state.session_mgr.create_session(user.clone());

            let mut headers = HeaderMap::new();
            let cookie_val = format!(
                "pxe_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
                token
            );
            headers.insert(header::SET_COOKIE, cookie_val.parse().unwrap());

            return Ok((
                StatusCode::OK,
                headers,
                Json(LoginResponse { token, user }),
            ));
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "Invalid username or password" })),
    ))
}

pub async fn logout(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Clear session token if present
    if let Some(cookie_hdr) = headers.get(header::COOKIE).and_then(|h| h.to_str().ok()) {
        for part in cookie_hdr.split(';') {
            let trimmed = part.trim();
            if let Some(token) = trimmed.strip_prefix("pxe_session=") {
                state.session_mgr.remove_session(token);
            }
        }
    }

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::SET_COOKIE,
        "pxe_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax".parse().unwrap(),
    );

    (
        StatusCode::OK,
        response_headers,
        Json(MessageResponse {
            message: "Logged out successfully".to_string(),
        }),
    )
}

pub async fn me(AuthUser(user): AuthUser) -> Json<User> {
    Json(user)
}
