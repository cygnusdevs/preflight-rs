// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::PdfInspection,
};

pub fn check(inspection: &PdfInspection) -> CheckResult {
    CheckResult::new(
        "readable",
        Severity::Error,
        if inspection.readable {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        json!({ "readable": inspection.readable }),
    )
}
