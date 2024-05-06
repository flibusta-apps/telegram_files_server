mod config;
mod core;

use std::{net::SocketAddr, str::FromStr};
use sentry::{integrations::debug_images::DebugImagesIntegration, types::Dsn, ClientOptions};
use sentry_tracing::EventFilter;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::core::views::get_router;


#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

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
        .with(filter::LevelFilter::WARN)
        .with(sentry_layer)
        .init();

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    let app = get_router().await;

    println!("Start webserver...");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    println!("Webserver shutdown...");
}
