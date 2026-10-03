# Design review status

The user rejected the first two general-purpose HTML mocks on 2026-10-03, describing the direction as editorial. Do not implement those designs in the Rust application.

The current direction is terminal-native: compact monospace typography, inline tool output and diffs, and a simple command prompt. Avoid hero headings, taglines, editorial spacing, and a persistent metadata sidebar.

Keep design revisions in docs/mocks/codelight-general-purpose.html. Visually review the working-session and approval states before presenting them. The user authorized building the mock; implementation in the Rust application still requires explicit design approval.

# Product direction

Codelight is a general-purpose coding agent. Support configurable OpenAI-compatible endpoints. The existing Gateway may remain as a backward-compatible default, but its credential must never be forwarded to a custom endpoint. See SPEC.md for the implementation scope.

# Crate naming

Use unprefixed crate names and directories: agent, cli, gateway, tools, skills, mcp, context, and types. Do not add vc or codelight prefixes to internal crates. The executable remains codelight.
