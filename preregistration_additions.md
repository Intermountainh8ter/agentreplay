## 2026-11-27T06:27:50Z chance-floor proposal:8ebba47da5cf449385ba82f07e77f65dd361593eaaff3534dc065c9d2954e12a

Hypothesis: on seeded synthetic trajectories (120 turns, call share 0.6, 200 trajectories, seed 7), a seeded random caller (p=0.5) passes a share of tool-call-presence assertions within 0.45-0.55; never-calls and always-calls pass the complement and the share of calls respectively.

Metric: `/chance_pass_rate/random_caller`. Expected: increases to the 0.45-0.55 range.

Falsifier: random_caller pass rate outside 0.45-0.55, or never/always rates differing from the generated suite's complement/share of calls by more than 0.03.
