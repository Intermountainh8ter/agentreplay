# agentreplay

**Deterministic trajectory format and replay/injection engine for LLM
agents.** Capture a run, rewind to any step, inject a different tool
result or model response, continue — all with byte-identical replay
semantics. Trajectory steps are content-addressed by a chained SHA-256
so any divergence is immediately detectable.

![status: initial scaffold](https://img.shields.io/badge/status-initial%20scaffold-lightgrey)
![language: Rust](https://img.shields.io/badge/language-Rust-blue)
![license: MIT](https://img.shields.io/badge/license-MIT-blue)

## Why

Agent debuggers today are tracing (LangSmith, Phoenix) or screen
recording. There is no replay + inject + continue tool that treats a
trajectory as a replayable program. Every working practitioner has
rebuilt a bad version of this in-house.

## What is in this scaffold

- `Step` enum (`LlmRequest`, `LlmResponse`, `ToolCall`, `ToolResult`,
  `Terminal`) with byte-identical serialization.
- `Trajectory` with `step_hashes` and `trajectory_hash` content-chain
  hashing.
- `inject(t, at, replacement)` — pre-serve step `at`, drop the tail.
- `ReplayEndpoint` trait: implementors return response bytes for a
  given request-bytes. HTTP-free; this crate never pulls a client.
- `RecordedEndpoint::from_trajectory(&t)` — a stub endpoint that
  replays the trajectory's own captured LlmResponse bytes in order.
  Primary use: a reproducibility smoke test for the plumbing.
- `replay(&t, &mut endpoint) -> ReplayReport`: pre-reg §2 byte-identical
  replay. Walks the trajectory, invokes the endpoint on every
  LlmRequest, compares its output to the next LlmResponse, stops at
  the first byte mismatch and names the drifted step (pre-reg §4:
  no "replayed close enough").
- `agentreplay hash`, `agentreplay inject`, `agentreplay replay` CLIs.
  `replay` exits 1 on drift so pipelines can gate.

Not in this commit (openly tracked):

- `agentreplay record --wrap <command>` — a shim that captures a run.
- Reference adapters for OpenAI-compatible / Anthropic / local
  llama.cpp endpoints (built on `ReplayEndpoint`).
- Deterministic seed capture so same trajectory + same seed = same
  continuation on a live endpoint.

## Usage (scaffold)

```sh
cargo install --path .
agentreplay hash trajectory.json
agentreplay inject trajectory.json --at 3 --replacement step.json modified.json
agentreplay replay trajectory.json  # reproducibility smoke test
```

## License

MIT. See `LICENSE`.

## Chance floor

`agentreplay chance` scores never-calls, always-calls and seeded random callers
against tool-call-presence assertions derived from synthetic trajectories.
In `results/636-chance-floor.json` the random caller passes 0.4975 of 24000
assertions, never-calls 0.3956 and always-calls 0.6044 (call share 0.6).
Reproduce: `cargo run --locked -q --release -- chance --seed 7 --trajectories 200 --turns 120 --call-share 0.6 --out results/636-chance-floor.json`

## Cut-point floor

`agentreplay cutpoint` serves the first k turns of synthetic trajectories from
the record and the rest from a never-calls, always-calls or seeded random stub,
and reports the share of trajectories whose tool-call sequence diverges.
In `results/653-cutpoint-floor.json` the random stub diverges in 0.925 of 200
trajectories at cut 116 (last 4 of 120 turns), 0.73 at cut 118 and 1.0 at cut 100.
Reproduce: `cargo run --locked -q --release -- cutpoint --seed 7 --trajectories 200 --turns 120 --call-share 0.6 --cuts 0,60,100,110,114,116,118,120 --out results/653-cutpoint-floor.json`

## Consistency floor

`agentreplay consistency` runs a uniform random caller R times per task and
reports pairwise structural consistency (same tool sequence) and argument
consistency (same tools and arguments) against the closed forms T^-L and (T*A)^-L.
In `results/690-consistency-floor.json` (5 tools, 4 argument values, 3 calls, 10 runs,
4000 tasks) the random caller scores 0.007794444444444444 structural against
0.008 closed form, and 0.0001388888888888889 argument against 0.000125.
Reproduce: `cargo run --locked -q --release -- consistency --seed 7 --tasks 4000 --runs 10 --calls 3 --tools 5 --arg-values 4 --out results/690-consistency-floor.json`

## Empty-answer floor

`agentreplay emptyfloor` scores empty, constant and seeded random policies on synthetic tasks where a share of graders accept any output and a share of tasks is unsolvable.
In `results/729-emptyfloor.json` the empty and constant policies each succeed on 0.2005 of 4000 tasks, equal to the observed accepting share (0.2005), and the random policy on 0.201.
Reproduce: `cargo run --locked -q --release -- emptyfloor --seed 7 --tasks 4000 --accept-share 0.2 --unsolvable-share 0.38 --out results/729-emptyfloor.json`

## Order sweep

`agentreplay ordersweep` serves synthetic trajectories through a stub whose uncached steps flip a tool-call decision with a set probability, and sweeps the cached-step share (a stand-in for replay order and prompt-cache state).
In `results/769-ordersweep.json` the share of 200 trajectories whose replay drifts is 0.47 at cache share 0, 0.41 at 0.25, 0.26 at 0.5, 0.095 at 0.75 and 0.0 at 1 (range 0.47, flip probability 0.005, 120 turns).
Reproduce: `cargo run --locked -q --release -- ordersweep --seed 7 --trajectories 200 --turns 120 --flip-prob 0.005 --cache-shares 0,0.25,0.5,0.75,1 --out results/769-ordersweep.json`

## Perturbation divergence

`agentreplay perturb` changes one recorded tool result per synthetic trajectory, carries the change into the next request, and replays against a request-sensitive stub (changes its response with probability 0.3 on a differing request) and against the position-keyed recorded endpoint.
In `results/808-perturb.json` the request-sensitive stub flags drift at the first affected response in 0.305 of 200 trajectories (61 of 200), and the recorded endpoint flags 0.0.
Reproduce: `cargo run --locked -q --release -- perturb --seed 7 --trajectories 200 --turns 120 --sensitivity 0.3 --out results/808-perturb.json`
