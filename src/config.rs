// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{env, net::SocketAddr, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisOptions {
    pub max_pages: u32,
    pub margin_mm: f64,
    pub min_dpi: f64,
    pub colour_threshold: f64,
    pub color_mode: ColorMode,
    pub fit_to_page: bool,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            max_pages: 500,
            margin_mm: 5.0,
            min_dpi: 150.0,
            colour_threshold: 0.01,
            color_mode: ColorMode::Color,
            fit_to_page: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    Color,
    Mono,
}

impl FromStr for ColorMode {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "color" => Ok(Self::Color),
            "mono" => Ok(Self::Mono),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub api_bearer_token: String,
    pub bind_addr: SocketAddr,
    pub max_upload_bytes: u64,
    pub defaults: AnalysisOptions,
    pub gs_concurrency: usize,
    pub gs_bin: String,
    pub gs_timeout: Duration,
    pub callback_hosts: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("API_BEARER_TOKEN")]
    MissingToken,
    #[error("{0}")]
    Invalid(&'static str),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_bearer_token =
            env::var("API_BEARER_TOKEN").map_err(|_| ConfigError::MissingToken)?;

        if api_bearer_token.trim().is_empty() || api_bearer_token == "change-me" {
            return Err(ConfigError::Invalid("API_BEARER_TOKEN"));
        }

        Ok(Self {
            api_bearer_token,
            bind_addr: parse_env("BIND_ADDR", "0.0.0.0:8080")?,
            max_upload_bytes: parse_env("MAX_UPLOAD_BYTES", "209715200")?,
            defaults: AnalysisOptions {
                max_pages: parse_env("MAX_PAGES", "500")?,
                margin_mm: parse_env("MARGIN_MM", "5")?,
                min_dpi: parse_env("MIN_DPI", "150")?,
                colour_threshold: parse_env("COLOUR_THRESHOLD", "0.01")?,
                color_mode: parse_env("COLOR_MODE", "color")?,
                fit_to_page: false,
            },
            gs_concurrency: env::var("GS_CONCURRENCY")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| ConfigError::Invalid("GS_CONCURRENCY"))
                })
                .transpose()?
                .unwrap_or_else(|| num_cpus::get().max(1)),
            gs_bin: env::var("GS_BIN").unwrap_or_else(|_| "gs".to_owned()),
            gs_timeout: Duration::from_secs(parse_env("GS_TIMEOUT_SECONDS", "300")?),
            callback_hosts: env::var("CALLBACK_HOSTS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }

    pub fn for_tests(token: &str) -> Self {
        Self {
            api_bearer_token: token.to_owned(),
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            max_upload_bytes: 1024 * 1024,
            defaults: AnalysisOptions::default(),
            gs_concurrency: 1,
            gs_bin: "gs".to_owned(),
            gs_timeout: Duration::from_secs(300),
            callback_hosts: vec!["127.0.0.1".to_owned()],
        }
    }
}

fn parse_env<T>(key: &'static str, default: &str) -> Result<T, ConfigError>
where
    T: FromStr,
{
    env::var(key)
        .unwrap_or_else(|_| default.to_owned())
        .parse()
        .map_err(|_| ConfigError::Invalid(key))
}
