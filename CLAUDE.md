# Codelight

Codelight is fundamentally a **general-purpose coding agent** that **prefers Vercel
technologies** (Next.js, the Vercel AI SDK, Vercel platform conventions) and routes **all
inference through the Vercel AI Gateway**. It is a terminal TUI (Rust, 8-crate Cargo
workspace) that writes code, type-checks, deploys previews, and inspects logs in one session.
The bet: a session ends with a live preview URL, not just a saved file.

## Invariants (do not break)

1. **All inference routes exclusively through the Vercel AI Gateway.**
   `vc-gateway` is the single inference chokepoint (`https://ai-gateway.vercel.sh`,
   OpenAI-compatible). Everything that touches a model - main chat, task classification,
   and (v0.3) embeddings - goes through it. No direct provider SDKs anywhere (no `anthropic`,
   `openai`, `google-*` clients); no crate calls a model except via `vc-gateway`.

2. **Coding agent first, Vercel specialization second.**
   The core (`vc-agent` + `vc-tools`) must work as a capable general coding agent on its own.
   Vercel is a specialization *layer* (`vc-skills`, `vc-context`, `vc-mcp`), not the
   foundation. The agent degrades gracefully when the Vercel platform is absent: all
   filesystem/shell tools still work.

3. **`vc-types` is the shared contract.** All cross-crate shapes (wire formats, the
   `StreamEvent`/`AgentEvent` event bus) live there; don't leak ad-hoc structs between crates.

4. **The renderer is pluggable.** `vc-agent` emits `AgentEvent`s over an mpsc channel with
   zero terminal knowledge; only `vc-cli` renders. Keep that seam clean.

## Architecture

`vc-cli -> vc-agent -> { vc-gateway, vc-tools, vc-skills, vc-mcp, vc-context, vc-types }`,
everyone depends on `vc-types`. Dependencies flow one direction.

The approved phased plan (v0.1-v0.4, walking-skeleton-first) is at
`~/.claude/plans/create-a-plan-for-memoized-hartmanis.md`. Full design is in Notion
(page id `38ddc33ac53f81fe8c2ddef16efbe3b5`).

## Conventions

- Rust edition 2024, `resolver = "3"`. Shared deps pinned once in `[workspace.dependencies]`;
  crates use `{ workspace = true }`.
- Self-documenting code: no comments/docstrings unless asked. No emojis or em dashes in code.
- Before commit: `cargo fmt`, `cargo clippy --workspace --all-targets` (clean), `cargo test --workspace`.
