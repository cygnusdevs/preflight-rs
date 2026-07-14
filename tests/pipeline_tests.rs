// SPDX-License-Identifier: AGPL-3.0-or-later

use preflight_rs::{
    config::ColorMode,
    models::{
        AnalysisInfo, CheckResult, CheckStatus, FileInfo, PreflightResult, ResultStatus, Severity,
    },
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn result_summary_counts_errors_warnings_pages_and_colour() {
    let mut result = result();
    result.push_check(CheckResult::new(
        "page_count",
        Severity::Info,
        CheckStatus::Pass,
        json!({ "count": 3 }),
    ));
    result.push_check(CheckResult::new(
        "colour",
        Severity::Info,
        CheckStatus::Pass,
        json!({ "has_colour": true, "colour_pages": [2] }),
    ));
    result.push_check(CheckResult::new(
        "margins",
        Severity::Warning,
        CheckStatus::Warn,
        json!({ "tight_pages": [1] }),
    ));
    result.push_check(CheckResult::new(
        "pdf_valid",
        Severity::Error,
        CheckStatus::Fail,
        json!({ "magic": false, "mupdf_open": false }),
    ));
    result.finalize();

    assert_eq!(result.status, ResultStatus::Failed);
    assert_eq!(result.summary.pages, 3);
    assert!(result.summary.has_colour);
    assert_eq!(result.summary.errors, 1);
    assert_eq!(result.summary.warnings, 1);
}

#[test]
fn result_serializes_page_facts_separately_from_checks() {
    let mut result = result();
    result.pages.push(preflight_rs::models::PageResult {
        page: 1,
        size: preflight_rs::models::PageSizeResult {
            w_mm: 210.0,
            h_mm: 297.0,
            is_a4: true,
        },
        margins: preflight_rs::models::PageMarginResult { tight: false },
        colour: preflight_rs::models::PageColourResult {
            has_colour: true,
            coverage: Some(preflight_rs::models::PageInkCoverage {
                c: 0.1,
                m: 0.0,
                y: 0.0,
                k: 0.2,
            }),
        },
        blank: false,
        images: vec![preflight_rs::models::PageImageResult {
            pixel_width: 300,
            pixel_height: 300,
            placed: preflight_rs::pdf::PageSizeMm {
                w_mm: 50.8,
                h_mm: 25.4,
            },
            dpi: 150.0,
            low_res: false,
        }],
    });
    result.push_check(CheckResult::new(
        "page_dimensions",
        Severity::Warning,
        CheckStatus::Pass,
        json!({ "non_a4_pages": [] }),
    ));

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["pages"][0]["page"], 1);
    assert_eq!(json["pages"][0]["size"]["is_a4"], true);
    assert_eq!(json["pages"][0]["colour"]["has_colour"], true);
    assert_eq!(json["pages"][0]["images"][0]["low_res"], false);
    assert!(json["checks"][0]["data"].get("sizes").is_none());
    assert!(json.get("file").is_none());
    assert_eq!(json["analysis"]["color_mode"], "color");
    assert_eq!(json["source_file"], json["analysed_file"]);
}

#[test]
fn colour_check_failure_is_a_hard_failure() {
    let check = preflight_rs::pipeline::checks::colour::check(Err(()), 0.01);

    assert_eq!(check.severity, Severity::Error);
    assert_eq!(check.status, CheckStatus::Fail);
}

#[test]
fn encrypted_check_only_rejects_encryption_or_disabled_printing() {
    let printable = preflight_rs::pdf::PdfInspection {
        readable: true,
        encrypted: false,
        printing_disallowed: false,
        page_count: Some(1),
        pages: Vec::new(),
    };
    let mut blocked = printable.clone();
    blocked.printing_disallowed = true;

    assert_eq!(
        preflight_rs::pipeline::checks::encrypted::check(&printable).status,
        CheckStatus::Pass
    );
    assert_eq!(
        preflight_rs::pipeline::checks::encrypted::check(&blocked).status,
        CheckStatus::Fail
    );
}

fn result() -> PreflightResult {
    let file = FileInfo {
        bytes: 10,
        sha256: "abc".to_owned(),
        pdf_version: Some("1.7".to_owned()),
    };

    PreflightResult::new(
        Uuid::nil(),
        file.clone(),
        file,
        AnalysisInfo {
            color_mode: ColorMode::Color,
            converted_to_grayscale: false,
            fit_to_page: false,
            fitted_to_page: false,
            max_pages: 500,
            margin_mm: 5.0,
            min_dpi: 150.0,
            colour_threshold: 0.01,
        },
    )
}
