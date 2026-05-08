use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use teloxide::RequestError;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// The requested file was not found or is temporarily unavailable.
    #[error("File not found or unavailable: {0}")]
    FileUnavailable(String),

    /// Rate-limited by Telegram API.
    #[error("Rate limited by Telegram: retry after {0}s")]
    RateLimited(u64),

    /// A Telegram API error that we don't handle specifically.
    #[error("Telegram API error: {0}")]
    TelegramApi(#[from] RequestError),

    /// An I/O error (e.g., file not found on disk).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for FileError {
    fn into_response(self) -> Response {
        match &self {
            FileError::FileUnavailable(_) => {
                (StatusCode::GONE, self.to_string()).into_response()
            }
            FileError::RateLimited(secs) => {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [("retry-after", secs.to_string())],
                    self.to_string(),
                )
                    .into_response()
            }
            FileError::TelegramApi(err) => {
                tracing::error!("Telegram API error: {err}");
                (StatusCode::BAD_GATEWAY, self.to_string()).into_response()
            }
            FileError::Io(err) => {
                tracing::error!("I/O error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
        }
    }
}