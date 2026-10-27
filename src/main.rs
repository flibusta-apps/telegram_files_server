mod config;
mod core;

use core::file_utils::clean_files;
use sentry::{integrations::debug_images::DebugImagesIntegration, types::Dsn, ClientOptions};
use sentry_tracing::EventFilter;
use std::{net::SocketAddr, str::FromStr};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::core::views::get_router;

async fn start_app() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    let app = get_router().await;

    println!("Start webserver...");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    println!("Webserver shutdown...");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn cron_jobs() {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5 * 60));

    loop {
        interval.tick().await;

        match clean_files().await {
            Ok(stats) => tracing::info!(
                removed = stats.removed,
                errors = stats.errors,
                "clean_files pass finished"
            ),
            Err(err) => tracing::error!("clean_files pass failed: {err}"),
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let options = ClientOptions {
        dsn: Some(Dsn::from_str(&config::CONFIG.sentry_dsn).unwrap()),
        ..Default::default()
    }
    .add_integration(DebugImagesIntegration::new());

    let _guard = sentry::init(options);

    let sentry_layer = sentry_tracing::layer().event_filter(|md| match md.level() {
        &tracing::Level::ERROR => EventFilter::Event,
        _ => EventFilter::Ignore,
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(sentry_layer)
        .init();

    // Validate configuration up front (exits with a clear error if invalid).
    once_cell::sync::Lazy::force(&config::CONFIG);
    once_cell::sync::Lazy::force(&core::bot::ROUND_ROBIN_BOT);

    tokio::select! {
        _ = cron_jobs() => {},
        _ = start_app() => {},
    }
}
