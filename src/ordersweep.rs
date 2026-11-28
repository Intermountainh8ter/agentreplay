//! Order/cache sweep. A synthetic replay serves each turn either from a
//! cached step (exact) or from an uncached step that flips the tool-call
//! decision with a set probability. The cached share stands in for replay
//! order and prompt-cache state. Model-free; synthetic data.

use crate::chance::{derive_assertions, generate, Rng};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Debug)]
pub struct Sweep {
    pub divergence_rate: BTreeMap<String, f64>,
    pub mean_first_drift_step: BTreeMap<String, f64>,
    pub divergence_rate_range: f64,
}

#[derive(Serialize, Debug)]
pub struct OrderSweepResult {
    pub order_sweep: Sweep,
    pub cache_shares: Vec<f64>,
    pub flip_prob: f64,
    pub trajectories: usize,
    pub turns: usize,
    pub seed: u64,
}

/// Index of the first turn whose decision flipped, if any.
pub fn first_drift(
    suite: &[(Vec<u8>, bool)],
    cache_share: f64,
    flip_prob: f64,
    rng: &mut Rng,
) -> Option<usize> {
    let mut first = None;
    for i in 0..suite.len() {
        // Draw both numbers every turn so the stream does not depend on outcomes.
        let cached = rng.unit() < cache_share;
        let flip = rng.unit() < flip_prob;
        if !cached && flip && first.is_none() {
            first = Some(i);
        }
    }
    first
}

pub fn run(
    seed: u64,
    trajectories: usize,
    turns: usize,
    flip_prob: f64,
    cache_shares: &[f64],
) -> OrderSweepResult {
    let mut gen = Rng::new(seed);
    let suites: Vec<_> = (0..trajectories)
        .map(|i| derive_assertions(&generate(&mut gen, i, turns, 0.6)))
        .collect();
    let mut rates = BTreeMap::new();
    let mut firsts = BTreeMap::new();
    let mut ordered: Vec<(f64, f64)> = Vec::new();
    for (k, &share) in cache_shares.iter().enumerate() {
        let mut rng = Rng::new(seed ^ 0x5EED ^ (k as u64 + 1));
        let drifts: Vec<usize> = suites
            .iter()
            .filter_map(|s| first_drift(s, share, flip_prob, &mut rng))
            .collect();
        let rate = drifts.len() as f64 / trajectories as f64;
        let mean = if drifts.is_empty() {
            0.0
        } else {
            drifts.iter().sum::<usize>() as f64 / drifts.len() as f64
        };
        rates.insert(format!("{share}"), rate);
        firsts.insert(format!("{share}"), mean);
        ordered.push((share, rate));
    }
    let lo = ordered.iter().min_by(|a, b| a.0.total_cmp(&b.0));
    let hi = ordered.iter().max_by(|a, b| a.0.total_cmp(&b.0));
    let range = match (lo, hi) {
        (Some(l), Some(h)) => l.1 - h.1,
        _ => 0.0,
    };
    OrderSweepResult {
        order_sweep: Sweep {
            divergence_rate: rates,
            mean_first_drift_step: firsts,
            divergence_rate_range: range,
        },
        cache_shares: cache_shares.to_vec(),
        flip_prob,
        trajectories,
        turns,
        seed,
    }
}
