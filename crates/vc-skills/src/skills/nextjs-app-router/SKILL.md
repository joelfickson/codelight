---
name: nextjs-app-router
description: Build and structure Next.js App Router apps correctly - the app/ directory, layouts, loading.tsx and Suspense streaming, route handlers, server vs client components, the Metadata API, and dynamic segments. Use when creating or editing pages, routes, layouts, or data fetching in a Next.js project.
---

# Next.js App Router

Structure lives in `app/`; each folder is a URL segment. A folder becomes a route when it has a `page.tsx` whose default export is a React component.

## File conventions
- `layout.tsx` wraps a segment and everything nested inside it; it receives `children`, preserves state across sibling navigations, and does not re-render on those navigations. The root layout is required and renders `<html>` and `<body>`.
- `loading.tsx` creates an instant loading UI by wrapping the segment in a React Suspense boundary, streaming the fallback to the browser while the server keeps rendering.
- `error.tsx` and `not-found.tsx` handle error and 404 states for a segment.
- `route.ts` defines a Route Handler: export async functions named after HTTP methods (`GET`, `POST`, ...). A single segment cannot define both a `page` and a `route`.

## Server vs client components
Components are Server Components by default: they render on the server and stay out of the client bundle. Add the `"use client"` directive only for state, effects, or browser event handlers, and keep client boundaries small and near the leaves. Server Components can import and render Client Components.

## Data and metadata
Prefer server-side `fetch` with caching and revalidation, Server Actions for mutations, and streaming via Suspense over client-side data waterfalls. Set head tags with the Metadata API - a static `metadata` object or an async `generateMetadata` exported from a layout or page - instead of manual `<head>` tags. Metadata merges down the route tree.

## Dynamic routes
A dynamic segment wraps a folder name in brackets, `[id]`, and the value arrives via the `params` prop. A catch-all segment `[...slug]` matches multiple path segments at once.
