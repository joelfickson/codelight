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
