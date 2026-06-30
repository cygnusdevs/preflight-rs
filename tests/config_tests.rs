// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::{Mutex, OnceLock};

use preflight_rs::config::{Config, ConfigError};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn config_rejects_placeholder_api_token() {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    std::env::set_var("API_BEARER_TOKEN", "change-me");

    let error = Config::from_env().expect_err("placeholder token is rejected");

    assert!(matches!(error, ConfigError::Invalid("API_BEARER_TOKEN")));
    std::env::remove_var("API_BEARER_TOKEN");
}
