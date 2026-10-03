# codelight

## Sessions and context

Interactive sessions are checkpointed automatically to the platform's local data directory under `codelight/sessions`. The session path appears in the conversation. Choose a path with `codelight --session /path/to/session.json`, and resume it from the original project directory with `codelight --resume /path/to/session.json`.

Session files contain the full conversation, including file contents and command output returned to the model. Files are created with owner-only permissions on Unix. Keep them outside repositories. Checkpoints use atomic replacement and an operating-system file lock prevents concurrent writers; the adjacent `.lock` file can remain after exit and does not itself indicate an active lock.

Resume uses the current system instructions, selected model, skills, and permission policy. An action interrupted before its result was saved is marked as uncertain: the model must inspect the project before deciding whether to retry. Saving a conversation does not roll back filesystem changes or resume shell processes.

`--context-bytes` limits the serialized messages plus tool definitions sent per request (default 262144 bytes). This is a byte budget, not a model token count. Complete older turns are omitted as needed, while the full transcript remains on disk. The current turn and its tool results stay together. If the current turn alone exceeds the budget, the turn ends with an error; increase the budget or start a fresh session. This does not perform model-generated summarization.

`--max-steps` sets the maximum number of model requests per turn (default 20). Reaching it reports an error while preserving the session for a follow-up prompt.
