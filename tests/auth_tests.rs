// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::body::Body;
use http::{Request, StatusCode};
use preflight_rs::{app, config::Config, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn pdf_routes_reject_missing_bearer_token() {
    let state = AppState::new(Config::for_tests("secret"));
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pdf_routes_reject_invalid_bearer_token() {
    let state = AppState::new(Config::for_tests("secret"));
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/pdf/analyse")
                .header("authorization", "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
