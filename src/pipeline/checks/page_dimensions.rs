// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::{is_a4_size_mm, PdfInspection},
};

pub fn check(inspection: &PdfInspection) -> CheckResult {
    let non_a4_pages = inspection
        .pages
        .iter()
        .filter(|page| !is_a4_size_mm(page.size.w_mm, page.size.h_mm))
        .map(|page| page.page)
        .collect::<Vec<_>>();

    CheckResult::new(
        "page_dimensions",
        Severity::Warning,
        if non_a4_pages.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        json!({ "non_a4_pages": non_a4_pages }),
    )
}
