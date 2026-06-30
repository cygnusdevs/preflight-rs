// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod checks;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    config::{AnalysisOptions, ColorMode},
    gs::InkCoverage,
    models::{
        AnalysisInfo, CallbackEvent, CallbackTarget, CheckResult, FileInfo, PageColourResult,
        PageImageResult, PageInkCoverage, PageMarginResult, PageResult, PageSizeResult,
        PreflightResult, ResultStatus,
    },
    pdf::{self, PdfInspection},
    AppState,
};

const TOTAL_CHECKS: u8 = 9;

pub async fn run(
    state: &AppState,
    job_id: Uuid,
    bytes: bytes::Bytes,
    options: AnalysisOptions,
    callback: Option<CallbackTarget>,
) -> PreflightResult {
    let source_file = file_info(&bytes);
    let mut analysis_bytes = bytes;
    let original_magic = pdf::has_pdf_magic(&analysis_bytes);
    let mut converted_to_grayscale = false;
    let conversion_failed = if original_magic && options.color_mode == ColorMode::Mono {
        match crate::gs::convert_pdf_to_grayscale(
            &state.config.gs_bin,
            &analysis_bytes,
            &state.gs_permits,
            state.config.gs_timeout,
        )
        .await
        {
            Ok(converted) => {
                analysis_bytes = bytes::Bytes::from(converted);
                converted_to_grayscale = true;
                false
            }
            Err(_) => true,
        }
    } else {
        false
    };
    let analysed_file = file_info(&analysis_bytes);
    let analysis = AnalysisInfo {
        color_mode: options.color_mode,
        converted_to_grayscale,
        max_pages: options.max_pages,
        margin_mm: options.margin_mm,
        min_dpi: options.min_dpi,
        colour_threshold: options.colour_threshold,
    };
    let mut result = PreflightResult::new(job_id, source_file, analysed_file, analysis);

    let magic = original_magic && !conversion_failed;
    let inspection = if magic {
        let inspect_bytes = analysis_bytes.clone();
        tokio::task::spawn_blocking(move || pdf::inspect_pdf(&inspect_bytes))
            .await
            .ok()
            .and_then(Result::ok)
    } else {
        None
    };

    let pdf_valid = checks::pdf_valid::check(magic, inspection.is_some());
    push_step(state, &callback, &mut result, pdf_valid, 1).await;
    if result.status == ResultStatus::Failed {
        return finalize(state, callback, result).await;
    }

    let inspection = inspection.expect("inspection exists after pdf_valid pass");

    push_step(
        state,
        &callback,
        &mut result,
        checks::readable::check(&inspection),
        2,
    )
    .await;
    if result.status == ResultStatus::Failed {
        return finalize(state, callback, result).await;
    }

    push_step(
        state,
        &callback,
        &mut result,
        checks::encrypted::check(&inspection),
        3,
    )
    .await;
    if result.status == ResultStatus::Failed {
        return finalize(state, callback, result).await;
    }

    push_step(
        state,
        &callback,
        &mut result,
        checks::page_count::check(&inspection, options.max_pages),
        4,
    )
    .await;
    if result.status == ResultStatus::Failed {
        return finalize(state, callback, result).await;
    }

    push_step(
        state,
        &callback,
        &mut result,
        checks::page_dimensions::check(&inspection),
        5,
    )
    .await;
    push_step(
        state,
        &callback,
        &mut result,
        checks::margins::check(&inspection, options.margin_mm),
        6,
    )
    .await;

    let coverage = crate::gs::run_inkcov(
        &state.config.gs_bin,
        &analysis_bytes,
        &state.gs_permits,
        state.config.gs_timeout,
    )
    .await
    .ok()
    .map(|coverage| coverage_for_mode(coverage, options.color_mode));
    result.pages = page_results(&inspection, coverage.as_deref(), &options);
    let colour = coverage.clone().map_or_else(
        || checks::colour::check(Err(()), options.colour_threshold),
        |coverage| checks::colour::check(Ok(coverage), options.colour_threshold),
    );
    push_step(state, &callback, &mut result, colour, 7).await;

    push_step(
        state,
        &callback,
        &mut result,
        checks::blank_pages::check(&inspection),
        8,
    )
    .await;
    push_step(
        state,
        &callback,
        &mut result,
        checks::image_resolution::check(&inspection, options.min_dpi),
        9,
    )
    .await;

    finalize(state, callback, result).await
}

fn coverage_for_mode(mut coverage: Vec<InkCoverage>, color_mode: ColorMode) -> Vec<InkCoverage> {
    if color_mode == ColorMode::Mono {
        for page in &mut coverage {
            let gray = page.c.max(page.m).max(page.y).max(page.k);
            page.c = 0.0;
            page.m = 0.0;
            page.y = 0.0;
            page.k = gray;
        }
    }

    coverage
}

async fn push_step(
    state: &AppState,
    callback: &Option<CallbackTarget>,
    result: &mut PreflightResult,
    check: CheckResult,
    index: u8,
) {
    let event_check = check.clone();
    result.push_check(check);
    result.finalize();

    if let Some(target) = callback {
        state
            .callbacks
            .post_event(
                target,
                &CallbackEvent::Step {
                    job_id: result.job_id,
                    step: event_check.id,
                    index,
                    total: TOTAL_CHECKS,
                    status: event_check.status,
                    data: event_check.data,
                    ts: Utc::now(),
                },
            )
            .await;
    }
}

async fn finalize(
    state: &AppState,
    callback: Option<CallbackTarget>,
    mut result: PreflightResult,
) -> PreflightResult {
    result.finalize();

    if let Some(target) = callback {
        let event = if result.status == ResultStatus::Failed {
            CallbackEvent::Failed {
                job_id: result.job_id,
                result: result.clone(),
                ts: Utc::now(),
            }
        } else {
            CallbackEvent::Completed {
                job_id: result.job_id,
                result: result.clone(),
                ts: Utc::now(),
            }
        };
        state.callbacks.post_event(&target, &event).await;
    }

    result
}

fn page_results(
    inspection: &PdfInspection,
    coverage: Option<&[InkCoverage]>,
    options: &AnalysisOptions,
) -> Vec<PageResult> {
    inspection
        .pages
        .iter()
        .map(|page| {
            let tight = page
                .content_bbox
                .map(|bbox| {
                    pdf::is_tight_to_edge(
                        pdf::RectMm::new(0.0, 0.0, page.size.w_mm, page.size.h_mm),
                        bbox,
                        options.margin_mm,
                    )
                })
                .unwrap_or(false);
            let coverage = coverage.and_then(|coverage| {
                coverage
                    .iter()
                    .find(|coverage| coverage.page == page.page)
                    .map(|coverage| PageInkCoverage {
                        c: coverage.c,
                        m: coverage.m,
                        y: coverage.y,
                        k: coverage.k,
                    })
            });
            let has_colour = coverage
                .as_ref()
                .map(|coverage| coverage.c + coverage.m + coverage.y > options.colour_threshold)
                .unwrap_or(false);

            PageResult {
                page: page.page,
                size: PageSizeResult {
                    w_mm: page.size.w_mm,
                    h_mm: page.size.h_mm,
                    is_a4: pdf::is_a4_size_mm(page.size.w_mm, page.size.h_mm),
                },
                margins: PageMarginResult { tight },
                colour: PageColourResult {
                    has_colour,
                    coverage,
                },
                blank: pdf::is_blank_page(&page.content),
                images: page
                    .images
                    .iter()
                    .map(|image| {
                        let dpi = pdf::image_dpi(image);
                        PageImageResult {
                            pixel_width: image.pixel_width,
                            pixel_height: image.pixel_height,
                            placed: image.placed,
                            dpi,
                            low_res: dpi < options.min_dpi,
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

fn file_info(bytes: &[u8]) -> FileInfo {
    FileInfo {
        bytes: bytes.len() as u64,
        sha256: pdf::sha256_hex(bytes),
        pdf_version: pdf::pdf_version(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_coverage_reports_gray_as_black_only() {
        let coverage = coverage_for_mode(
            vec![InkCoverage {
                page: 1,
                c: 0.12445,
                m: 0.12445,
                y: 0.12445,
                k: 0.06161,
            }],
            ColorMode::Mono,
        );

        assert_eq!(coverage[0].c, 0.0);
        assert_eq!(coverage[0].m, 0.0);
        assert_eq!(coverage[0].y, 0.0);
        assert_eq!(coverage[0].k, 0.12445);
    }
}
