---
name: debug-and-test
description: Diagnose bugs and failing tests, fix the root cause, and verify behavior using the project’s own tools.
---

Reproduce the failure with the smallest relevant test or command. Record observed behavior and expected behavior; do not infer success from a command being started.

Trace the failing path and examine inputs, state transitions, error handling, and relevant dependencies. Form a hypothesis and use a focused check to distinguish it from alternatives.

Fix the cause with a minimal change. Add a regression test when it meaningfully guards the failure. Cover adjacent error cases rather than weakening assertions or swallowing errors.

Run the focused test, then the related suite. Report the actual results and any checks you could not perform. Follow the existing language, test framework, and package manager.
