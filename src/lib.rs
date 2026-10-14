//! Trajectory types and determinism primitives for agentreplay.
//!
//! A `Trajectory` is a sequence of `Step`s. Each step is content-addressed
//! by a chained SHA-256 that commits to the step's data *and* the previous
//! step's hash, so a replay that modifies step `k` leaves every step from
//! `k` onward with a different content hash. Determinism checks are then
//! equality on this chained hash.

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
}
