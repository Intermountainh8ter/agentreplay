use agentreplay::cutpoint::*;

#[test]
fn cut_at_end_never_diverges() {
    let r = run(7, 30, 40, 0.6, &[40]);
    assert_eq!(r.cutpoint.random_divergence_share["40"], 0.0);
    assert_eq!(r.cutpoint.never_divergence_share["40"], 0.0);
}

#[test]
fn full_replay_by_degenerate_stub_diverges_everywhere() {
    let r = run(7, 30, 40, 0.6, &[0]);
    assert_eq!(r.cutpoint.never_divergence_share["0"], 1.0);
    assert_eq!(r.cutpoint.always_divergence_share["0"], 1.0);
}

#[test]
fn random_share_non_decreasing_toward_earlier_cuts_and_near_analytic() {
    let r = run(7, 400, 30, 0.6, &[0, 20, 26, 28, 30]);
    let m = &r.cutpoint.random_divergence_share;
    assert!(m["0"] >= m["20"] && m["20"] >= m["26"] && m["26"] >= m["28"] && m["28"] >= m["30"]);
    assert!((m["28"] - 0.75).abs() < 0.08);
}

#[test]
fn deterministic_for_a_seed() {
    let a = run(3, 10, 20, 0.6, &[10, 18]);
    let b = run(3, 10, 20, 0.6, &[10, 18]);
    assert_eq!(a.cutpoint.random_divergence_share, b.cutpoint.random_divergence_share);
}
