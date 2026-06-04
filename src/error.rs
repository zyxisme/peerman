use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

impl From<AppError> for tonic::Status {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(msg) => tonic::Status::not_found(msg),
            AppError::Validation(msg) => tonic::Status::invalid_argument(msg),
            AppError::Database(e) => tonic::Status::internal(format!("Database error: {e}")),
            AppError::Internal(msg) => tonic::Status::internal(msg),
            AppError::Unauthorized(msg) => tonic::Status::unauthenticated(msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Database(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
        };
        (status, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unauthorized_to_status() {
        let err = AppError::Unauthorized("test".into());
        let status: tonic::Status = err.into();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }
}
