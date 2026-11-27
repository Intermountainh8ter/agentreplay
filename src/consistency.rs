//! Consistency floor of repeated tool-calling runs.
//!
//! A uniform random caller picks, for each of `calls` calls, one of `tools`
//! tools and one of `arg_values` argument values. Each task is run `runs`
//! times; pairwise structural consistency (same tool sequence) and argument
//! consistency (same tools and arguments) are compared to the closed forms
//! `tools^-calls` and `(tools*arg_values)^-calls`. Model-free; synthetic.

use crate::chance::Rng;
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
pub struct Floor {
    pub structural_consistency_random_caller: f64,
    pub argument_consistency_random_caller: f64,
    pub structural_closed_form: f64,
    pub argument_closed_form: f64,
    pub structural_abs_gap: f64,
    pub argument_abs_gap: f64,
    pub pairs: usize,
}

#[derive(Serialize, Debug)]
pub struct ConsistencyResult {
    pub consistency_floor: Floor,
    pub tasks: usize,
    pub runs: usize,
    pub calls: usize,
    pub tools: usize,
    pub arg_values: usize,
    pub seed: u64,
}

pub fn run(
    seed: u64,
    tasks: usize,
    runs: usize,
    calls: usize,
    tools: usize,
    arg_values: usize,
) -> ConsistencyResult {
    let mut rng = Rng::new(seed);
    let (mut s_same, mut a_same, mut pairs) = (0usize, 0usize, 0usize);
    for _ in 0..tasks {
        let rs: Vec<Vec<(usize, usize)>> = (0..runs)
            .map(|_| {
                (0..calls)
                    .map(|_| {
                        let t = (rng.next_u64() % tools as u64) as usize;
                        let a = (rng.next_u64() % arg_values as u64) as usize;
                        (t, a)
                    })
                    .collect()
            })
            .collect();
        for i in 0..runs {
            for j in (i + 1)..runs {
                pairs += 1;
                if rs[i].iter().zip(&rs[j]).all(|(x, y)| x.0 == y.0) {
                    s_same += 1;
                }
                if rs[i] == rs[j] {
                    a_same += 1;
                }
            }
        }
    }
    let n = pairs.max(1) as f64;
    let (s, a) = (s_same as f64 / n, a_same as f64 / n);
    let sc = (tools as f64).powi(-(calls as i32));
    let ac = ((tools * arg_values) as f64).powi(-(calls as i32));
    ConsistencyResult {
        consistency_floor: Floor {
            structural_consistency_random_caller: s,
            argument_consistency_random_caller: a,
            structural_closed_form: sc,
            argument_closed_form: ac,
            structural_abs_gap: (s - sc).abs(),
            argument_abs_gap: (a - ac).abs(),
            pairs,
        },
        tasks,
        runs,
        calls,
        tools,
        arg_values,
        seed,
    }
}
