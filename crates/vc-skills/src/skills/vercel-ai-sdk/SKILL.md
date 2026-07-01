---
name: vercel-ai-sdk
description: Build AI features with the Vercel AI SDK - generateText and streamText, tool calling, structured output, the provider abstraction, and the useChat client hook, optionally routed through the AI Gateway. Use when adding chat, text generation, streaming, or model calls to a project.
---

# Vercel AI SDK

The AI SDK is a TypeScript toolkit (npm package `ai`) for building AI applications across React, Next.js, Vue, Svelte, and Node.

## Core
- `generateText` produces a complete result in a single call that resolves when generation finishes; `streamText` returns a stream so you can render tokens incrementally. Both take a `model` plus a prompt or a list of messages.
- Define `tools` with a schema for their arguments and an `execute` function; when the model calls a tool the SDK validates the arguments, runs execute, and feeds the result back into the conversation.
- Structured output returns typed objects instead of free-form text.
- The provider abstraction targets many model providers through one interface, so you can switch models (for example OpenAI to Anthropic) by changing the model string, usually a couple of lines.

## Client UI
`useChat` (with `useCompletion` and `useObject`) manages chat state on the client - the message list and the current input - sending user messages to a server route and streaming the assistant reply back into the list.

## AI Gateway
Route model calls through the Vercel AI Gateway: one OpenAI-compatible endpoint to many providers, with unified billing, observability, and provider failover. Point the provider at the gateway and resolve models by a `provider/model` string.
