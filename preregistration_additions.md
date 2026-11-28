## 2026-11-27T06:27:50Z chance-floor proposal:8ebba47da5cf449385ba82f07e77f65dd361593eaaff3534dc065c9d2954e12a

Hypothesis: on seeded synthetic trajectories (120 turns, call share 0.6, 200 trajectories, seed 7), a seeded random caller (p=0.5) passes a share of tool-call-presence assertions within 0.45-0.55; never-calls and always-calls pass the complement and the share of calls respectively.

Metric: `/chance_pass_rate/random_caller`. Expected: increases to the 0.45-0.55 range.

Falsifier: random_caller pass rate outside 0.45-0.55, or never/always rates differing from the generated suite's complement/share of calls by more than 0.03.

## 2026-11-27T15:26:39Z cutpoint-floor proposal:5df9b6c84cade9cdbc37c87a072eb5ad0fb966046d16eae904eb1685464a7a31

Hypothesis: on 200 seeded synthetic trajectories of 120 turns (call share 0.6, seed 7), a seeded random p=0.5 stub serving the last 4 turns after a cut at turn 116 changes the tool-call sequence in 0.88 to 0.98 of trajectories, and the divergence share is non-decreasing as the cut moves earlier.

Metric: `/cutpoint/random_divergence_share/116`. Expected: at least 0.88 (analytic expectation 1-0.5^4).

Falsifier: random-caller divergence share at cut 116 outside 0.88-0.98, or a later cut showing a higher share than an earlier cut for the random caller.

## 2026-11-27T18:44:55Z consistency-floor proposal:0a83c0712d01d8462e9aecd28be714a7216ce46f6e0bd2fa88fd9995e0f1267d

Hypothesis: for a uniform random caller with 5 tools, 4 argument values, 3 calls per run, 10 repeated runs per task and 4000 tasks (seed 7), the mean pairwise structural consistency (same tool sequence) is within 0.003 of the closed form 5^-3 = 0.008, and the mean pairwise argument consistency (same tools and arguments) is within 0.0005 of 20^-3 = 0.000125.

Metric: `/consistency_floor/structural_consistency_random_caller`. Expected: within 0.003 of 0.008 (range 0.005-0.011).

Falsifier: simulated structural consistency outside 0.005-0.011, or an absolute gap to the closed form above 0.003.

## 2026-11-27T22:37:21Z emptyfloor proposal:f85f9b89afa9e94b1e6bc588d542c12ffee6989277db7081455e8a6ae6cd7efc

Hypothesis: on 4000 seeded synthetic tasks (seed 7) with an empty-answer-accepting share of 0.2 and an unsolvable share of 0.38, an input-oblivious empty-answer policy succeeds on a share within 0.02 of 0.2, and a constant policy that never matches a specific answer succeeds on no more than that share plus 0.02, so the floor is set by the benchmark's accept share and not by the policy.

Metric: `/floor/constant_policy_success_rate`. Expected: within 0.02 of 0.2 (range 0.18-0.22).

Falsifier: constant-policy success rate outside 0.18-0.22, or the random policy exceeding the empty-answer policy by more than 0.02.

## 2026-11-28T01:31:52Z ordersweep proposal:3e59d744379dcca0a873ad90b02a3be50d64aa38bf85c1e1a6710bdd90b8137f

Hypothesis: with 200 synthetic trajectories of 120 turns, per-uncached-step flip probability 0.005 and seed 7, the share of trajectories whose replay drifts falls monotonically as the cached-step share rises from 0 to 1, and the range (divergence at cache share 0 minus at cache share 1) is between 0.35 and 0.55, with 0 divergence at share 1.

Metric: `/order_sweep/divergence_rate_range`. Expected: about 0.45, decreasing divergence with cache share (range 0.35-0.55).

Falsifier: a non-monotone divergence curve across cache shares, a range outside 0.35 to 0.55, or nonzero divergence at cache share 1.

## 2026-11-28T04:11:36Z perturb proposal:915e27be209dc8cd58e66060aefeb421c256f7b504c48c35f9dade85a1ef9cd0

Hypothesis: with a stub that changes its next response with probability 0.3 when its request bytes differ from the record, over 200 seeded 120-turn trajectories with one perturbed tool result each (seed 7), the share of trajectories whose replay flags drift at the first affected LlmResponse is between 0.2 and 0.4, while the position-keyed RecordedEndpoint flags 0.0 of them.

Metric: `/divergence/first_step_detection_rate`. Expected: about 0.3 (range 0.2-0.4); recorded-endpoint detection 0.0.

Falsifier: first_step_detection_rate outside 0.2 to 0.4 for the request-sensitive stub, or any detection by the position-keyed recorded endpoint.
