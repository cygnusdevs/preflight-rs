// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{config::ColorMode, pdf::PageSizeMm};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub severity: Severity,
    pub status: CheckStatus,
    pub data: Value,
}

impl CheckResult {
    pub fn new(
        id: impl Into<String>,
        severity: Severity,
        status: CheckStatus,
        data: Value,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            status,
            data,
        }
    }

    pub fn is_hard_failure(&self) -> bool {
        self.severity == Severity::Error && self.status == CheckStatus::Fail
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileInfo {
    pub bytes: u64,
    pub sha256: String,
    pub pdf_version: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub pages: u32,
    pub has_colour: bool,
    pub errors: u32,
    pub warnings: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisInfo {
    pub color_mode: ColorMode,
    pub converted_to_grayscale: bool,
    pub fit_to_page: bool,
    pub fitted_to_page: bool,
    pub max_pages: u32,
    pub margin_mm: f64,
    pub min_dpi: f64,
    pub colour_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSizeResult {
    pub w_mm: f64,
    pub h_mm: f64,
    pub is_a4: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageMarginResult {
    pub tight: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageInkCoverage {
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageColourResult {
    pub has_colour: bool,
    pub coverage: Option<PageInkCoverage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageImageResult {
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub placed: PageSizeMm,
    pub dpi: f64,
    pub low_res: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageResult {
    pub page: u32,
    pub size: PageSizeResult,
    pub margins: PageMarginResult,
    pub colour: PageColourResult,
    pub blank: bool,
    pub images: Vec<PageImageResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightResult {
    pub schema_version: String,
    pub job_id: Uuid,
    pub status: ResultStatus,
    pub source_file: FileInfo,
    pub analysed_file: FileInfo,
    pub analysis: AnalysisInfo,
    pub summary: Summary,
    pub pages: Vec<PageResult>,
    pub checks: Vec<CheckResult>,
}

impl PreflightResult {
    pub fn new(
        job_id: Uuid,
        source_file: FileInfo,
        analysed_file: FileInfo,
        analysis: AnalysisInfo,
    ) -> Self {
        Self {
            schema_version: "3.0".to_owned(),
            job_id,
            status: ResultStatus::Completed,
            source_file,
            analysed_file,
            analysis,
            summary: Summary::default(),
            pages: Vec::new(),
            checks: Vec::new(),
        }
    }

    pub fn push_check(&mut self, check: CheckResult) {
        self.checks.push(check);
    }

    pub fn finalize(&mut self) {
        self.status = if self.checks.iter().any(CheckResult::is_hard_failure) {
            ResultStatus::Failed
        } else {
            ResultStatus::Completed
        };
        self.summary.errors = self
            .checks
            .iter()
            .filter(|check| check.severity == Severity::Error && check.status == CheckStatus::Fail)
            .count() as u32;
        self.summary.warnings = self
            .checks
            .iter()
            .filter(|check| {
                check.severity == Severity::Warning && check.status != CheckStatus::Pass
            })
            .count() as u32;

        if let Some(count) = self
            .checks
            .iter()
            .find(|check| check.id == "page_count")
            .and_then(|check| check.data.get("count"))
            .and_then(Value::as_u64)
        {
            self.summary.pages = count as u32;
        }

        if let Some(has_colour) = self
            .checks
            .iter()
            .find(|check| check.id == "colour")
            .and_then(|check| check.data.get("has_colour"))
            .and_then(Value::as_bool)
        {
            self.summary.has_colour = has_colour;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallbackTarget {
    pub url: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum CallbackEvent {
    Step {
        job_id: Uuid,
        step: String,
        index: u8,
        total: u8,
        status: CheckStatus,
        data: Value,
        ts: DateTime<Utc>,
    },
    Completed {
        job_id: Uuid,
        result: PreflightResult,
        ts: DateTime<Utc>,
    },
    Failed {
        job_id: Uuid,
        result: PreflightResult,
        ts: DateTime<Utc>,
    },
}
