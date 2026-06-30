// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::PdfInspection,
};

pub fn check(inspection: &PdfInspection, max_pages: u32) -> CheckResult {
    let count = inspection.page_count.unwrap_or(0);
    let failed = count == 0 || count > max_pages;
    CheckResult::new(
        "page_count",
        Severity::Error,
        if failed {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        json!({ "count": count, "max": max_pages, "within_max": count <= max_pages }),
    )
}
