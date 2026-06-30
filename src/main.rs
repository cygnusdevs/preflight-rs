// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Context;
use preflight_rs::{app, config::Config, AppState};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr;
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("bind {bind_addr}"))?;

    axum::serve(listener, app(AppState::new(config)))
        .await
        .context("server")?;

    Ok(())
}
