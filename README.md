# codelight

## Sessions and context

Interactive sessions are checkpointed automatically to the platform's local data directory under `codelight/sessions`. The session path appears in the conversation. Choose a path with `codelight --session /path/to/session.json`, and resume it from the original project directory with `codelight --resume /path/to/session.json`.

Session files contain the full conversation, including file contents and command output returned to the model. Files are created with owner-only permissions on Unix. Keep them outside repositories. Checkpoints use atomic replacement and an operating-system file lock prevents concurrent writers; the adjacent `.lock` file can remain after exit and does not itself indicate an active lock.

Resume uses the current system instructions, selected model, skills, and permission policy. An action interrupted before its result was saved is marked as uncertain: the model must inspect the project before deciding whether to retry. Saving a conversation does not roll back filesystem changes or resume shell processes.

`--context-bytes` limits the serialized messages plus tool definitions sent per request (default 262144 bytes). This is a byte budget, not a model token count. Complete older turns are omitted as needed, while the full transcript remains on disk. The current turn and its tool results stay together. If the current turn alone exceeds the budget, the turn ends with an error; increase the budget or start a fresh session. This does not perform model-generated summarization.

`--max-steps` sets the maximum number of model requests per turn (default 20). Reaching it reports an error while preserving the session for a follow-up prompt.

## Coding evaluations

Run a single fixture first:

```sh
cargo run -p vc-eval -- --case boundary
```

Run all fixtures with the same pipeline:

```sh
cargo run -p vc-eval
```

The default uses scripted model responses through the actual agent loop, file tools, command tool, and session checkpoints. These runs test the coding pipeline; they do not measure a model's reasoning quality. The tasks cover an inclusive-range bug, a two-file function rename, and recovery after a failing test. Acceptance requires a completed agent turn, a source change, an observed successful check, and a separate successful acceptance-test run. The recovery task additionally requires an observed failed check. A final answer claiming success cannot satisfy these conditions.

Each case emits one JSON line containing the mode, model when live, pass/fail, check exit codes, tool-call count, model-step count, elapsed milliseconds, and any agent error. Failed cases make the process exit nonzero. Fixtures are isolated temporary Rust packages with no external dependencies and are deleted afterward.

To measure the configured Gateway model, explicitly opt in:

```sh
cargo run -p vc-eval -- --live --case boundary
cargo run -p vc-eval -- --live
```

Export `AI_GATEWAY_API_KEY` and optionally `CODELIGHT_MODEL` in the environment first; the evaluation runner does not load `.env` files. Live runs make paid model requests and compile and execute generated Rust on the host. The harness restricts tool file writes to fixture source files and shell calls to the fixture test command, but it is not an execution sandbox. Use a disposable environment for untrusted models. No live-model quality claim follows from passing scripted tests.
