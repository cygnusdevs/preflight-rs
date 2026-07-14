// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::{
    extract::{Multipart, State},
    Json,
};
use uuid::Uuid;

use crate::{
    pipeline,
    routes::{multipart::parse_upload, ApiError},
    AppState,
};

pub async fn analyse(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<crate::models::PreflightResult>, ApiError> {
    let upload = parse_upload(&state, multipart, false).await?;
    let result = pipeline::run(&state, Uuid::new_v4(), upload.file, upload.options, None).await;
    Ok(Json(result.result))
}
