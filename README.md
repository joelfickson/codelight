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

## Sessions and context

Interactive sessions are checkpointed automatically to the platform's local data directory under `codelight/sessions`. The session path appears in the conversation. Choose a path with `codelight --session /path/to/session.json`, and resume it from the original project directory with `codelight --resume /path/to/session.json`.

Session files contain the full conversation, including file contents and command output returned to the model. Files are created with owner-only permissions on Unix. Keep them outside repositories. Checkpoints use atomic replacement and an operating-system file lock prevents concurrent writers; the adjacent `.lock` file can remain after exit and does not itself indicate an active lock.

Resume uses the current system instructions, selected model, skills, and permission policy. An action interrupted before its result was saved is marked as uncertain: the model must inspect the project before deciding whether to retry. Saving a conversation does not roll back filesystem changes or resume shell processes.

`--context-bytes` limits the serialized messages plus tool definitions sent per request (default 262144 bytes). This is a byte budget, not a model token count. Complete older turns are omitted as needed, while the full transcript remains on disk. The current turn and its tool results stay together. If the current turn alone exceeds the budget, the turn ends with an error; increase the budget or start a fresh session. This does not perform model-generated summarization.

`--max-steps` sets the maximum number of model requests per turn (default 20). Reaching it reports an error while preserving the session for a follow-up prompt.

## Coding evaluations

Run a single fixture first:

```sh
cargo run -p eval -- --case boundary
```

Run all fixtures with the same pipeline:

```sh
cargo run -p eval
```

The default uses scripted model responses through the actual agent loop, file tools, command tool, and session checkpoints. These runs test the coding pipeline; they do not measure a model's reasoning quality. The tasks cover an inclusive-range bug, a two-file function rename, and recovery after a failing test. Acceptance requires a completed agent turn, a source change, an observed successful check, and a separate successful acceptance-test run. The recovery task additionally requires an observed failed check. A final answer claiming success cannot satisfy these conditions.

Each case emits one JSON line containing the mode, model when live, pass/fail, check exit codes, tool-call count, model-step count, elapsed milliseconds, and any agent error. Failed cases make the process exit nonzero. Fixtures are isolated temporary Rust packages with no external dependencies and are deleted afterward.

To measure the configured model endpoint, explicitly opt in:

```sh
cargo run -p eval -- --live --case boundary
cargo run -p eval -- --live
```

Configure `CODELIGHT_BASE_URL`, `CODELIGHT_MODEL`, and optionally `CODELIGHT_API_KEY` for a custom endpoint, or `AI_GATEWAY_API_KEY` for the default Gateway; the evaluation runner does not load `.env` files. Live runs make paid model requests and compile and execute generated Rust on the host. The harness restricts tool file writes to fixture source files and shell calls to the fixture test command, but it is not an execution sandbox. Use a disposable environment for untrusted models. No live-model quality claim follows from passing scripted tests.

## Sign in with ChatGPT

On macOS and Linux, connect a ChatGPT account using browser sign-in:

```sh
codelight login
codelight --provider chatgpt --list-models
codelight --provider chatgpt --check
codelight --provider chatgpt
```

Approve Codelight's access and ChatGPT plan usage in the browser. Availability and usage limits depend on your account and OpenAI's preview eligibility. Review or revoke access in [ChatGPT usage settings](https://chatgpt.com/settings/usage).

ChatGPT mode discovers the models available to the selected account. It uses the first visible model unless you pass `--model <model-id>`. The provider is explicit: logging in does not change existing API-key connections. ChatGPT mode ignores `CODELIGHT_API_KEY`, `CODELIGHT_BASE_URL`, and `CODELIGHT_MODEL`; OAuth credentials only go to official OpenAI endpoints.

Manage accounts with:

```sh
codelight accounts
codelight login --new
codelight accounts --select <account-id>
codelight login --account <account-id>
codelight logout
```

`logout --account <account-id>` disconnects a specific account. Logout clears local credentials and attempts server revocation, while retaining the client registration for future sign-ins. If revocation cannot be confirmed, the CLI reports it. A running session keeps its original account even when another process selects a different account.

Credentials are stored outside the repository in Codelight's platform configuration directory, under `chatgpt/accounts.json`, with owner-only permissions. On macOS this is `~/Library/Application Support/codelight/chatgpt/`; on Linux it is `$XDG_CONFIG_HOME/codelight/chatgpt/` or `~/.config/codelight/chatgpt/`. Token refreshes are serialized across processes. Windows credential storage is not implemented yet.

The ChatGPT backend uses streamed Responses requests with `store: false`. Saved sessions retain response items, including opaque reasoning context, for tool-call continuation. Failed or incomplete responses never execute pending tools. Session files contain conversation content and should remain private.
