// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod auth;
pub mod callback;
pub mod config;
pub mod gs;
pub mod models;
pub mod pdf;
pub mod pipeline;
pub mod routes;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};
use tokio::sync::Semaphore;

use crate::{callback::CallbackClient, config::Config};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub gs_permits: Arc<Semaphore>,
    pub callbacks: CallbackClient,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let gs_permits = Arc::new(Semaphore::new(config.gs_concurrency));
        Self {
            config: Arc::new(config),
            gs_permits,
            callbacks: CallbackClient::new(),
        }
    }
}

pub fn app(state: AppState) -> Router {
    let max_upload_bytes = state.config.max_upload_bytes.min(usize::MAX as u64) as usize;

    Router::new()
        .route("/healthz", get(routes::healthz))
        .route("/version", get(routes::version))
        .nest(
            "/pdf",
            Router::new()
                .route("/analyse", post(routes::analyse::analyse))
                .route("/process", post(routes::process::process)),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_bearer,
        ))
        .layer(DefaultBodyLimit::max(max_upload_bytes))
        .with_state(state)
}
