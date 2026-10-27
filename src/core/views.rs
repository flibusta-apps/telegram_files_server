use axum::{
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{self, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use once_cell::sync::Lazy;
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;
use tokio_util::io::ReaderStream;
use tower_http::trace::{self, TraceLayer};
use tracing::{error, info, Level};

use crate::config::CONFIG;
use crate::core::errors::FileError;
use crate::core::file_utils::{download_file, upload_file, SpooledData};

/// Slack on top of the max file size for multipart framing and text fields.
const BODY_OVERHEAD: usize = 1024 * 1024;
const MAX_TEXT_FIELD_SIZE: usize = 64 * 1024;

static UPLOAD_SEMAPHORE: Lazy<Semaphore> =
    Lazy::new(|| Semaphore::new(CONFIG.max_concurrent_uploads));

async fn auth(req: Request<axum::body::Body>, next: Next) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let auth_header = if let Some(auth_header) = auth_header {
        auth_header
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !bool::from(auth_header.as_bytes().ct_eq(CONFIG.api_key.as_bytes())) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

pub async fn get_router() -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let app_router = Router::new()
        .route("/api/v1/files/upload/", post(upload))
        .route(
            "/api/v1/files/download_by_message/{chat_id}/{message_id}",
            get(download),
        )
        .layer(DefaultBodyLimit::max(
            CONFIG.max_upload_size + BODY_OVERHEAD,
        ))
        .layer(middleware::from_fn(auth))
        .layer(prometheus_layer);

    let metric_router =
        Router::new().route("/metrics", get(|| async move { metric_handle.render() }));

    let health_router = Router::new().route("/health", get(health));

    Router::new()
        .merge(app_router)
        .merge(metric_router)
        .merge(health_router)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
}

fn multipart_error(e: axum::extract::multipart::MultipartError) -> FileError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        FileError::TooLarge(format!("multipart error: {e}"))
    } else {
        FileError::BadRequest(format!("multipart error: {e}"))
    }
}

/// Reads a small text field, rejecting anything over `MAX_TEXT_FIELD_SIZE`.
async fn read_text_field(
    mut field: axum::extract::multipart::Field<'_>,
) -> Result<String, FileError> {
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
        if buf.len() + chunk.len() > MAX_TEXT_FIELD_SIZE {
            return Err(FileError::TooLarge(format!(
                "text field exceeds {MAX_TEXT_FIELD_SIZE} bytes"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf)
        .map_err(|_| FileError::BadRequest("text field is not valid UTF-8".to_string()))
}

async fn upload(mut multipart: Multipart) -> Result<impl IntoResponse, FileError> {
    let _permit = UPLOAD_SEMAPHORE
        .acquire()
        .await
        .map_err(|_| FileError::Unavailable("upload limiter closed".to_string()))?;

    let mut spooled: Option<SpooledData> = None;
    let mut filename: Option<String> = None;
    let mut caption: Option<String> = None;
    let mut file_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                // Only use Content-Disposition filename as fallback.
                // An explicit "filename" field always takes priority.
                if filename.is_none() {
                    filename = Some(field.file_name().unwrap_or("unknown").to_string());
                }
                spooled =
                    Some(SpooledData::from_multipart_field(field, CONFIG.max_upload_size).await?);
            }
            "filename" => filename = Some(read_text_field(field).await?),
            "caption" => caption = Some(read_text_field(field).await?),
            "file_type" => file_type = Some(read_text_field(field).await?),
            _ => {
                info!(name = %name, "ignoring unknown multipart field");
            }
        }
    }

    let spooled =
        spooled.ok_or_else(|| FileError::BadRequest("missing required field: file".to_string()))?;

    let chat_id = match file_type.as_deref() {
        Some("audiobook") => CONFIG.telegram_audio_chat_id,
        _ => CONFIG.telegram_chat_id,
    };

    let result = upload_file(
        spooled,
        filename.unwrap_or_else(|| "unknown".to_string()),
        caption,
        chat_id,
    )
    .await?;

    Ok(serde_json::to_string(&result).unwrap())
}

async fn health() -> impl IntoResponse {
    StatusCode::OK
}

async fn download(Path((chat_id, message_id)): Path<(i64, i32)>) -> Result<Response, FileError> {
    let (file, len) = match download_file(chat_id, message_id).await.map_err(|err| {
        error!(%chat_id, %message_id, error = %err, "Failed to download file");
        err
    })? {
        Some(file) => file,
        None => return Ok(StatusCode::NO_CONTENT.into_response()),
    };

    let reader = ReaderStream::new(file);
    Ok((
        [
            (
                http::header::CONTENT_TYPE,
                "application/octet-stream".to_string(),
            ),
            (http::header::CONTENT_LENGTH, len.to_string()),
        ],
        axum::body::Body::from_stream(reader),
    )
        .into_response())
}
