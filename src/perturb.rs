//! Perturbation divergence. One recorded tool result per synthetic trajectory
//! is changed, and the next request carries the change. The perturbed run is
//! replayed against a request-sensitive stub (changes its response with a set
//! probability when the request bytes differ from the record) and against the
//! position-keyed `RecordedEndpoint`, which never looks at the request.
//! Model-free; synthetic data.

use crate::chance::{generate, Rng};
use crate::{replay, RecordedEndpoint, ReplayEndpoint, Step, Trajectory};
use serde::Serialize;

/// Stub that answers from the record but, when the request differs from the
/// recorded request at the same position, changes its response with
/// probability `sensitivity`.
pub struct SensitiveEndpoint {
    requests: Vec<Vec<u8>>,
    responses: Vec<Vec<u8>>,
    cursor: usize,
    sensitivity: f64,
    rng: Rng,
}

impl SensitiveEndpoint {
    pub fn from_trajectory(t: &Trajectory, sensitivity: f64, seed: u64) -> Self {
        let mut requests = Vec::new();
        let mut responses = Vec::new();
        for s in &t.steps {
            match s {
                Step::LlmRequest { request_bytes } => requests.push(request_bytes.clone()),
                Step::LlmResponse { response_bytes } => responses.push(response_bytes.clone()),
                _ => {}
            }
        }
        Self { requests, responses, cursor: 0, sensitivity, rng: Rng::new(seed) }
    }
}

impl ReplayEndpoint for SensitiveEndpoint {
    fn invoke(&mut self, request_bytes: &[u8]) -> Vec<u8> {
        let i = self.cursor;
        self.cursor += 1;
        // Draw every call so the stream does not depend on outcomes.
        let u = self.rng.unit();
        let mut out = self.responses[i].clone();
        if request_bytes != self.requests[i].as_slice() && u < self.sensitivity {
            out.extend_from_slice(b"!changed");
        }
        out
    }
}

/// Change one tool result that is followed by another request; the next
/// request carries the change. Returns the perturbed trajectory and the index
/// of the first affected `LlmResponse`, or `None` if no tool result qualifies.
pub fn perturb_one(t: &Trajectory, rng: &mut Rng) -> Option<(Trajectory, usize)> {
    let eligible: Vec<usize> = (0..t.steps.len())
        .filter(|&i| matches!(t.steps[i], Step::ToolResult { .. }))
        .filter(|&i| t.steps[i..].iter().any(|s| matches!(s, Step::LlmRequest { .. })))
        .collect();
    if eligible.is_empty() {
        return None;
    }
    let at = eligible[(rng.next_u64() % eligible.len() as u64) as usize];
    let mut p = t.clone();
    if let Step::ToolResult { result_bytes, .. } = &mut p.steps[at] {
        result_bytes.extend_from_slice(b"|perturbed");
    }
    let next_req = (at + 1..p.steps.len())
        .find(|&j| matches!(p.steps[j], Step::LlmRequest { .. }))?;
    if let Step::LlmRequest { request_bytes } = &mut p.steps[next_req] {
        request_bytes.extend_from_slice(b"|perturbed");
    }
    let resp = (next_req + 1..p.steps.len())
        .find(|&j| matches!(p.steps[j], Step::LlmResponse { .. }))?;
    Some((p, resp))
}

#[derive(Serialize, Debug)]
pub struct Divergence {
    pub first_step_detection_rate: f64,
    pub later_or_never_rate: f64,
    pub recorded_endpoint_detection_rate: f64,
    pub detected: usize,
    pub recorded_detected: usize,
    pub perturbed_trajectories: usize,
}

#[derive(Serialize, Debug)]
pub struct PerturbResult {
    pub divergence: Divergence,
    pub sensitivity: f64,
    pub trajectories: usize,
    pub skipped_no_eligible_tool_result: usize,
    pub turns: usize,
    pub seed: u64,
}

pub fn run(seed: u64, trajectories: usize, turns: usize, sensitivity: f64) -> PerturbResult {
    let mut gen = Rng::new(seed);
    let mut prng = Rng::new(seed ^ 0xBEEF);
    let (mut n, mut hit, mut rec_hit, mut skipped) = (0usize, 0usize, 0usize, 0usize);
    for i in 0..trajectories {
        let t = generate(&mut gen, i, turns, 0.6);
        let Some((p, first_resp)) = perturb_one(&t, &mut prng) else {
            skipped += 1;
            continue;
        };
        n += 1;
        let mut s = SensitiveEndpoint::from_trajectory(&t, sensitivity, seed ^ 0x5EED ^ i as u64);
        let r = replay(&p, &mut s);
        if r.first_divergence.map(|d| d.step_index) == Some(first_resp) {
            hit += 1;
        }
        let mut rec = RecordedEndpoint::from_trajectory(&t);
        if !replay(&p, &mut rec).reproducible {
            rec_hit += 1;
        }
    }
    let d = n.max(1) as f64;
    PerturbResult {
        divergence: Divergence {
            first_step_detection_rate: hit as f64 / d,
            later_or_never_rate: (n - hit) as f64 / d,
            recorded_endpoint_detection_rate: rec_hit as f64 / d,
            detected: hit,
            recorded_detected: rec_hit,
            perturbed_trajectories: n,
        },
        sensitivity,
        trajectories,
        skipped_no_eligible_tool_result: skipped,
        turns,
        seed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_endpoint_never_notices() {
        let r = run(7, 50, 40, 1.0);
        assert_eq!(r.divergence.recorded_detected, 0);
        assert_eq!(r.divergence.detected, r.divergence.perturbed_trajectories);
    }

    #[test]
    fn zero_sensitivity_detects_nothing() {
        let r = run(7, 50, 40, 0.0);
        assert_eq!(r.divergence.detected, 0);
        assert_eq!(r.divergence.later_or_never_rate, 1.0);
    }

    #[test]
    fn deterministic() {
        let a = serde_json::to_string(&run(7, 30, 30, 0.3)).unwrap();
        let b = serde_json::to_string(&run(7, 30, 30, 0.3)).unwrap();
        assert_eq!(a, b);
    }
}
