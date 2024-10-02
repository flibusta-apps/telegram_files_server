mod config;
mod core;

use core::file_utils::clean_files;
use std::{net::SocketAddr, str::FromStr};
use sentry::{integrations::debug_images::DebugImagesIntegration, types::Dsn, ClientOptions};
use sentry_tracing::EventFilter;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::core::views::get_router;


async fn start_app() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    let app = get_router().await;

    println!("Start webserver...");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    println!("Webserver shutdown...");
}


async fn cron_jobs() {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5 * 60));

    loop {
        interval.tick().await;

        let _ = clean_files().await;
    }
}


#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let options = ClientOptions {
        dsn: Some(Dsn::from_str(&config::CONFIG.sentry_dsn).unwrap()),
        default_integrations: false,
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
        .with(filter::LevelFilter::INFO)
        .with(sentry_layer)
        .init();

    tokio::join![cron_jobs(), start_app()];
}
