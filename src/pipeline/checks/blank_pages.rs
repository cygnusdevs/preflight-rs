// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::{is_blank_page, PdfInspection},
};

pub fn check(inspection: &PdfInspection) -> CheckResult {
    let blank_pages = inspection
        .pages
        .iter()
        .filter(|page| is_blank_page(&page.content))
        .map(|page| page.page)
        .collect::<Vec<_>>();

    CheckResult::new(
        "blank_pages",
        Severity::Warning,
        if blank_pages.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        json!({ "blank_pages": blank_pages }),
    )
}
