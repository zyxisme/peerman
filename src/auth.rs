use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tonic::{metadata::MetadataMap, Request, Status};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Generate a random 64-character hex JWT secret.
pub fn generate_jwt_secret() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| format!("{:02x}", rng.gen::<u8>()))
        .collect()
}

/// Create a JWT token for the given username, valid for 1 hour.
pub fn create_token(username: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: username.to_string(),
        iat: now,
        exp: now + 3600, // 1 hour (was 30 days)
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Verify and decode a JWT token.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}

/// Extract a named value from a Cookie header string.
pub fn parse_cookie<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    cookie_header.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        if k == name {
            Some(v)
        } else {
            None
        }
    })
}

/// Extract the JWT token from gRPC request metadata (Cookie header).
fn extract_jwt(metadata: &MetadataMap) -> Option<String> {
    let cookie = metadata
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    parse_cookie(cookie, "jwt").map(|s| s.to_string())
}

/// Check auth on a gRPC request. Returns Ok(()) if the request is authenticated,
/// or an unauthenticated Status if not.
#[allow(clippy::result_large_err)]
pub fn check_auth<T>(req: &Request<T>, secret: &str) -> Result<(), Status> {
    let token = extract_jwt(req.metadata())
        .ok_or_else(|| Status::unauthenticated("authentication required"))?;

    verify_token(&token, secret)
        .map(|_| ())
        .map_err(|_| Status::unauthenticated("invalid or expired token"))
}
