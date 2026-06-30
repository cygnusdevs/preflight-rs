// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::{is_tight_to_edge, PdfInspection, RectMm},
};

pub fn check(inspection: &PdfInspection, threshold_mm: f64) -> CheckResult {
    let tight_pages = inspection
        .pages
        .iter()
        .filter(|page| {
            page.content_bbox
                .map(|bbox| {
                    is_tight_to_edge(
                        RectMm::new(0.0, 0.0, page.size.w_mm, page.size.h_mm),
                        bbox,
                        threshold_mm,
                    )
                })
                .unwrap_or(false)
        })
        .map(|page| page.page)
        .collect::<Vec<_>>();

    CheckResult::new(
        "margins",
        Severity::Warning,
        if tight_pages.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        json!({ "tight_pages": tight_pages, "threshold_mm": threshold_mm }),
    )
}
