use agentreplay::ordersweep::*;

#[test]
fn full_cache_never_diverges() {
    let r = run(7, 50, 40, 0.2, &[1.0]);
    assert_eq!(r.order_sweep.divergence_rate["1"], 0.0);
}

#[test]
fn no_cache_zero_flip_never_diverges() {
    let r = run(7, 50, 40, 0.0, &[0.0]);
    assert_eq!(r.order_sweep.divergence_rate["0"], 0.0);
}

#[test]
fn no_cache_certain_flip_always_diverges_at_step_zero() {
    let r = run(7, 20, 10, 1.0, &[0.0]);
    assert_eq!(r.order_sweep.divergence_rate["0"], 1.0);
    assert_eq!(r.order_sweep.mean_first_drift_step["0"], 0.0);
}

#[test]
fn divergence_falls_with_cache_share_near_analytic() {
    let r = run(7, 400, 60, 0.01, &[0.0, 0.5, 1.0]);
    let m = &r.order_sweep.divergence_rate;
    assert!(m["0"] > m["0.5"] && m["0.5"] > m["1"]);
    // 1 - 0.99^60 = 0.453
    assert!((m["0"] - 0.453).abs() < 0.08);
    assert!((r.order_sweep.divergence_rate_range - m["0"]).abs() < 1e-12);
}

#[test]
fn deterministic_for_a_seed() {
    let a = run(3, 20, 20, 0.05, &[0.0, 0.5]);
    let b = run(3, 20, 20, 0.05, &[0.0, 0.5]);
    assert_eq!(a.order_sweep.divergence_rate, b.order_sweep.divergence_rate);
}
