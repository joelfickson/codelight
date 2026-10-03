# Codelight

A general-purpose AI coding agent for your terminal. Explore, edit, debug, and verify projects using their existing languages, frameworks, and tools.

## Install

Use a current stable Rust toolchain:

```sh
cargo install --path crates/cli --locked
```

Run `codelight` from the project you want to work on. `codelight --demo` shows an offline sample session without changing files or calling a model.

## Model connection

Codelight uses the OpenAI-compatible Chat Completions protocol, including streaming and tool calls. Inference configuration is independent of the project's stack or deployment platform.

| Setting | Purpose |
| --- | --- |
| `CODELIGHT_BASE_URL` | API base URL, including the API prefix such as `/v1`. Do not include `/chat/completions`. |
| `CODELIGHT_API_KEY` | Bearer credential for the configured endpoint. Optional for custom endpoints that do not require authentication. |
| `CODELIGHT_MODEL` | Exact model ID accepted by the endpoint. Required for custom endpoints unless supplied with `--model`. |
| `AI_GATEWAY_API_KEY` | Backward-compatible credential for the default Vercel AI Gateway only. Never used for a custom endpoint. |

Codelight reads environment variables and loads a `.env` from the current directory or its ancestors. Existing environment variables take precedence. Keep credentials out of source control.

With no `CODELIGHT_BASE_URL`, the existing Gateway connection remains the default and uses `anthropic/claude-sonnet-4-6`. `CODELIGHT_API_KEY` takes precedence over `AI_GATEWAY_API_KEY` for that connection. There is no Vercel preference in the coding instructions, bundled skills, or tools.

For a local OpenAI-compatible server, set its actual base URL and model ID. For example, if your server listens on port 1234 and serves a model named `my-local-model`:

```sh
export CODELIGHT_BASE_URL=http://127.0.0.1:1234/v1
export CODELIGHT_MODEL=my-local-model
codelight --check
codelight
```

For an authenticated remote endpoint, set `CODELIGHT_API_KEY` securely in your environment as well. Select a model and endpoint that support Chat Completions streaming and function tool calls. Provider-specific native APIs and the Responses API are not supported. Some compatible servers may not support model listing or the requested streaming usage options.

```sh
codelight --check
codelight --list-models
codelight --model your-model-id
```

Inside a session, `/model` opens the model picker; `/model your-model-id` selects an exact ID. IDs do not need a provider prefix. Press Esc or Ctrl+C to exit.

## Tools and skills

Codelight can read, search, write, edit, move, and delete files; run shell commands; fetch web pages; and load task-specific skills. The bundled skills cover codebase exploration, debugging and regression testing, and change verification. `search_docs` searches general offline workflow guidance, not current framework documentation.

Custom skills load from `~/.codelight/skills`, `~/.claude/skills`, `.codelight/skills`, and `.claude/skills`. Each skill lives in its own directory with a `SKILL.md` file. The old `.vercelcode/skills` location is no longer loaded; move skills you want to retain into `.codelight/skills`. Skills installed through the skill tool continue to use `.claude/skills` for compatibility with its installer.

Command permissions are configured in `.codelight.toml`. The existing approval policy remains in place. `--yolo` bypasses approval prompts. No deployment or preview integration is built in.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
