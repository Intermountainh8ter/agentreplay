//! Cut-point replay floor. The first `cut` turns of a synthetic trajectory
//! are served from the record; the remaining turns come from a degenerate
//! stub. A trajectory diverges when its tool-call presence sequence differs
//! from the record. Model-free; synthetic data.

use crate::chance::{derive_assertions, generate, FixedCaller, RandomCaller, Rng, CALL};
use crate::ReplayEndpoint;
use serde::Serialize;
use std::collections::BTreeMap;

/// True when the cut replay's presence sequence differs from the record.
pub fn diverges(suite: &[(Vec<u8>, bool)], cut: usize, ep: &mut dyn ReplayEndpoint) -> bool {
    suite
        .iter()
        .skip(cut)
        .any(|(req, want)| (ep.invoke(req) == CALL) != *want)
}

#[derive(Serialize, Debug)]
pub struct Shares {
    pub never_divergence_share: BTreeMap<String, f64>,
    pub always_divergence_share: BTreeMap<String, f64>,
    pub random_divergence_share: BTreeMap<String, f64>,
}

#[derive(Serialize, Debug)]
pub struct CutpointResult {
    pub cutpoint: Shares,
    pub cuts: Vec<usize>,
    pub trajectories: usize,
    pub turns: usize,
    pub call_share: f64,
    pub seed: u64,
}

pub fn run(
    seed: u64,
    trajectories: usize,
    turns: usize,
    call_share: f64,
    cuts: &[usize],
) -> CutpointResult {
    let mut gen = Rng::new(seed);
    let suites: Vec<_> = (0..trajectories)
        .map(|i| derive_assertions(&generate(&mut gen, i, turns, call_share)))
        .collect();
    let mut s = Shares {
        never_divergence_share: BTreeMap::new(),
        always_divergence_share: BTreeMap::new(),
        random_divergence_share: BTreeMap::new(),
    };
    for &cut in cuts {
        let (mut n, mut a, mut r) = (0usize, 0usize, 0usize);
        // One random stub per cut, advanced across trajectories.
        let mut rnd = RandomCaller::new(seed ^ 0xA5A5 ^ cut as u64, 0.5);
        for suite in &suites {
            n += diverges(suite, cut, &mut FixedCaller(false)) as usize;
            a += diverges(suite, cut, &mut FixedCaller(true)) as usize;
            r += diverges(suite, cut, &mut rnd) as usize;
        }
        let f = |c: usize| c as f64 / trajectories as f64;
        s.never_divergence_share.insert(cut.to_string(), f(n));
        s.always_divergence_share.insert(cut.to_string(), f(a));
        s.random_divergence_share.insert(cut.to_string(), f(r));
    }
    CutpointResult {
        cutpoint: s,
        cuts: cuts.to_vec(),
        trajectories,
        turns,
        call_share,
        seed,
    }
}
