---
name: vercel-ship-check
description: A step-by-step procedure to verify a web project is ready to deploy to Vercel - run the typechecker, linter, tests, and production build, then audit environment variables and common Next.js/Vercel pitfalls. Use when the user wants to prepare, verify, or check a project before deploying or shipping, or asks whether it is ready to deploy.
---

# Vercel ship check

Follow these steps in order when asked to prepare or verify a project for deployment. Report results honestly: do not claim a step passed unless you ran it and saw it pass.

## 1. Identify the toolchain
Read `package.json` and the lockfile to determine the package manager (pnpm, npm, yarn, or bun) and which scripts exist (`build`, `lint`, `test`, `typecheck`).

## 2. Run the checks, narrowest first
Use run_command and read the real output; fix the root cause of any failure before continuing.
- Typecheck: `tsc --noEmit`, or the project's typecheck script.
- Lint: the project's lint script (for Next.js, `next lint`).
- Tests: the project's test script, if one exists.
- Production build: `next build`, or the project's build script. This is the closest local signal to what Vercel runs.

## 3. Audit environment variables
Search the code for `process.env.` references. For each variable, tell the user which ones they must set in Vercel and in which environment (production, preview, or development). Never print secret values.

## 4. Check common pitfalls
- Client components importing server-only code, or `"use client"` files using server APIs.
- Slow or fallible routes missing `loading.tsx` or `error.tsx`.
- Hardcoded localhost URLs or secrets committed to the repo.
- Dynamic APIs (cookies, headers) used where static rendering is expected.

## 5. Report
Summarize what you ran and its result, what you fixed, which variables the user must configure, and whether the project is ready to deploy. If you could not run a step, say so explicitly.
