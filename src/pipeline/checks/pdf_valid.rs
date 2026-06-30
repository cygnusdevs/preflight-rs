// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::models::{CheckResult, CheckStatus, Severity};

pub fn check(magic: bool, mupdf_open: bool) -> CheckResult {
    let status = if magic && mupdf_open {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail
    };

    CheckResult::new(
        "pdf_valid",
        Severity::Error,
        status,
        json!({ "magic": magic, "mupdf_open": mupdf_open }),
    )
}
