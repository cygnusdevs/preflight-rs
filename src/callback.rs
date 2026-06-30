// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use reqwest::redirect::Policy;
use tracing::warn;

use crate::models::{CallbackEvent, CallbackTarget};

#[derive(Clone)]
pub struct CallbackClient {
    client: reqwest::Client,
}

impl CallbackClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client");
        Self { client }
    }

    pub async fn post_event(&self, target: &CallbackTarget, event: &CallbackEvent) {
        for attempt in 0..3 {
            let mut request = self.client.post(&target.url).json(event);
            if let Some(token) = &target.token {
                request = request.bearer_auth(token);
            }

            match request.send().await {
                Ok(response) if response.status().is_success() => return,
                Ok(response) if !response.status().is_server_error() => {
                    warn!(status = %response.status(), "callback rejected");
                    return;
                }
                Ok(response) => {
                    warn!(status = %response.status(), attempt = attempt + 1, "callback retry");
                }
                Err(error) => {
                    warn!(%error, attempt = attempt + 1, "callback retry");
                }
            }

            tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
        }

        warn!("callback give up");
    }
}

impl Default for CallbackClient {
    fn default() -> Self {
        Self::new()
    }
}
