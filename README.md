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
- `agentreplay hash` and `agentreplay inject` CLI.

Not in this commit (openly tracked):

- `agentreplay record --wrap <command>` — a shim that captures a run.
- `agentreplay replay <trajectory>` that drives a live model stub or
  endpoint with the captured prompts and compares its outputs to the
  recorded ones.
- Reference adapters for OpenAI-compatible / Anthropic / local
  llama.cpp endpoints.
- Deterministic seed capture so same trajectory + same seed = same
  continuation.

## Usage (scaffold)

```sh
cargo install --path .
agentreplay hash trajectory.json
agentreplay inject trajectory.json --at 3 --replacement step.json modified.json
```

## License

MIT. See `LICENSE`.
