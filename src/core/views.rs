use axum::{
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{self, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use futures::StreamExt;
use once_cell::sync::Lazy;
use std::time::Duration;
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

    let auth_header = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);
    if !bool::from(auth_header.as_bytes().ct_eq(CONFIG.api_key.as_bytes())) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

pub async fn get_router() -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let app_router = Router::new()
        .route(
            "/api/v1/files/upload/",
            post(upload).layer(middleware::from_fn(upload_body_timeout)),
        )
        .route(
            "/api/v1/files/download_by_message/{chat_id}/{message_id}",
            get(download),
        )
        .layer(DefaultBodyLimit::max(
            CONFIG.max_upload_size + BODY_OVERHEAD,
        ))
        .layer(middleware::from_fn(auth))
        .layer(prometheus_layer);

    let metric_router = Router::new()
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(middleware::from_fn(auth));

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

#[derive(Debug, thiserror::Error)]
#[error("upload body idle timeout")]
struct UploadIdleTimeout;

async fn upload_body_timeout(req: Request<axum::body::Body>, next: Next) -> Response {
    let (parts, body) = req.into_parts();
    let body = idle_timeout_body(
        body,
        Duration::from_secs(CONFIG.upload_body_idle_timeout_secs),
    );
    next.run(Request::from_parts(parts, body)).await
}

fn idle_timeout_body(body: axum::body::Body, idle: Duration) -> axum::body::Body {
    let stream = futures::stream::unfold(Some(body.into_data_stream()), move |state| async move {
        let mut stream = state?;
        let next = tokio::time::timeout(idle, async {
            loop {
                match stream.next().await {
                    Some(Ok(bytes)) if bytes.is_empty() => continue,
                    item => break item,
                }
            }
        })
        .await;
        match next {
            Ok(Some(item)) => Some((item, Some(stream))),
            Ok(None) => None,
            Err(_) => Some((Err(axum::Error::new(UploadIdleTimeout)), None)),
        }
    });
    axum::body::Body::from_stream(stream)
}

pub(super) fn multipart_error(e: axum::extract::multipart::MultipartError) -> FileError {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&e);
    while let Some(err) = source {
        if err.is::<UploadIdleTimeout>() {
            return FileError::RequestTimeout("upload body idle timeout".into());
        }
        source = err.source();
    }
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
    let _permit = tokio::time::timeout(
        Duration::from_secs(CONFIG.upload_permit_timeout_secs),
        UPLOAD_SEMAPHORE.acquire(),
    )
    .await
    .map_err(|_| FileError::Unavailable("upload permit wait timed out".into()))?
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, extract::FromRequest};
    use bytes::Bytes;

    async fn stalled_multipart(prefix: &'static str) -> Multipart {
        let stream =
            futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(prefix)) })
                .chain(futures::stream::pending());
        let req = Request::builder()
            .header("content-type", "multipart/form-data; boundary=test")
            .body(idle_timeout_body(
                Body::from_stream(stream),
                Duration::from_millis(10),
            ))
            .unwrap();
        Multipart::from_request(req, &()).await.unwrap()
    }

    #[tokio::test]
    async fn idle_timeout_while_waiting_for_boundary() {
        let mut multipart = stalled_multipart("").await;
        let err = multipart.next_field().await.unwrap_err();
        assert!(matches!(multipart_error(err), FileError::RequestTimeout(_)));
    }

    #[tokio::test]
    async fn idle_timeout_while_reading_file() {
        let mut multipart = stalled_multipart(
            "--test\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x\"\r\n\r\ndata",
        )
        .await;
        let field = multipart.next_field().await.unwrap().unwrap();
        assert!(matches!(
            SpooledData::from_multipart_field(field, 1024).await,
            Err(FileError::RequestTimeout(_))
        ));
    }

    #[tokio::test]
    async fn idle_timeout_while_reading_text() {
        let mut multipart = stalled_multipart(
            "--test\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\ndata",
        )
        .await;
        let field = multipart.next_field().await.unwrap().unwrap();
        assert!(matches!(
            read_text_field(field).await,
            Err(FileError::RequestTimeout(_))
        ));
    }
}
