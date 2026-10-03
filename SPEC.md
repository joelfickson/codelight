# Coding agent reliability stack

## Scope

Three stacked PRs from committed master, developed in an isolated worktree.

1. Failure recovery: complete failed turns, refuse incomplete streamed tool calls, bound network waits, retry transient request failures before consuming responses, and test recovery.
2. Sessions and context: explicit session files and resume, atomic checkpoints after tool results and turns, bounded serialized conversation context retaining whole recent turns, clear overflow errors, and persistence tests. Full transcript remains on disk; context pruning does not claim to summarize or count model tokens.
3. Coding benchmarks: deterministic end-to-end coding fixtures using real file and shell tools, assertions on final files and check results, plus an opt-in live-model runner with machine-readable results.

## Constraints

No UI layout changes, code comments, or edits to the primary checkout. No automatic replay of a failed or interrupted tool call. Session files must be private and must reject mismatched workspaces. PRs target their predecessor and document the merge order.

## Verification

Each PR runs cargo fmt, cargo clippy --workspace --all-targets, and cargo test --workspace. Network protocol tests use local servers. Benchmark fixtures run at small scale before expanding. Live model quality is unverified unless a live evaluation actually runs.

## Implemented validation

The stack implements failure recovery, atomic local session checkpoints, whole-turn byte-bounded context, and the `vc-eval` runner. Workspace tests and Clippy pass. The three scripted coding fixtures pass independent acceptance checks, and a false-success regression fails evaluation as intended. Live-model evaluations have not been run. Context pruning intentionally does not summarize earlier work or guarantee a provider token limit.
