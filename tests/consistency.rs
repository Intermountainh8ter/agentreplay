use agentreplay::consistency::run;

#[test]
fn matches_closed_form() {
    let r = run(7, 2000, 10, 3, 5, 4).consistency_floor;
    assert!(r.structural_abs_gap < 0.003);
    assert!(r.argument_abs_gap < 0.0005);
    assert_eq!(r.pairs, 2000 * 45);
}

#[test]
fn single_tool_is_fully_structural() {
    let r = run(1, 10, 4, 3, 1, 2).consistency_floor;
    assert_eq!(r.structural_consistency_random_caller, 1.0);
}

#[test]
fn deterministic_for_a_seed() {
    assert_eq!(run(3, 20, 5, 3, 5, 4).consistency_floor, run(3, 20, 5, 3, 5, 4).consistency_floor);
}
