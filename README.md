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
