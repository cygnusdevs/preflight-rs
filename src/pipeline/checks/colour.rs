// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    gs::InkCoverage,
    models::{CheckResult, CheckStatus, Severity},
};

pub fn check(coverage: Result<Vec<InkCoverage>, ()>, threshold: f64) -> CheckResult {
    match coverage {
        Ok(coverage) => {
            let colour_pages = coverage
                .iter()
                .filter(|page| page.c + page.m + page.y > threshold)
                .map(|page| page.page)
                .collect::<Vec<_>>();
            CheckResult::new(
                "colour",
                Severity::Info,
                CheckStatus::Pass,
                json!({
                    "has_colour": !colour_pages.is_empty(),
                    "colour_pages": colour_pages,
                    "threshold": threshold
                }),
            )
        }
        Err(()) => CheckResult::new(
            "colour",
            Severity::Error,
            CheckStatus::Fail,
            json!({
                "has_colour": false,
                "colour_pages": [],
                "threshold": threshold
            }),
        ),
    }
}
