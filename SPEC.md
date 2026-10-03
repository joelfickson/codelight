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

## Implemented validation

The stack implements failure recovery, atomic local session checkpoints, whole-turn byte-bounded context, and the `eval` runner. Workspace tests and Clippy pass. The three scripted coding fixtures pass independent acceptance checks, and a false-success regression fails evaluation as intended. Live-model evaluations have not been run. Context pruning intentionally does not summarize earlier work or guarantee a provider token limit.

# ChatGPT sign-in integration

## Requested extension

Add Sign in with ChatGPT alongside existing API-key connections. Use the official open-source ChatGPT plan-usage flow and the public Responses API. Keep the current terminal layout.

## Implementation plan

1. Add authentication support under the gateway crate: persistent installation host ID, separate account registrations, browser authorization with fresh state, nonce, and S256 PKCE, and a loopback callback at /auth/callback.
2. Retain the issued client ID before code exchange. Validate ID-token signature, issuer, audience, expiry, nonce, and returning-account identity before activating credentials. Require granted plan-usage scope before inference.
3. Store credentials atomically outside repositories with owner-only permissions. Serialize refreshes across processes. Preserve registration identity through logout, attempt remote session revocation, and clearly report unconfirmed revocation.
4. Add login, logout, and account selection commands. Keep provider selection explicit so logging in does not silently reroute an existing API-key session.
5. Add a Responses backend with account-specific model discovery, streaming text and function calls, tool-result submission, and retained response items needed for subsequent turns. Require response.completed; reject failed, incomplete, malformed, and interrupted streams before executing pending tools.
6. Route ChatGPT OAuth credentials only to the official OpenAI endpoints. Retain the current Chat Completions backend for API-key connections.
7. Test callback validation, registration reuse, account isolation, token refresh, storage permissions, logout, model discovery, tool-call round trips, and interrupted streams against local fixtures. Run workspace tests, Clippy, and formatting, then reinstall the CLI. Live authentication requires the user to complete browser consent.

## Current integration dependency

The user authorized continuing with the existing work. Integrate against the latest session-recovery implementation and preserve its behavior.

## Official references

- https://developers.openai.com/siwc/token-sharing-open-source/sign-in
- https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions
- https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference
- https://developers.openai.com/siwc/token-sharing-open-source/token-reference

## ChatGPT validation status

Implemented browser PKCE login, validated identity tokens, account selection, protected credential storage, serialized rotating-token refresh, logout, and a separate Responses backend. Response items persist in saved sessions and are excluded from Chat Completions requests. Workspace tests and strict Clippy pass; targeted tests cover signed identity validation, concurrent refresh, callback validation, account isolation, permissions, response continuation, incomplete streams, and CLI parsing. Live browser consent, account eligibility, server revocation, and live inference remain unverified. Credential storage currently supports Unix platforms only.
