mod app;
mod db;
mod error;
mod models;
mod runtime;

use std::sync::Arc;
use std::time::Duration;

use app::{AppConfig, build_base_state, build_router, hydrate_route_map, reconcile_once};
use db::Db;
use runtime::ShellRuntimeManager;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    let db = Db::connect(&config.db_url).await?;
    let runtime = Arc::new(ShellRuntimeManager);
    let state = build_base_state(db, runtime, &config);
    hydrate_route_map(&state).await?;
    if let Err(err) = reconcile_once(&state).await {
        warn!(error = %err, "initial reconcile failed");
    }

    let reconcile_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(err) = reconcile_once(&reconcile_state).await {
                warn!(error = %err, "periodic reconcile failed");
            }
        }
    });

    let app = build_router(state).await;
    let listener = TcpListener::bind(&config.bind_addr).await?;

    info!(bind = %config.bind_addr, "backend server started");
    axum::serve(listener, app).await?;

    Ok(())
}
