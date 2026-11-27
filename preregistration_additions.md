## 2026-11-27T06:27:50Z chance-floor proposal:8ebba47da5cf449385ba82f07e77f65dd361593eaaff3534dc065c9d2954e12a

Hypothesis: on seeded synthetic trajectories (120 turns, call share 0.6, 200 trajectories, seed 7), a seeded random caller (p=0.5) passes a share of tool-call-presence assertions within 0.45-0.55; never-calls and always-calls pass the complement and the share of calls respectively.

Metric: `/chance_pass_rate/random_caller`. Expected: increases to the 0.45-0.55 range.

Falsifier: random_caller pass rate outside 0.45-0.55, or never/always rates differing from the generated suite's complement/share of calls by more than 0.03.

## 2026-11-27T15:26:39Z cutpoint-floor proposal:5df9b6c84cade9cdbc37c87a072eb5ad0fb966046d16eae904eb1685464a7a31

Hypothesis: on 200 seeded synthetic trajectories of 120 turns (call share 0.6, seed 7), a seeded random p=0.5 stub serving the last 4 turns after a cut at turn 116 changes the tool-call sequence in 0.88 to 0.98 of trajectories, and the divergence share is non-decreasing as the cut moves earlier.

Metric: `/cutpoint/random_divergence_share/116`. Expected: at least 0.88 (analytic expectation 1-0.5^4).

Falsifier: random-caller divergence share at cut 116 outside 0.88-0.98, or a later cut showing a higher share than an earlier cut for the random caller.
