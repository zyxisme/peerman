use axum::http::header::SET_COOKIE;
use axum::Json;
use serde::{Deserialize, Serialize};

use super::rate_limit::LOGIN_RATE_LIMITER;
use crate::app_config;
use crate::auth;

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct UserInfo {
    username: String,
}

#[derive(Serialize)]
pub(crate) struct MeResponse {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

pub async fn handle_health() -> &'static str {
    "ok"
}

pub async fn handle_login(
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> axum::response::Response {
    let ip = addr.ip().to_string();

    if let Err(retry_after) = LOGIN_RATE_LIMITER.check(&ip) {
        return json_response(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            &LoginResponse {
                success: false,
                user: None,
                error: Some(format!("Too many attempts. Try again in {retry_after}s")),
            },
            None,
        );
    }

    let cfg = app_config();
    if req.username != cfg.auth.username {
        return json_response(
            axum::http::StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                user: None,
                error: Some("Invalid credentials".into()),
            },
            None,
        );
    }

    let password_ok = if cfg.auth.password_hash.is_empty() {
        tracing::warn!("Using plaintext password comparison — set password_hash in config");
        req.password == cfg.auth.password
    } else {
        auth::password::verify_password(&req.password, &cfg.auth.password_hash).unwrap_or(false)
    };

    if !password_ok {
        return json_response(
            axum::http::StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                user: None,
                error: Some("Invalid credentials".into()),
            },
            None,
        );
    }

    let secret = if cfg.auth.jwt_secret.is_empty() {
        ""
    } else {
        &cfg.auth.jwt_secret
    };

    match auth::create_token(&req.username, secret) {
        Ok(token) => {
            let cookie = format!(
                "jwt={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600; Secure",
                token
            );
            json_response(
                axum::http::StatusCode::OK,
                &LoginResponse {
                    success: true,
                    user: Some(UserInfo {
                        username: req.username,
                    }),
                    error: None,
                },
                Some(&cookie),
            )
        }
        Err(e) => json_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            &LoginResponse {
                success: false,
                user: None,
                error: Some(format!("Token creation failed: {e}")),
            },
            None,
        ),
    }
}

pub async fn handle_logout() -> axum::response::Response {
    json_response(
        axum::http::StatusCode::OK,
        &serde_json::json!({"success": true}),
        Some("jwt=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0; Secure"),
    )
}

pub async fn handle_me(headers: axum::http::HeaderMap) -> Json<MeResponse> {
    let cfg = app_config();
    let cookie_header = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    match auth::parse_cookie(cookie_header, "jwt") {
        Some(token) => match auth::verify_token(token, &cfg.auth.jwt_secret) {
            Ok(claims) => Json(MeResponse {
                authenticated: true,
                username: Some(claims.sub),
            }),
            Err(_) => Json(MeResponse {
                authenticated: false,
                username: None,
            }),
        },
        None => Json(MeResponse {
            authenticated: false,
            username: None,
        }),
    }
}

fn json_response(
    status: axum::http::StatusCode,
    body: &impl Serialize,
    cookie: Option<&str>,
) -> axum::response::Response {
    let json = serde_json::to_string(body).unwrap_or_default();
    let mut builder = axum::response::Response::builder()
        .status(status)
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        builder = builder.header(SET_COOKIE, c);
    }
    builder
        .body(axum::body::Body::from(json))
        .expect("body is infallible")
}
