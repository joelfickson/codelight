# General-purpose Codelight

## Requested outcome

Codelight should code across languages, frameworks, and hosting providers without preferring Vercel. Support configurable OpenAI-compatible model endpoints while preserving existing Gateway configuration.

## Scope

- Replace platform-specific prompt defaults, bundled skills, and offline guidance with general coding workflows.
- Retain the current terminal layout; replace deployment-specific text and remove simulated preview state.
- Configure inference with CODELIGHT_BASE_URL, CODELIGHT_API_KEY, and CODELIGHT_MODEL. Preserve AI_GATEWAY_API_KEY for the default Gateway only. Never forward that key to a custom endpoint.
- Require an explicit model for a custom endpoint. Allow endpoints without authentication when no custom key is supplied.
- Use the selected endpoint for chat, streaming, and model listing. Accept model IDs without a provider prefix.
- Update usage documentation, test behavior, and reinstall the local binary.

## Verification

Run Rust formatting, workspace tests, and Clippy. Test endpoint routing and authentication with a local mock server, including streaming tool calls. Verify the installed binary and existing Gateway connection.

## Not in scope

The rejected HTML designs are not approved for implementation. No UI redesign, native provider-specific protocols, deployment automation, or changes to existing credentials and permission policy.

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
