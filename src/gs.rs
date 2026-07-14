// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{path::Path, process::Output, time::Duration};

use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use thiserror::Error;
use tokio::{process::Command, sync::Semaphore};

/// Render resolution for ink coverage analysis. Ghostscript's default (72dpi)
/// under-samples thin strokes and text, skewing coverage on text-heavy pages.
const INKCOV_RESOLUTION_DPI: u32 = 150;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InkCoverage {
    pub page: u32,
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
}

#[derive(Debug, Error)]
pub enum GsError {
    #[error("coverage")]
    MissingCoverage,
    #[error("parse")]
    Parse,
    #[error("io")]
    Io(#[from] std::io::Error),
    #[error("join")]
    Join(#[from] tokio::task::JoinError),
    #[error("status")]
    Status,
    #[error("semaphore")]
    Semaphore,
    #[error("timeout")]
    Timeout,
}

pub fn parse_inkcov(output: &str) -> Result<Vec<InkCoverage>, GsError> {
    let mut coverage = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.ends_with("CMYK OK") {
            continue;
        }

        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() < 6 {
            return Err(GsError::Parse);
        }

        coverage.push(InkCoverage {
            page: coverage.len() as u32 + 1,
            c: parts[0].parse().map_err(|_| GsError::Parse)?,
            m: parts[1].parse().map_err(|_| GsError::Parse)?,
            y: parts[2].parse().map_err(|_| GsError::Parse)?,
            k: parts[3].parse().map_err(|_| GsError::Parse)?,
        });
    }

    if coverage.is_empty() {
        Err(GsError::MissingCoverage)
    } else {
        Ok(coverage)
    }
}

pub async fn run_inkcov(
    gs_bin: &str,
    pdf: &[u8],
    semaphore: &Semaphore,
    timeout: Duration,
) -> Result<Vec<InkCoverage>, GsError> {
    let _permit = semaphore.acquire().await.map_err(|_| GsError::Semaphore)?;
    let dir = tempdir()?;
    let path = dir.path().join("input.pdf");
    tokio::fs::write(&path, pdf).await?;
    run_inkcov_file(gs_bin, &path, timeout).await
}

pub async fn convert_pdf_to_grayscale(
    gs_bin: &str,
    pdf: &[u8],
    semaphore: &Semaphore,
    timeout: Duration,
) -> Result<Vec<u8>, GsError> {
    let _permit = semaphore.acquire().await.map_err(|_| GsError::Semaphore)?;
    let dir = tempdir()?;
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("output.pdf");
    tokio::fs::write(&input, pdf).await?;

    let mut command = Command::new(gs_bin);
    command
        .arg("-q")
        .arg("-dSAFER")
        .arg("-dBATCH")
        .arg("-dNOPAUSE")
        .arg("-sDEVICE=pdfwrite")
        .arg("-sColorConversionStrategy=Gray")
        .arg("-dProcessColorModel=/DeviceGray")
        .arg("-dCompatibilityLevel=1.4")
        .arg(format!("-sOutputFile={}", output.display()))
        .arg(&input);

    let output_result = output_with_timeout(command, timeout).await?;

    if !output_result.status.success() {
        return Err(GsError::Status);
    }

    Ok(tokio::fs::read(output).await?)
}

async fn run_inkcov_file(
    gs_bin: &str,
    path: &Path,
    timeout: Duration,
) -> Result<Vec<InkCoverage>, GsError> {
    let mut command = Command::new(gs_bin);
    command
        .arg("-q")
        .arg("-o")
        .arg("-")
        .arg("-sDEVICE=inkcov")
        .arg(format!("-r{INKCOV_RESOLUTION_DPI}"))
        .arg(path);
    let output = output_with_timeout(command, timeout).await?;

    if !output.status.success() {
        return Err(GsError::Status);
    }

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    parse_inkcov(&combined)
}

async fn output_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, GsError> {
    let output = command.kill_on_drop(true).output();

    tokio::time::timeout(timeout, output)
        .await
        .map_err(|_| GsError::Timeout)?
        .map_err(GsError::Io)
}

pub async fn ghostscript_version(gs_bin: &str) -> String {
    match Command::new(gs_bin).arg("--version").output().await {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        _ => "unknown".to_owned(),
    }
}
