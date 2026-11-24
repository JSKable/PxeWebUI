use crate::db::Database;
use crate::models::User;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, (User, Instant)>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_session(&self, user: User) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let mut sessions = self.sessions.lock().unwrap();
        // Expire old sessions after 24 hours
        let now = Instant::now();
        sessions.retain(|_, (_, time)| now.duration_since(*time) < Duration::from_secs(86400));
        sessions.insert(token.clone(), (user, now));
        token
    }

    pub fn validate_token(&self, token: &str) -> Option<User> {
        let sessions = self.sessions.lock().unwrap();
        if let Some((user, created_at)) = sessions.get(token) {
            if Instant::now().duration_since(*created_at) < Duration::from_secs(86400) {
                return Some(user.clone());
            }
        }
        None
    }

    pub fn remove_session(&self, token: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(token);
    }
}

pub struct AuthUser(pub User);

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthErrorResponse {
    pub error: String,
}

impl IntoResponse for AuthErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"PxeWebUI\"")],
            Json(self),
        )
            .into_response()
    }
}

// Extractor that works with AppState
#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Database: FromRef<S>,
    SessionManager: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let db = Database::from_ref(state);
        let session_mgr = SessionManager::from_ref(state);

        // 1. Check Cookie: pxe_session=<token>
        if let Some(cookie_hdr) = parts.headers.get(header::COOKIE).and_then(|h| h.to_str().ok()) {
            for part in cookie_hdr.split(';') {
                let trimmed = part.trim();
                if let Some(token) = trimmed.strip_prefix("pxe_session=") {
                    if let Some(user) = session_mgr.validate_token(token) {
                        return Ok(AuthUser(user));
                    }
                }
            }
        }

        // 2. Check Authorization header
        if let Some(auth_hdr) = parts.headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
            // Bearer Token
            if let Some(token) = auth_hdr.strip_prefix("Bearer ") {
                if let Some(user) = session_mgr.validate_token(token.trim()) {
                    return Ok(AuthUser(user));
                }
            }

            // Basic Auth
            if let Some(basic_b64) = auth_hdr.strip_prefix("Basic ") {
                if let Ok(decoded) = BASE64.decode(basic_b64.trim()) {
                    if let Ok(credentials) = String::from_utf8(decoded) {
                        if let Some((username, password)) = credentials.split_once(':') {
                            if let Ok(Some(user)) = db.get_user_by_username(username) {
                                if bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
                                    return Ok(AuthUser(user));
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(AuthErrorResponse {
            error: "Authentication required. Please log in with Username & Password.".to_string(),
        }
        .into_response())
    }
}
