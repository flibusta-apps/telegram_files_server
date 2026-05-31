use axum::{
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{self, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use tokio_util::io::ReaderStream;
use tower_http::trace::{self, TraceLayer};
use tracing::{error, info, Level};

use crate::config::CONFIG;
use crate::core::errors::FileError;
use crate::core::file_utils::{download_file, upload_file, SpooledData};

const BODY_LIMIT: usize = 4 * (2 << 30); // bytes: 4GB

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

    if auth_header != CONFIG.api_key {
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
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
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

async fn upload(mut multipart: Multipart) -> Result<impl IntoResponse, FileError> {
    let mut spooled: Option<SpooledData> = None;
    let mut filename: Option<String> = None;
    let mut caption: Option<String> = None;
    let mut file_type: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| FileError::FileUnavailable(format!("multipart error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                // Only use Content-Disposition filename as fallback.
                // An explicit "filename" field always takes priority.
                if filename.is_none() {
                    filename = Some(field.file_name().unwrap_or("unknown").to_string());
                }
                spooled = Some(
                    SpooledData::from_multipart_field(field)
                        .await
                        .map_err(|e| {
                            FileError::FileUnavailable(format!("failed to read file: {e}"))
                        })?,
                );
            }
            "filename" => {
                filename =
                    Some(field.text().await.map_err(|e| {
                        FileError::FileUnavailable(format!("multipart error: {e}"))
                    })?);
            }
            "caption" => {
                caption =
                    Some(field.text().await.map_err(|e| {
                        FileError::FileUnavailable(format!("multipart error: {e}"))
                    })?);
            }
            "file_type" => {
                file_type =
                    Some(field.text().await.map_err(|e| {
                        FileError::FileUnavailable(format!("multipart error: {e}"))
                    })?);
            }
            _ => {
                info!(name = %name, "ignoring unknown multipart field");
            }
        }
    }

    let spooled = spooled
        .ok_or_else(|| FileError::FileUnavailable("missing required field: file".to_string()))?;

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
    let file = match download_file(chat_id, message_id).await.map_err(|err| {
        error!(%chat_id, %message_id, error = %err, "Failed to download file");
        err
    })? {
        Some(file) => file,
        None => return Ok(StatusCode::NO_CONTENT.into_response()),
    };

    let reader = ReaderStream::new(file);
    Ok(axum::body::Body::from_stream(reader).into_response())
}
