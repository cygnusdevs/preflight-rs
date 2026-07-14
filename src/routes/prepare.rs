// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::{
    body::Body,
    extract::{Multipart, State},
    http::{header, Response},
};
use uuid::Uuid;

use crate::{
    pipeline,
    routes::{multipart::parse_upload, ApiError},
    AppState,
};

pub async fn prepare(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Response<Body>, ApiError> {
    let upload = parse_upload(&state, multipart, false).await?;
    let output = pipeline::run(&state, Uuid::new_v4(), upload.file, upload.options, None).await;
    let boundary = Uuid::new_v4().simple().to_string();
    let json = serde_json::to_vec(&output.result).map_err(|_| ApiError::BadRequest)?;
    let mut body =
        Vec::with_capacity(json.len() + output.pdf.as_ref().map_or(0, bytes::Bytes::len) + 512);

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
    body.extend_from_slice(&json);
    body.extend_from_slice(b"\r\n");

    if let Some(pdf) = output.pdf {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/pdf\r\n");
        body.extend_from_slice(
            b"Content-Disposition: attachment; filename=\"prepared.pdf\"\r\n\r\n",
        );
        body.extend_from_slice(&pdf);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    Response::builder()
        .header(
            header::CONTENT_TYPE,
            format!("multipart/mixed; boundary={boundary}"),
        )
        .body(Body::from(body))
        .map_err(|_| ApiError::BadRequest)
}
