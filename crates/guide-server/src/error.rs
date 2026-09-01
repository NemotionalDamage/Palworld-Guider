use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    InvalidJson,
    JsonDepthExceeded,
    InvalidRequest,
    InvalidQuestion,
    InvalidSnapshot,
    PayloadTooLarge,
    SessionNotFound,
    SessionExpired,
    SessionLimitReached,
    AskRateLimited,
    SnapshotRateLimited,
}

impl ApiErrorCode {
    fn message(self) -> &'static str {
        match self {
            Self::InvalidJson => "Request body is not valid JSON",
            Self::JsonDepthExceeded => "JSON nesting depth exceeds the limit",
            Self::InvalidRequest => "Request shape is invalid",
            Self::InvalidQuestion => "Question must contain 1 to 2000 characters",
            Self::InvalidSnapshot => "Snapshot validation failed",
            Self::PayloadTooLarge => "Request body exceeds 65536 bytes",
            Self::SessionNotFound => "Session does not exist",
            Self::SessionExpired => "Session expired",
            Self::SessionLimitReached => "Session limit reached",
            Self::AskRateLimited => "Ask rate limit reached",
            Self::SnapshotRateLimited => "Snapshot rate limit reached",
        }
    }

    fn status(self) -> StatusCode {
        match self {
            Self::InvalidJson
            | Self::JsonDepthExceeded
            | Self::InvalidRequest
            | Self::InvalidQuestion
            | Self::InvalidSnapshot => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::SessionNotFound => StatusCode::NOT_FOUND,
            Self::SessionExpired | Self::SessionLimitReached => StatusCode::GONE,
            Self::AskRateLimited | Self::SnapshotRateLimited => StatusCode::TOO_MANY_REQUESTS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiErrorBody {
    pub code: ApiErrorCode,
    pub message: &'static str,
}

pub fn api_error(code: ApiErrorCode) -> Response {
    (
        code.status(),
        Json(ApiErrorResponse {
            error: ApiErrorBody {
                code,
                message: code.message(),
            },
        }),
    )
        .into_response()
}
