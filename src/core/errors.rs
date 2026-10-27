use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use teloxide::RequestError;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// Telegram definitively says the message does not exist (HTTP 410).
    #[error("Message not found: {0}")]
    NotFound(String),

    /// An upstream call timed out (HTTP 504).
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Telegram / network is temporarily unavailable (HTTP 503).
    #[error("Temporarily unavailable: {0}")]
    Unavailable(String),

    /// Malformed request, e.g. multipart errors or a missing field (HTTP 400).
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Payload exceeds configured limits (HTTP 413).
    #[error("Payload too large: {0}")]
    TooLarge(String),

    /// Rate-limited by Telegram API (HTTP 429).
    #[error("Rate limited by Telegram: retry after {0}s")]
    RateLimited(u64),

    /// A Telegram API error that we don't handle specifically.
    #[error("Telegram API error: {0}")]
    TelegramApi(RequestError),

    /// An I/O error (e.g., file not found on disk).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A system time error (e.g., from file metadata).
    #[error("System time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
}

/// Returns true only if Telegram definitively says the message doesn't exist.
pub fn is_not_found_error(err: &RequestError) -> bool {
    match err {
        RequestError::Api(api_err) => match api_err {
            teloxide::ApiError::MessageToForwardNotFound | teloxide::ApiError::MessageIdInvalid => {
                true
            }
            teloxide::ApiError::Unknown(msg) => {
                let msg = msg.to_lowercase();
                msg.contains("message to forward not found")
                    || msg.contains("message not found")
                    || msg.contains("message_id_invalid")
            }
            _ => false,
        },
        _ => false,
    }
}

impl From<RequestError> for FileError {
    fn from(err: RequestError) -> Self {
        if is_not_found_error(&err) {
            return FileError::NotFound(err.to_string());
        }
        match &err {
            RequestError::RetryAfter(secs) => FileError::RateLimited(secs.seconds() as u64),
            RequestError::Network(e) if e.is_timeout() => FileError::Timeout(err.to_string()),
            RequestError::Network(_) => FileError::Unavailable(err.to_string()),
            RequestError::Api(teloxide::ApiError::Unknown(msg))
                if msg.contains("wrong file_id") || msg.contains("temporarily unavailable") =>
            {
                FileError::Unavailable(err.to_string())
            }
            RequestError::Api(
                teloxide::ApiError::WrongFileId
                | teloxide::ApiError::WrongFileIdOrUrl
                | teloxide::ApiError::FileIdInvalid,
            ) => FileError::Unavailable(err.to_string()),
            _ => FileError::TelegramApi(err),
        }
    }
}

impl IntoResponse for FileError {
    fn into_response(self) -> Response {
        match &self {
            FileError::NotFound(_) => (StatusCode::GONE, self.to_string()).into_response(),
            FileError::Timeout(_) => {
                (StatusCode::GATEWAY_TIMEOUT, self.to_string()).into_response()
            }
            FileError::Unavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string()).into_response()
            }
            FileError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()).into_response(),
            FileError::TooLarge(_) => {
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string()).into_response()
            }
            FileError::RateLimited(secs) => (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", secs.to_string())],
                Json(json!({
                    "detail": self.to_string(),
                    "retry_after_secs": secs,
                })),
            )
                .into_response(),
            FileError::TelegramApi(err) => {
                tracing::error!("Telegram API error: {err}");
                (StatusCode::BAD_GATEWAY, self.to_string()).into_response()
            }
            FileError::Io(err) => {
                tracing::error!("I/O error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
            FileError::SystemTime(err) => {
                tracing::error!("System time error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::ApiError;

    fn status(err: FileError) -> StatusCode {
        err.into_response().status()
    }

    #[test]
    fn status_mapping() {
        assert_eq!(status(FileError::NotFound("x".into())), StatusCode::GONE);
        assert_eq!(
            status(FileError::Timeout("x".into())),
            StatusCode::GATEWAY_TIMEOUT
        );
        assert_eq!(
            status(FileError::Unavailable("x".into())),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            status(FileError::BadRequest("x".into())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status(FileError::TooLarge("x".into())),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            status(FileError::RateLimited(10)),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            status(FileError::TelegramApi(RequestError::Api(
                ApiError::BotBlocked
            ))),
            StatusCode::BAD_GATEWAY
        );
    }

    #[tokio::test]
    async fn rate_limited_has_header_and_json() {
        let resp = FileError::RateLimited(42).into_response();
        assert_eq!(resp.headers().get("retry-after").unwrap(), "42");
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["retry_after_secs"], 42);
        assert!(v["detail"].is_string());
    }

    #[test]
    fn not_found_classification() {
        assert!(is_not_found_error(&RequestError::Api(
            ApiError::MessageToForwardNotFound
        )));
        assert!(is_not_found_error(&RequestError::Api(
            ApiError::MessageIdInvalid
        )));
        assert!(is_not_found_error(&RequestError::Api(ApiError::Unknown(
            "Bad Request: message not found".into()
        ))));
        // Ambiguous / transient errors must not be classified as not found.
        assert!(!is_not_found_error(&RequestError::Api(ApiError::Unknown(
            "Bad Request: wrong file_id or the file is temporarily unavailable".into()
        ))));
        assert!(!is_not_found_error(&RequestError::Api(
            ApiError::BotBlocked
        )));
        assert!(!is_not_found_error(&RequestError::RetryAfter(
            teloxide::types::Seconds::from_seconds(3)
        )));
    }

    #[test]
    fn request_error_conversion() {
        assert!(matches!(
            FileError::from(RequestError::Api(ApiError::MessageIdInvalid)),
            FileError::NotFound(_)
        ));
        assert!(matches!(
            FileError::from(RequestError::RetryAfter(
                teloxide::types::Seconds::from_seconds(7)
            )),
            FileError::RateLimited(7)
        ));
        assert!(matches!(
            FileError::from(RequestError::Api(ApiError::Unknown(
                "wrong file_id or the file is temporarily unavailable".into()
            ))),
            FileError::Unavailable(_)
        ));
    }
}
