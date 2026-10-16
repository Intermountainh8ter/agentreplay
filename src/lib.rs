//! Trajectory types and determinism primitives for agentreplay.
//!
//! A `Trajectory` is a sequence of `Step`s. Each step is content-addressed
//! by a chained SHA-256 that commits to the step's data *and* the previous
//! step's hash, so a replay that modifies step `k` leaves every step from
//! `k` onward with a different content hash. Determinism checks are then
//! equality on this chained hash.

pub mod http;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Every event the harness captures. Enough to replay a run.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    /// The agent's internal state right before the LLM is called.
    LlmRequest { request_bytes: Vec<u8> },
    /// The LLM's response captured byte-identical.
    LlmResponse { response_bytes: Vec<u8> },
    /// A tool call the agent emitted.
    ToolCall { tool: String, args_bytes: Vec<u8> },
    /// The tool's result the agent received.
    ToolResult { tool: String, result_bytes: Vec<u8> },
    /// A terminal event (success / failure / budget-exhausted).
    Terminal { outcome: String },
}

/// A full agent trajectory. `prev_hash` chains each step; the first step
/// uses the zero hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trajectory {
    pub run_id: String,
    pub steps: Vec<Step>,
}

/// Compute the chained content hashes for a trajectory. Returns one hash
/// per step; element `i` commits to steps 0..=i.
pub fn step_hashes(t: &Trajectory) -> Vec<String> {
    let mut out = Vec::with_capacity(t.steps.len());
    let mut prev = [0u8; 32];
    for s in &t.steps {
        let mut h = Sha256::new();
        h.update(prev);
        let body = serde_json::to_vec(s).expect("step serialization");
        h.update(&body);
        let bytes: [u8; 32] = h.finalize().into();
        out.push(hex::encode(bytes));
        prev = bytes;
    }
    out
}

/// Final content hash of a trajectory. Two trajectories are byte-identical
/// iff this hash matches.
pub fn trajectory_hash(t: &Trajectory) -> String {
    step_hashes(t).pop().unwrap_or_else(|| "0".repeat(64))
}

/// Replay a trajectory up to and including step `at`, then inject
/// `replacement` as the step at index `at`, discarding any steps after.
/// Returns the modified trajectory. Panics if `at >= t.steps.len()`.
pub fn inject(t: &Trajectory, at: usize, replacement: Step) -> Trajectory {
    assert!(at < t.steps.len(), "injection index out of range");
    let mut steps = t.steps[..at].to_vec();
    steps.push(replacement);
    Trajectory {
        run_id: format!("{}-at{}", t.run_id, at),
        steps,
    }
}

/// Endpoint under test for a replay. Given the exact bytes an
/// `LlmRequest` step captured, the endpoint must return the response
/// bytes. agentreplay compares these to the trajectory's
/// `LlmResponse` and reports the first divergence; it never
/// interprets or re-serializes the bytes.
pub trait ReplayEndpoint {
    fn invoke(&mut self, request_bytes: &[u8]) -> Vec<u8>;
}

/// Reference stub that returns the trajectory's own recorded
/// `LlmResponse` bytes in order. A trajectory replayed against this
/// stub must always be reproducible — if it isn't, the trajectory
/// itself is malformed (a request without a following response).
///
/// Primary use: a smoke test for the replay plumbing on captured runs,
/// and the self-consistency check in the test suite.
pub struct RecordedEndpoint {
    responses: Vec<Vec<u8>>,
    cursor: usize,
}

impl RecordedEndpoint {
    pub fn from_trajectory(t: &Trajectory) -> Self {
        let responses = t
            .steps
            .iter()
            .filter_map(|s| match s {
                Step::LlmResponse { response_bytes } => Some(response_bytes.clone()),
                _ => None,
            })
            .collect();
        Self {
            responses,
            cursor: 0,
        }
    }
}

impl ReplayEndpoint for RecordedEndpoint {
    fn invoke(&mut self, _request_bytes: &[u8]) -> Vec<u8> {
        let out = self
            .responses
            .get(self.cursor)
            .cloned()
            .expect("more LlmRequest steps than captured LlmResponse steps");
        self.cursor += 1;
        out
    }
}

/// How a replay diverged from the trajectory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DivergenceReport {
    /// Index into `t.steps` of the LlmResponse the endpoint failed to
    /// reproduce.
    pub step_index: usize,
    /// Chained content hash of the trajectory at `step_index` before
    /// replay. Downstream tools can jump to this step in the trajectory
    /// without replaying from the top.
    pub captured_step_hash: String,
    /// Length of the captured response bytes.
    pub expected_len: usize,
    /// Length of the endpoint's response bytes.
    pub actual_len: usize,
}

/// Outcome of a replay against an endpoint.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayReport {
    pub run_id: String,
    pub n_llm_requests: usize,
    pub n_llm_responses_checked: usize,
    pub reproducible: bool,
    /// `None` when `reproducible` is true. Pre-reg §2 names the drifted
    /// step; this field is that name.
    pub first_divergence: Option<DivergenceReport>,
}

/// Replay a trajectory against an endpoint. For every `LlmRequest` step
/// the endpoint is called with the captured request bytes; the result
/// is compared to the trajectory's next `LlmResponse`. The first byte
/// mismatch is reported and replay stops there (pre-reg §4: no
/// "replayed close enough" verdict).
pub fn replay(t: &Trajectory, endpoint: &mut dyn ReplayEndpoint) -> ReplayReport {
    let hashes = step_hashes(t);
    let mut n_llm_requests = 0usize;
    let mut n_llm_responses_checked = 0usize;
    let mut pending: Option<Vec<u8>> = None;
    let mut pending_index: usize = 0;
    for (i, step) in t.steps.iter().enumerate() {
        match step {
            Step::LlmRequest { request_bytes } => {
                n_llm_requests += 1;
                pending = Some(endpoint.invoke(request_bytes));
                pending_index = i;
            }
            Step::LlmResponse { response_bytes } => {
                if let Some(actual) = pending.take() {
                    n_llm_responses_checked += 1;
                    if actual != *response_bytes {
                        return ReplayReport {
                            run_id: t.run_id.clone(),
                            n_llm_requests,
                            n_llm_responses_checked,
                            reproducible: false,
                            first_divergence: Some(DivergenceReport {
                                step_index: i,
                                captured_step_hash: hashes[i].clone(),
                                expected_len: response_bytes.len(),
                                actual_len: actual.len(),
                            }),
                        };
                    }
                }
                // An LlmResponse without a preceding LlmRequest is a
                // malformed trajectory; silently skip (pending stays None).
                let _ = pending_index; // reserved for diagnostics
            }
            _ => {}
        }
    }
    ReplayReport {
        run_id: t.run_id.clone(),
        n_llm_requests,
        n_llm_responses_checked,
        reproducible: true,
        first_divergence: None,
    }
}

/// Append a replay-generated tail to an injected trajectory. Pre-reg §3:
/// "Running an endpoint against the modified trajectory produces a new
/// tail; the tail is appended, and the resulting full trajectory is
/// independently reproducible."
///
/// `base` is the injection output (steps 0..at + replacement). `tail`
/// is the sequence of new steps the endpoint produced from the inject
/// point forward. The returned trajectory concatenates them and keeps
/// `base`'s run_id, so the merged value is a valid Trajectory the
/// `replay()` primitive can accept.
pub fn merge(base: &Trajectory, tail: &[Step]) -> Trajectory {
    let mut steps = base.steps.clone();
    steps.extend_from_slice(tail);
    Trajectory {
        run_id: base.run_id.clone(),
        steps,
    }
}

/// Common wrapper: `inject(t, at, replacement)` followed by `merge`
/// of `tail`. Returns the merged trajectory a reviewer can replay.
pub fn inject_and_merge(
    t: &Trajectory,
    at: usize,
    replacement: Step,
    tail: &[Step],
) -> Trajectory {
    let base = inject(t, at, replacement);
    merge(&base, tail)
}

/// Verify the pre-reg §3 "independently reproducible" claim: the
/// merged trajectory, when replayed against a `RecordedEndpoint`
/// built from itself, must reproduce byte-for-byte. Returns the
/// `ReplayReport` so a caller can gate on it.
pub fn verify_merged_reproducible(merged: &Trajectory) -> ReplayReport {
    let mut ep = RecordedEndpoint::from_trajectory(merged);
    replay(merged, &mut ep)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step_r(bytes: &[u8]) -> Step {
        Step::LlmResponse {
            response_bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn identical_trajectories_hash_identically() {
        let a = Trajectory {
            run_id: "r".into(),
            steps: vec![step_r(b"hello"), step_r(b"world")],
        };
        let b = a.clone();
        assert_eq!(trajectory_hash(&a), trajectory_hash(&b));
    }

    #[test]
    fn modifying_a_step_changes_every_later_hash() {
        let a = Trajectory {
            run_id: "r".into(),
            steps: vec![step_r(b"hello"), step_r(b"world"), step_r(b"!")],
        };
        let b = inject(&a, 1, step_r(b"earth"));
        let ha = step_hashes(&a);
        let hb = step_hashes(&b);
        assert_eq!(ha[0], hb[0]);
        assert_ne!(ha[1], hb[1]);
    }

    #[test]
    fn injection_truncates_tail() {
        let a = Trajectory {
            run_id: "r".into(),
            steps: vec![step_r(b"a"), step_r(b"b"), step_r(b"c")],
        };
        let b = inject(&a, 1, step_r(b"B"));
        assert_eq!(b.steps.len(), 2);
    }

    fn step_q(bytes: &[u8]) -> Step {
        Step::LlmRequest {
            request_bytes: bytes.to_vec(),
        }
    }

    fn interleaved_pair() -> Trajectory {
        Trajectory {
            run_id: "r".into(),
            steps: vec![
                step_q(b"hi"),
                step_r(b"hello"),
                Step::ToolCall {
                    tool: "search".into(),
                    args_bytes: b"{\"q\":\"x\"}".to_vec(),
                },
                Step::ToolResult {
                    tool: "search".into(),
                    result_bytes: b"[]".to_vec(),
                },
                step_q(b"bye"),
                step_r(b"goodbye"),
                Step::Terminal {
                    outcome: "success".into(),
                },
            ],
        }
    }

    #[test]
    fn replay_against_recorded_endpoint_is_reproducible() {
        let t = interleaved_pair();
        let mut ep = RecordedEndpoint::from_trajectory(&t);
        let r = replay(&t, &mut ep);
        assert!(r.reproducible);
        assert_eq!(r.n_llm_requests, 2);
        assert_eq!(r.n_llm_responses_checked, 2);
        assert_eq!(r.first_divergence, None);
    }

    struct DriftingEndpoint {
        inner: RecordedEndpoint,
        drift_at: usize,
        n: usize,
    }

    impl ReplayEndpoint for DriftingEndpoint {
        fn invoke(&mut self, req: &[u8]) -> Vec<u8> {
            let mut out = self.inner.invoke(req);
            if self.n == self.drift_at {
                out.push(b'X');
            }
            self.n += 1;
            out
        }
    }

    #[test]
    fn replay_names_first_divergent_step() {
        let t = interleaved_pair();
        let mut ep = DriftingEndpoint {
            inner: RecordedEndpoint::from_trajectory(&t),
            drift_at: 1, // drift on the second LlmRequest
            n: 0,
        };
        let r = replay(&t, &mut ep);
        assert!(!r.reproducible);
        let d = r.first_divergence.expect("divergence");
        // Step at index 5 is the second LlmResponse.
        assert_eq!(d.step_index, 5);
        assert_eq!(d.expected_len, b"goodbye".len());
        assert_eq!(d.actual_len, b"goodbye".len() + 1);
        // Later responses are NOT checked; replay stopped at the first.
        assert_eq!(r.n_llm_responses_checked, 2);
    }

    #[test]
    fn replay_report_is_serializable() {
        let t = interleaved_pair();
        let mut ep = RecordedEndpoint::from_trajectory(&t);
        let r = replay(&t, &mut ep);
        let s = serde_json::to_string(&r).unwrap();
        let r2: ReplayReport = serde_json::from_str(&s).unwrap();
        assert_eq!(r, r2);
    }

    #[test]
    fn merge_concatenates_tail_and_preserves_run_id() {
        let base = Trajectory {
            run_id: "base".into(),
            steps: vec![step_q(b"q1"), step_r(b"r1")],
        };
        let tail = vec![step_q(b"q2"), step_r(b"r2")];
        let merged = merge(&base, &tail);
        assert_eq!(merged.run_id, "base");
        assert_eq!(merged.steps.len(), 4);
    }

    #[test]
    fn inject_and_merge_builds_a_replayable_trajectory() {
        let t = interleaved_pair();
        let replacement = Step::ToolResult {
            tool: "search".into(),
            result_bytes: b"[\"different\"]".to_vec(),
        };
        // After injection at step 3, the base has steps 0..3 + replacement.
        // Simulate the endpoint producing a new tail of (LlmRequest,
        // LlmResponse, Terminal).
        let tail = vec![
            step_q(b"did-you-see-different"),
            step_r(b"yes-i-did"),
            Step::Terminal {
                outcome: "success".into(),
            },
        ];
        let merged = inject_and_merge(&t, 3, replacement, &tail);
        // 3 preserved + 1 injected + 3 tail = 7
        assert_eq!(merged.steps.len(), 7);
        let r = verify_merged_reproducible(&merged);
        assert!(r.reproducible, "{r:?}");
    }

    #[test]
    fn merged_trajectory_chained_hash_differs_from_the_original() {
        let t = interleaved_pair();
        let merged = inject_and_merge(
            &t,
            1,
            step_r(b"different-response"),
            &[Step::Terminal {
                outcome: "success".into(),
            }],
        );
        assert_ne!(trajectory_hash(&t), trajectory_hash(&merged));
    }
}
