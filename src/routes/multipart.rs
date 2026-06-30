// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::{
    extract::{multipart::MultipartError, Multipart},
    http::StatusCode,
};
use bytes::Bytes;

use crate::{config::AnalysisOptions, routes::ApiError, AppState};

pub struct Upload {
    pub file: Bytes,
    pub callback_url: Option<String>,
    pub callback_token: Option<String>,
    pub options: AnalysisOptions,
}

pub async fn parse_upload(
    state: &AppState,
    mut multipart: Multipart,
    require_callback: bool,
) -> Result<Upload, ApiError> {
    let mut file = None;
    let mut callback_url = None;
    let mut callback_token = None;
    let mut options = state.config.defaults.clone();

    while let Some(field) = multipart.next_field().await.map_err(map_multipart_error)? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "file" => {
                let bytes = field.bytes().await.map_err(map_multipart_error)?;
                if bytes.len() as u64 > state.config.max_upload_bytes {
                    return Err(ApiError::PayloadTooLarge);
                }
                file = Some(bytes);
            }
            "callback_url" => {
                callback_url = Some(field.text().await.map_err(map_multipart_error)?);
            }
            "callback_token" => {
                callback_token = Some(field.text().await.map_err(map_multipart_error)?);
            }
            "max_pages" => {
                options.max_pages = parse_field(field).await?;
            }
            "margin_mm" => {
                options.margin_mm = parse_field(field).await?;
            }
            "min_dpi" => {
                options.min_dpi = parse_field(field).await?;
            }
            "colour_threshold" => {
                options.colour_threshold = parse_field(field).await?;
            }
            "color_mode" => {
                options.color_mode = parse_field(field).await?;
            }
            _ => {}
        }
    }

    if require_callback && callback_url.as_deref().unwrap_or_default().is_empty() {
        return Err(ApiError::BadRequest);
    }

    Ok(Upload {
        file: file.ok_or(ApiError::BadRequest)?,
        callback_url,
        callback_token,
        options,
    })
}

async fn parse_field<T>(field: axum::extract::multipart::Field<'_>) -> Result<T, ApiError>
where
    T: std::str::FromStr,
{
    field
        .text()
        .await
        .map_err(map_multipart_error)?
        .parse()
        .map_err(|_| ApiError::BadRequest)
}

fn map_multipart_error(error: MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::PayloadTooLarge
    } else {
        ApiError::BadRequest
    }
}
