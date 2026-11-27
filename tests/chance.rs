use agentreplay::chance::*;

#[test]
fn degenerate_rates_follow_call_share() {
    let r = run(7, 50, 120, 0.6);
    let c = r.chance_pass_rate;
    assert!((c.always_caller - r.observed_call_share).abs() < 1e-12);
    assert!((c.never_caller - (1.0 - r.observed_call_share)).abs() < 1e-12);
    assert!((0.45..=0.55).contains(&c.random_caller));
}

#[test]
fn deterministic_for_a_seed() {
    assert_eq!(run(3, 5, 20, 0.6).chance_pass_rate, run(3, 5, 20, 0.6).chance_pass_rate);
}

#[test]
fn assertions_one_per_response() {
    let mut g = Rng::new(1);
    let t = generate(&mut g, 0, 10, 0.5);
    assert_eq!(derive_assertions(&t).len(), 10);
}
