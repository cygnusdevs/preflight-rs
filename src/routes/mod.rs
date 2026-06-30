// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod analyse;
mod multipart;
pub mod process;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

use crate::{gs, AppState};

pub async fn healthz() -> &'static str {
    "ok"
}

#[derive(Serialize)]
pub struct VersionInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub source_url: &'static str,
    pub license: &'static str,
    pub ghostscript_version: String,
}

pub async fn version(State(state): State<AppState>) -> Json<VersionInfo> {
    Json(VersionInfo {
        name: "preflight-rs",
        version: env!("CARGO_PKG_VERSION"),
        source_url: "https://github.com/cygnusdevs/preflight-rs",
        license: "AGPL-3.0-or-later",
        ghostscript_version: gs::ghostscript_version(&state.config.gs_bin).await,
    })
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest,
    PayloadTooLarge,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        }
        .into_response()
    }
}
