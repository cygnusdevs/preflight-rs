// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    models::CallbackTarget,
    pipeline,
    routes::{multipart::parse_upload, ApiError},
    AppState,
};

pub async fn process(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let upload = parse_upload(&state, multipart, true).await?;
    let job_id = Uuid::new_v4();
    let target = CallbackTarget {
        url: upload.callback_url.ok_or(ApiError::BadRequest)?,
        token: upload.callback_token,
    };
    let state_for_job = state.clone();

    tokio::spawn(async move {
        pipeline::run(
            &state_for_job,
            job_id,
            upload.file,
            upload.options,
            Some(target),
        )
        .await;
    });

    Ok((StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))))
}
