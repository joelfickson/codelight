---
name: verify-changes
description: Review and validate code changes before handoff using repository-specific tests, lint, and build checks.
---

Inspect the diff and project instructions. Identify the affected behavior, callers, configuration, and public interfaces. Preserve unrelated edits.

Discover checks from the repository’s manifests, build files, CI workflows, and documentation. Run the narrowest relevant tests first, followed by required lint, type checks, and builds. Do not assume a web framework or deployment provider.

Check for regressions, missing error handling, accidental secret exposure, and unsupported claims. Do not print credentials or modify deployment settings.

Report what changed, which commands passed or failed, and any unverified behavior. Commit, publish, or deploy only when the user requests it.
