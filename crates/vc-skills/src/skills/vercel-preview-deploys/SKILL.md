---
name: vercel-preview-deploys
description: Understand Vercel deployments, preview URLs, environments, and environment variables when preparing a project to ship. Use when the user asks about deploying, previews, environments, rollbacks, deployment protection, or env var configuration on Vercel.
---

# Vercel deployments and previews

Vercel builds a deployment for every git push and gives each commit or branch its own preview URL, isolated from production - so changes can be reviewed, tested, and shared before they are promoted.

## Environments
There are three environments: production (your primary domain), preview (branch and pull-request deployments), and development (running locally). Environment variables are scoped per environment, so production and preview can hold different values such as separate API keys or database URLs. Variables are made available to builds and functions at deploy time.

## Shipping and safety
- Promotion to production happens by merging to the production branch, or with `vercel --prod`.
- Instant rollback re-points production back to a previous deployment without a rebuild.
- Deployment protection (password, SSO, or trusted IPs) can gate access to preview or production deployments.

## Working in Codelight
This agent cannot deploy, create preview URLs, or read build/runtime logs itself yet. Prepare and verify the code locally first (typecheck, lint, tests, and a production build), then tell the user the exact command or step to deploy, and flag any environment variables they must configure per environment.
