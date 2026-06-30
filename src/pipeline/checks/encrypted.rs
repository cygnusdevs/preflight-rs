// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::PdfInspection,
};

pub fn check(inspection: &PdfInspection) -> CheckResult {
    let encrypted = inspection.encrypted || inspection.restrictive_permissions;
    CheckResult::new(
        "encrypted",
        Severity::Error,
        if encrypted {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        json!({ "encrypted": encrypted }),
    )
}
