//! Chance floor of a tool-call-presence assertion suite.
//!
//! For every `LlmResponse` step a presence assertion is derived: the
//! response is followed by a `ToolCall` or it is not. Degenerate callers
//! (never-calls, always-calls, seeded random) are scored through the
//! `ReplayEndpoint` trait against that suite. Model-free; synthetic data.

use crate::{ReplayEndpoint, Step, Trajectory};
use serde::Serialize;

pub const CALL: &[u8] = b"call";
pub const TEXT: &[u8] = b"text";

/// splitmix64: tiny seeded generator, no dependency.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Synthetic trajectory: `turns` turns, each LlmRequest, LlmResponse and,
/// with probability `call_share`, a ToolCall and ToolResult.
pub fn generate(rng: &mut Rng, id: usize, turns: usize, call_share: f64) -> Trajectory {
    let mut steps = Vec::new();
    for turn in 0..turns {
        steps.push(Step::LlmRequest {
            request_bytes: format!("turn {turn}").into_bytes(),
        });
        if rng.unit() < call_share {
            steps.push(Step::LlmResponse { response_bytes: CALL.to_vec() });
            steps.push(Step::ToolCall { tool: "t".into(), args_bytes: b"{}".to_vec() });
            steps.push(Step::ToolResult { tool: "t".into(), result_bytes: b"ok".to_vec() });
        } else {
            steps.push(Step::LlmResponse { response_bytes: TEXT.to_vec() });
        }
    }
    steps.push(Step::Terminal { outcome: "success".into() });
    Trajectory { run_id: format!("synthetic-{id}"), steps }
}

/// One presence assertion per `LlmResponse`: (request bytes, expects call).
pub fn derive_assertions(t: &Trajectory) -> Vec<(Vec<u8>, bool)> {
    let mut out = Vec::new();
    let mut req: Vec<u8> = Vec::new();
    for (i, s) in t.steps.iter().enumerate() {
        match s {
            Step::LlmRequest { request_bytes } => req = request_bytes.clone(),
            Step::LlmResponse { .. } => {
                let call = matches!(t.steps.get(i + 1), Some(Step::ToolCall { .. }));
                out.push((req.clone(), call));
            }
            _ => {}
        }
    }
    out
}

pub struct FixedCaller(pub bool);

impl ReplayEndpoint for FixedCaller {
    fn invoke(&mut self, _r: &[u8]) -> Vec<u8> {
        if self.0 { CALL.to_vec() } else { TEXT.to_vec() }
    }
}

pub struct RandomCaller {
    rng: Rng,
    p: f64,
}

impl RandomCaller {
    pub fn new(seed: u64, p: f64) -> Self {
        Self { rng: Rng::new(seed), p }
    }
}

impl ReplayEndpoint for RandomCaller {
    fn invoke(&mut self, _r: &[u8]) -> Vec<u8> {
        if self.rng.unit() < self.p { CALL.to_vec() } else { TEXT.to_vec() }
    }
}

/// Pass count over a suite: the caller's presence matches the assertion.
pub fn score(suite: &[(Vec<u8>, bool)], ep: &mut dyn ReplayEndpoint) -> usize {
    suite
        .iter()
        .filter(|(req, want)| (ep.invoke(req) == CALL) == *want)
        .count()
}

#[derive(Serialize, Debug, PartialEq)]
pub struct PassRates {
    pub never_caller: f64,
    pub always_caller: f64,
    pub random_caller: f64,
}

#[derive(Serialize, Debug)]
pub struct ChanceResult {
    pub chance_pass_rate: PassRates,
    pub n_assertions: usize,
    pub observed_call_share: f64,
    pub trajectories: usize,
    pub turns: usize,
    pub call_share: f64,
    pub seed: u64,
}

pub fn run(seed: u64, trajectories: usize, turns: usize, call_share: f64) -> ChanceResult {
    let mut gen = Rng::new(seed);
    let mut suite = Vec::new();
    for i in 0..trajectories {
        suite.extend(derive_assertions(&generate(&mut gen, i, turns, call_share)));
    }
    let n = suite.len();
    let calls = suite.iter().filter(|(_, c)| *c).count();
    let rate = |p: usize| p as f64 / n as f64;
    ChanceResult {
        chance_pass_rate: PassRates {
            never_caller: rate(score(&suite, &mut FixedCaller(false))),
            always_caller: rate(score(&suite, &mut FixedCaller(true))),
            random_caller: rate(score(&suite, &mut RandomCaller::new(seed ^ 0xA5A5, 0.5))),
        },
        n_assertions: n,
        observed_call_share: rate(calls),
        trajectories,
        turns,
        call_share,
        seed,
    }
}
