use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::{self, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use axum_typed_multipart::{TryFromMultipart, TypedMultipart};
use tokio_util::io::ReaderStream;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;
use tracing::log;

use crate::config::CONFIG;

use super::file_utils::{download_file, upload_file};

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
        .route("/upload/", post(upload))
        .route("/download_by_message/{chat_id}/{message_id}", get(download))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn(auth))
        .layer(prometheus_layer);

    let metric_router =
        Router::new().route("/metrics", get(|| async move { metric_handle.render() }));

    Router::new()
        .merge(app_router)
        .merge(metric_router)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
}


#[derive(TryFromMultipart)]
pub struct UploadFileRequest {
    #[form_data(limit = "unlimited")]
    file: Bytes,
    filename: String,
    caption: Option<String>,
}


async fn upload(data: TypedMultipart<UploadFileRequest>) -> impl IntoResponse {
    let result = match upload_file(
        data.file.clone(),
        data.filename.to_string(),
        data.caption.clone(),
    )
    .await
    {
        Ok(file) => serde_json::to_string(&file),
        Err(err) => Ok(err),
    };

    result.unwrap()
}

async fn download(Path((chat_id, message_id)): Path<(i64, i32)>) -> impl IntoResponse {
    let file = match download_file(chat_id, message_id).await {
        Ok(v) => {
            match v {
                Some(v) => v,
                None => return StatusCode::NO_CONTENT.into_response(),
            }
        },
        Err(err) => {
            log::error!("{}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

    let reader = ReaderStream::new(file);

    axum::body::Body::from_stream(reader).into_response()
}
