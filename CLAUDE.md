# Codelight

Codelight is a general-purpose terminal coding agent implemented as an eight-crate Rust workspace. Follow the user's project choices without favoring a language, framework, model vendor, or hosting platform.

## Invariants

1. All inference goes through `gateway`, using OpenAI-compatible Chat Completions with streaming and tool calls. Configure custom endpoints with `CODELIGHT_BASE_URL`, `CODELIGHT_API_KEY`, and `CODELIGHT_MODEL`. The existing Gateway remains the default for backward compatibility. Never forward `AI_GATEWAY_API_KEY` to a custom endpoint.
2. Bundled prompts, tools, and skills must remain useful across stacks. There is no built-in deployment, live preview, or remote log integration. Do not advertise capabilities the agent does not have.
3. Cross-crate contracts live in `types`.
4. `agent` emits `AgentEvent`s without terminal knowledge; `cli` owns rendering.

## Architecture

`cli -> agent -> { gateway, tools, skills, mcp, context, types }`.

The current general-purpose scope is in `SPEC.md`. Historical platform-specific plans do not override it. The trust layer is described in `docs/superpowers/specs/2026-08-29-trust-layer-design.md`.

## Conventions

- Rust edition 2024, resolver 3. Shared dependencies live in `[workspace.dependencies]`.
- No comments or docstrings unless explicitly requested. Preserve existing comments.
- Preserve unrelated work. Stage only task-owned paths.
- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` before handoff.
- Follow the local `AGENTS.md` design approval requirements before changing UI layout.
