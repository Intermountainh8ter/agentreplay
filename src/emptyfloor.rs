//! Empty-answer floor of a task suite.
//!
//! Each synthetic task has a grader that either accepts any output (an
//! accepting grader) or accepts one specific answer out of `ANSWER_SPACE`.
//! A share of tasks is unsolvable: the specific answer is unreachable, so
//! only an accepting grader can pass. Input-oblivious policies (empty,
//! constant, uniform random) are scored. Model-free; synthetic data.

use crate::chance::Rng;
use serde::Serialize;

pub const ANSWER_SPACE: u64 = 1000;

#[derive(Clone, Copy, Debug)]
pub struct Task {
    pub accepts_any: bool,
    pub unsolvable: bool,
    pub answer: u64,
}

pub fn generate(rng: &mut Rng, accept_share: f64, unsolvable_share: f64) -> Task {
    let accepts_any = rng.unit() < accept_share;
    let unsolvable = rng.unit() < unsolvable_share;
    let answer = rng.next_u64() % ANSWER_SPACE;
    Task { accepts_any, unsolvable, answer }
}

/// `None` is the empty answer; `Some(a)` an answer id.
pub fn grade(task: &Task, out: Option<u64>) -> bool {
    if task.accepts_any {
        return true;
    }
    !task.unsolvable && out == Some(task.answer)
}

#[derive(Serialize, Debug)]
pub struct Floor {
    pub empty_policy_success_rate: f64,
    pub constant_policy_success_rate: f64,
    pub random_policy_success_rate: f64,
    pub observed_accept_share: f64,
    pub observed_unsolvable_share: f64,
    pub random_minus_empty: f64,
}

#[derive(Serialize, Debug)]
pub struct EmptyFloorResult {
    pub floor: Floor,
    pub tasks: usize,
    pub accept_share: f64,
    pub unsolvable_share: f64,
    pub answer_space: u64,
    pub seed: u64,
}

pub fn run(seed: u64, tasks: usize, accept_share: f64, unsolvable_share: f64) -> EmptyFloorResult {
    let mut gen = Rng::new(seed);
    let mut pol = Rng::new(seed ^ 0x5A5A);
    let suite: Vec<Task> = (0..tasks)
        .map(|_| generate(&mut gen, accept_share, unsolvable_share))
        .collect();
    let n = tasks as f64;
    let rate = |c: usize| c as f64 / n;
    let empty = suite.iter().filter(|t| grade(t, None)).count();
    // Constant policy: one fixed answer id outside the answer space.
    let constant = suite.iter().filter(|t| grade(t, Some(ANSWER_SPACE))).count();
    let random = suite
        .iter()
        .filter(|t| grade(t, Some(pol.next_u64() % ANSWER_SPACE)))
        .count();
    let (e, c, r) = (rate(empty), rate(constant), rate(random));
    EmptyFloorResult {
        floor: Floor {
            empty_policy_success_rate: e,
            constant_policy_success_rate: c,
            random_policy_success_rate: r,
            observed_accept_share: rate(suite.iter().filter(|t| t.accepts_any).count()),
            observed_unsolvable_share: rate(suite.iter().filter(|t| t.unsolvable).count()),
            random_minus_empty: r - e,
        },
        tasks,
        accept_share,
        unsolvable_share,
        answer_space: ANSWER_SPACE,
        seed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_per_seed() {
        let a = run(7, 500, 0.2, 0.38);
        let b = run(7, 500, 0.2, 0.38);
        assert_eq!(a.floor.constant_policy_success_rate, b.floor.constant_policy_success_rate);
    }

    #[test]
    fn floor_equals_accept_share_when_nothing_else_passes() {
        let r = run(3, 2000, 0.2, 1.0);
        assert_eq!(r.floor.empty_policy_success_rate, r.floor.observed_accept_share);
        assert_eq!(r.floor.constant_policy_success_rate, r.floor.observed_accept_share);
        assert_eq!(r.floor.random_policy_success_rate, r.floor.observed_accept_share);
    }

    #[test]
    fn zero_accept_share_gives_zero_constant_floor() {
        let r = run(3, 2000, 0.0, 0.38);
        assert_eq!(r.floor.constant_policy_success_rate, 0.0);
        assert_eq!(r.floor.empty_policy_success_rate, 0.0);
    }

    #[test]
    fn random_never_below_empty() {
        let r = run(7, 4000, 0.2, 0.38);
        assert!(r.floor.random_minus_empty >= 0.0);
    }
}
