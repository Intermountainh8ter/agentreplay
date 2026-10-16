//! Live HTTP record-mode wrapper.
//!
//! An `HttpEndpoint` posts the LlmRequest bytes to a configured URL and
//! returns the response bytes verbatim, so the trajectory engine can
//! record a real agent run as a `Trajectory` the existing replay path
//! then re-runs byte-identical against the same endpoint or a mock.
//!
//! The scaffold records single-turn trajectories (one LlmRequest ->
//! LlmResponse pair). A multi-turn tool-dispatch loop needs a scripted
//! agent on top of this; its wrapper will reuse `HttpEndpoint::invoke`
//! and append ToolCall / ToolResult steps between the LLM pairs.

use crate::{ReplayEndpoint, Step, Trajectory};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpEndpointConfig {
    pub url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_timeout_s")]
    pub timeout_s: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_timeout_s() -> u64 {
    60
}

fn default_max_retries() -> u32 {
    2
}

#[derive(Debug, Error)]
pub enum HttpEndpointError {
    #[error("api_key_env `{0}` is set but the environment variable is not set")]
    AuthMissing(String),
    #[error("endpoint refused authentication (HTTP {status})")]
    AuthRejected { status: u16 },
    #[error("endpoint returned HTTP {status} after {attempts} attempts")]
    HttpStatus { status: u16, attempts: u32 },
    #[error("transport error after {attempts} attempts: {source}")]
    Transport {
        attempts: u32,
        #[source]
        source: reqwest::Error,
    },
}

pub struct HttpEndpoint {
    config: HttpEndpointConfig,
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    last_error: Option<HttpEndpointError>,
}

impl HttpEndpoint {
    pub fn new(config: HttpEndpointConfig) -> Result<Self, HttpEndpointError> {
        if let Some(name) = &config.api_key_env {
            if std::env::var(name).is_err() {
                return Err(HttpEndpointError::AuthMissing(name.clone()));
            }
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_s))
            .build()
            .map_err(|e| HttpEndpointError::Transport {
                attempts: 0,
                source: e,
            })?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime builds");
        Ok(Self {
            config,
            client,
            runtime,
            last_error: None,
        })
    }

    /// Take the last transport-layer error, if any. `invoke` returns
    /// bytes (because the trait says so) and sets this field on failure;
    /// the caller inspects it after the replay.
    pub fn take_last_error(&mut self) -> Option<HttpEndpointError> {
        self.last_error.take()
    }

    fn resolved_key(&self) -> Option<String> {
        self.config.api_key_env.as_deref().and_then(|n| std::env::var(n).ok())
    }
}

impl ReplayEndpoint for HttpEndpoint {
    fn invoke(&mut self, request_bytes: &[u8]) -> Vec<u8> {
        let url = self.config.url.clone();
        let auth = self.resolved_key();
        let max_retries = self.config.max_retries;
        let client = self.client.clone();
        let body = request_bytes.to_vec();
        let result = self
            .runtime
            .block_on(async move { send_bytes(&client, &url, auth.as_deref(), body, max_retries).await });
        match result {
            Ok(bytes) => bytes,
            Err(e) => {
                self.last_error = Some(e);
                Vec::new()
            }
        }
    }
}

async fn send_bytes(
    client: &reqwest::Client,
    url: &str,
    auth: Option<&str>,
    body: Vec<u8>,
    max_retries: u32,
) -> Result<Vec<u8>, HttpEndpointError> {
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        let mut req = client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(body.clone());
        if let Some(k) = auth {
            req = req.bearer_auth(k);
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return resp
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(|e| HttpEndpointError::Transport {
                            attempts: attempt,
                            source: e,
                        });
                }
                if status.as_u16() == 401 || status.as_u16() == 403 {
                    return Err(HttpEndpointError::AuthRejected {
                        status: status.as_u16(),
                    });
                }
                let retryable = status.as_u16() == 429 || status.is_server_error();
                if !retryable || attempt > max_retries {
                    return Err(HttpEndpointError::HttpStatus {
                        status: status.as_u16(),
                        attempts: attempt,
                    });
                }
            }
            Err(e) => {
                let retryable = e.is_timeout() || e.is_connect() || e.is_request();
                if !retryable || attempt > max_retries {
                    return Err(HttpEndpointError::Transport {
                        attempts: attempt,
                        source: e,
                    });
                }
            }
        }
        let delay_ms = 100u64.saturating_mul(1u64 << (attempt - 1).min(10));
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
}

/// Record a single-turn trajectory: POST `request_bytes` to the endpoint,
/// capture the response, and emit a two-step Trajectory
/// `[LlmRequest, LlmResponse]`. The caller supplies the `run_id` so a
/// recorded trajectory can be re-identified later.
pub fn record_single_turn(
    endpoint: &mut HttpEndpoint,
    run_id: &str,
    request_bytes: &[u8],
) -> Result<Trajectory, HttpEndpointError> {
    let response_bytes = endpoint.invoke(request_bytes);
    if let Some(e) = endpoint.take_last_error() {
        return Err(e);
    }
    Ok(Trajectory {
        run_id: run_id.to_string(),
        steps: vec![
            Step::LlmRequest {
                request_bytes: request_bytes.to_vec(),
            },
            Step::LlmResponse { response_bytes },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{replay, trajectory_hash, RecordedEndpoint};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config(base: &str) -> HttpEndpointConfig {
        HttpEndpointConfig {
            url: format!("{base}/invoke"),
            api_key_env: None,
            timeout_s: 2,
            max_retries: 2,
        }
    }

    #[test]
    fn constructor_rejects_missing_env_var() {
        std::env::remove_var("AGENTREPLAY_TEST_MISSING_X7Y3");
        let mut c = config("http://127.0.0.1:1");
        c.api_key_env = Some("AGENTREPLAY_TEST_MISSING_X7Y3".into());
        match HttpEndpoint::new(c) {
            Err(HttpEndpointError::AuthMissing(name)) => {
                assert_eq!(name, "AGENTREPLAY_TEST_MISSING_X7Y3");
            }
            Err(other) => panic!("wrong variant: {other:?}"),
            Ok(_) => panic!("constructor should have rejected missing env var"),
        }
    }

    #[test]
    fn records_a_single_turn_trajectory_the_recorded_endpoint_can_replay() {
        let server = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async { MockServer::start().await });
        let body = b"hello-world";
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                Mock::given(method("POST"))
                    .and(path("/invoke"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
                    .mount(&server)
                    .await;
            });
        let mut endpoint = HttpEndpoint::new(config(&server.uri())).expect("constructs");
        let traj = record_single_turn(&mut endpoint, "run-1", b"request-bytes").expect("records");
        assert_eq!(traj.steps.len(), 2);
        match &traj.steps[1] {
            Step::LlmResponse { response_bytes } => assert_eq!(response_bytes, body),
            s => panic!("want LlmResponse, got {s:?}"),
        }
        let h = trajectory_hash(&traj);
        // A replay against RecordedEndpoint reproduces the same chained hash.
        let mut re = RecordedEndpoint::from_trajectory(&traj);
        let rep = replay(&traj, &mut re);
        assert!(rep.reproducible, "replay failed: {rep:?}");
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn retries_a_429_then_succeeds() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let server = runtime.block_on(async { MockServer::start().await });
        runtime.block_on(async {
            Mock::given(method("POST"))
                .and(path("/invoke"))
                .respond_with(ResponseTemplate::new(429))
                .up_to_n_times(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/invoke"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
                .mount(&server)
                .await;
        });
        let mut endpoint = HttpEndpoint::new(config(&server.uri())).unwrap();
        let traj = record_single_turn(&mut endpoint, "r", b"x").expect("retry ok");
        match &traj.steps[1] {
            Step::LlmResponse { response_bytes } => assert_eq!(response_bytes, b"ok"),
            s => panic!("want LlmResponse, got {s:?}"),
        }
    }

    #[test]
    fn auth_rejected_surfaces_as_an_error_not_an_empty_response() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let server = runtime.block_on(async { MockServer::start().await });
        runtime.block_on(async {
            Mock::given(method("POST"))
                .and(path("/invoke"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&server)
                .await;
        });
        let mut endpoint = HttpEndpoint::new(config(&server.uri())).unwrap();
        let err = record_single_turn(&mut endpoint, "r", b"x").expect_err("auth refused");
        assert!(matches!(err, HttpEndpointError::AuthRejected { status: 401 }));
    }
}
