// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::json;

use crate::{
    models::{CheckResult, CheckStatus, Severity},
    pdf::{image_dpi, PdfInspection},
};

pub fn check(inspection: &PdfInspection, min_dpi: f64) -> CheckResult {
    let mut low_res_pages = inspection
        .pages
        .iter()
        .flat_map(|page| page.images.iter())
        .filter_map(|image| {
            let dpi = image_dpi(image);
            (dpi < min_dpi).then_some(image.page)
        })
        .collect::<Vec<_>>();
    low_res_pages.sort_unstable();
    low_res_pages.dedup();

    CheckResult::new(
        "image_resolution",
        Severity::Warning,
        if low_res_pages.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        json!({
            "low_res_pages": low_res_pages,
            "min_dpi": min_dpi
        }),
    )
}
