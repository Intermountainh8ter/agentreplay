# agentreplay pre-registration

Locked before the first captured-trajectory corpus is committed.
Append-only.

## 1. Scope

agentreplay captures and replays agent runs where every event is one of
the five `Step` kinds (`LlmRequest`, `LlmResponse`, `ToolCall`,
`ToolResult`, `Terminal`). Runs that use capabilities outside this set
(agent-to-agent handoffs, cross-process state, asynchronous callbacks
with external side effects) are explicitly out of scope and reported as
such in the recorded trajectory.

## 2. The determinism claim

For a trajectory `T` and an endpoint `E`:

**Byte-identical replay** means: feeding `T.steps[k].request_bytes` to
`E` at step `k`, with the same decoding settings and seed `T` records,
produces a response whose bytes equal `T.steps[k].response_bytes` for
every `k` where `T.steps[k]` is `LlmResponse`.

A trajectory that passes byte-identical replay is **reproducible on
endpoint E**. A trajectory that fails is **drifted on E**, and the
report names the first step that diverged.

Determinism is a property of the (trajectory, endpoint) pair, never of
the trajectory alone. The repo refuses to claim "replay works" without
naming the endpoint and its SHA.

## 3. The injection semantics

`inject(T, at, replacement)` returns a *new* trajectory that:

- preserves steps `0..at` byte-identically,
- replaces step `at` with `replacement`,
- drops every step after `at`.

Trajectory hashes update accordingly. Running an endpoint against the
modified trajectory produces a new tail; the tail is appended, and the
resulting full trajectory is independently reproducible.

## 4. What this repo refuses to do

- No lossy capture. If a byte in the request or response is dropped at
  capture time, replay will diverge and the tool will not pretend
  otherwise.
- No "replayed close enough" verdict. Replay either produces identical
  bytes or it is drifted; the specific drifted step is named.
- No numbers in README not produced by committed `results/*.json`.

## 5. Threats to validity

- **Endpoint non-determinism.** Many hosted endpoints are not actually
  deterministic even at `temperature=0` (floating-point parallelism in
  batched decoding, scheduling noise). We report the drifted step and
  attribute to the endpoint, not the trajectory.
- **Hidden state.** Some agents keep state outside the trajectory
  (vector DB caches, HTTP cookies). Replay against a fresh state will
  diverge; the trajectory itself records the state the agent *shows
  it uses*, not state it hides.
- **Clock-dependent tool outputs.** A `now()` call returns different
  bytes on replay. We capture the recorded bytes and surface the
  drift when replay produces different bytes; the recommended
  practice is to inject a frozen clock at capture.
