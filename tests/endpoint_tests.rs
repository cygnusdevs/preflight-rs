// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::body::{to_bytes, Body};
use http::{Request, StatusCode};
use preflight_rs::{app, config::Config, AppState};
use serde_json::Value;
use tower::ServiceExt;

const NORMAL_PDF: &[u8] = include_bytes!("fixtures/normal_text.pdf");
const ENCRYPTED_PDF: &[u8] = include_bytes!("fixtures/encrypted.pdf");
const COLOUR_PDF: &[u8] = include_bytes!("fixtures/colour.pdf");

#[tokio::test]
async fn healthz_rejects_missing_auth() {
    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn healthz_returns_ok_with_auth() {
    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn version_returns_service_and_dependency_versions() {
    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .uri("/version")
                .header("authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "preflight-rs");
    assert!(!json["version"].as_str().unwrap().is_empty());
    assert_eq!(
        json["source_url"],
        "https://github.com/cygnusdevs/preflight-rs"
    );
    assert_eq!(json["license"], "AGPL-3.0-or-later");
    assert!(json.get("mupdf_version").is_none());
    assert!(json["ghostscript_version"].as_str().is_some());
}

#[tokio::test]
async fn analyse_requires_pdf_file_part() {
    let boundary = "X-BOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"max_pages\"\r\n\r\n5\r\n--{boundary}--\r\n"
    );

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn analyse_accepts_pdf_fixture_and_returns_result() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(boundary, NORMAL_PDF, &[("max_pages", "5")]);

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["schema_version"], "3.0");
    assert_eq!(json["status"], "completed");
    assert_eq!(json["summary"]["pages"], 1);
    assert!(json.get("file").is_none());
    assert_eq!(json["source_file"], json["analysed_file"]);
    assert_eq!(json["analysis"]["color_mode"], "color");
    assert_eq!(json["analysis"]["converted_to_grayscale"], false);
    assert_eq!(json["analysis"]["max_pages"], 5);
    assert_eq!(json["checks"][0]["id"], "pdf_valid");
    assert!(json["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|check| check["id"] != "page_count_max"));
    assert_eq!(json["pages"].as_array().unwrap().len(), 1);
    assert_eq!(json["pages"][0]["page"], 1);
    assert!(json["pages"][0]["size"]["is_a4"].as_bool().is_some());
    assert!(json["pages"][0]["colour"]["has_colour"].as_bool().is_some());
    assert!(json["pages"][0]["blank"].as_bool().is_some());
    assert!(json["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "page_dimensions")
        .unwrap()["data"]
        .get("sizes")
        .is_none());
}

#[tokio::test]
async fn analyse_rejects_upload_over_configured_limit() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(boundary, NORMAL_PDF, &[]);
    let mut config = Config::for_tests("secret");
    config.max_upload_bytes = 128;

    let response = app(AppState::new(config))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn analyse_short_circuits_encrypted_pdf_fixture() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(boundary, ENCRYPTED_PDF, &[]);

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "failed");
    assert_eq!(json["checks"][2]["id"], "encrypted");
    assert_eq!(json["checks"][2]["status"], "fail");
}

#[tokio::test]
async fn analyse_converts_to_mono_when_requested() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(boundary, COLOUR_PDF, &[("color_mode", "mono")]);

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "completed");
    assert_eq!(json["analysis"]["color_mode"], "mono");
    assert_eq!(json["analysis"]["converted_to_grayscale"], true);
    assert_ne!(
        json["source_file"]["sha256"],
        json["analysed_file"]["sha256"]
    );
    assert_eq!(json["summary"]["has_colour"], false);
    assert_eq!(
        json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == "colour")
            .unwrap()["data"]["colour_pages"],
        serde_json::json!([])
    );

    for page in json["pages"].as_array().unwrap() {
        let coverage = &page["colour"]["coverage"];
        assert_eq!(page["colour"]["has_colour"], false);
        assert_eq!(coverage["c"], 0.0);
        assert_eq!(coverage["m"], 0.0);
        assert_eq!(coverage["y"], 0.0);
    }
}

#[tokio::test]
async fn prepare_returns_analysis_and_fitted_pdf() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(
        boundary,
        COLOUR_PDF,
        &[
            ("fit_to_page", "true"),
            ("margin_mm", "5"),
            ("color_mode", "mono"),
        ],
    );

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/prepare")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(content_type.starts_with("multipart/mixed; boundary="));

    let response_boundary = content_type.split("boundary=").nth(1).unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = mixed_part(&body, response_boundary, "application/json");
    let pdf = mixed_part(&body, response_boundary, "application/pdf");
    let result: Value = serde_json::from_slice(json).unwrap();
    let inspection = preflight_rs::pdf::inspect_pdf(pdf).unwrap();

    assert_eq!(result["analysis"]["fit_to_page"], true);
    assert_eq!(result["analysis"]["fitted_to_page"], true);
    assert_eq!(result["analysis"]["converted_to_grayscale"], true);
    assert_eq!(result["summary"]["has_colour"], false);
    assert!(pdf.starts_with(b"%PDF"));
    assert!(inspection.pages.iter().all(|page| page.size.w_mm > 208.0
        && page.size.w_mm < 212.0
        && page.size.h_mm > 295.0
        && page.size.h_mm < 299.0));
}

#[tokio::test]
async fn prepare_enforces_page_limit_before_conversion() {
    let boundary = "X-BOUNDARY";
    let pdf = two_page_pdf();
    let body = multipart_body(
        boundary,
        &pdf,
        &[
            ("max_pages", "1"),
            ("fit_to_page", "true"),
            ("color_mode", "mono"),
        ],
    );
    let mut config = Config::for_tests("secret");
    config.gs_bin = "/missing/ghostscript".to_owned();

    let response = app(AppState::new(config))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/prepare")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let response_boundary = content_type.split("boundary=").nth(1).unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value =
        serde_json::from_slice(mixed_part(&body, response_boundary, "application/json")).unwrap();
    let failed_check = result["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["status"] == "fail")
        .unwrap();

    assert_eq!(failed_check["id"], "page_count");
    assert_eq!(result["analysis"]["fitted_to_page"], false);
    assert_eq!(result["analysis"]["converted_to_grayscale"], false);
    assert!(!body
        .windows(b"Content-Type: application/pdf".len())
        .any(|window| window == b"Content-Type: application/pdf"));
}

#[tokio::test]
async fn prepare_rejects_margin_that_leaves_no_printable_area() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(
        boundary,
        NORMAL_PDF,
        &[("fit_to_page", "true"), ("margin_mm", "105")],
    );

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/prepare")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn process_requires_callback_url() {
    let boundary = "X-BOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.7\n%%EOF\r\n--{boundary}--\r\n"
    );

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/process")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn process_accepts_pdf_fixture_and_returns_job_id() {
    let boundary = "X-BOUNDARY";
    let body = multipart_body(
        boundary,
        NORMAL_PDF,
        &[("callback_url", "http://127.0.0.1:9/callback")],
    );

    let response = app(AppState::new(Config::for_tests("secret")))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/process")
                .header("authorization", "Bearer secret")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["job_id"].as_str().unwrap().len() > 10);
}

fn multipart_body(boundary: &str, file: &[u8], fields: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"fixture.pdf\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/pdf\r\n\r\n");
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

fn mixed_part<'a>(body: &'a [u8], boundary: &str, content_type: &str) -> &'a [u8] {
    let header = format!("Content-Type: {content_type}\r\n");
    let header_start = body
        .windows(header.len())
        .position(|window| window == header.as_bytes())
        .unwrap();
    let content_start = body[header_start..]
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + header_start
        + 4;
    let delimiter = format!("\r\n--{boundary}");
    let content_end = body[content_start..]
        .windows(delimiter.len())
        .position(|window| window == delimiter.as_bytes())
        .unwrap()
        + content_start;

    &body[content_start..content_end]
}

fn two_page_pdf() -> Vec<u8> {
    let mut document = mupdf::pdf::PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    document.duplicate_page(0).unwrap();
    let mut output = Vec::new();
    document.write_to(&mut output).unwrap();
    output
}
