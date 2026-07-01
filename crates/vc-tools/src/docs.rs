pub const CHUNKS: &[(&str, &str)] = &[
    (
        "Next.js App Router: the app directory",
        "The App Router lives in a directory named app, where each nested folder maps to a segment of the URL. A route becomes publicly accessible once a folder contains a page file whose default export is a React component. Special files such as layout, loading, and error add shared behavior to a segment and its children.",
    ),
    (
        "Next.js layouts and nested layouts",
        "A layout file defines UI shared across a segment and everything nested inside it, wrapping child segments through a children prop. Layouts nest, so a parent layout wraps the layouts and pages of deeper folders. The root layout is required and must render the html and body tags. Layouts preserve their state and do not re-render when navigating between sibling routes.",
    ),
    (
        "Next.js loading.tsx and streaming",
        "A loading file creates an instant loading state that is shown while a route segment and its data load. Next.js implements this by wrapping the segment in a React Suspense boundary, so the fallback is streamed to the browser while the server keeps rendering. Users see a loading skeleton immediately instead of a blank page. You can also place Suspense around individual components for more granular streaming.",
    ),
    (
        "Next.js route handlers",
        "Route handlers let you respond to requests for a path using a route file inside the app directory. You export async functions named after HTTP methods such as GET and POST, each receiving a request and returning a response. A single segment cannot define both a page and a route handler at the same time.",
    ),
    (
        "Next.js Server and Client Components",
        "In the App Router, components are Server Components by default, so they render on the server and their code stays out of the client bundle. Adding the \"use client\" directive at the top of a file marks that module and the modules it imports as Client Components, which can use state, effects, and browser event handlers. Server Components can import and render Client Components, letting you keep interactive code at the leaves of the tree.",
    ),
    (
        "Next.js Metadata API",
        "The Metadata API sets document head tags such as title and description from inside the app directory. You can export a static metadata object or an async generateMetadata function from a layout or a page. Next.js merges metadata down the route tree, so nested segments can extend or override values defined by their parents.",
    ),
    (
        "Next.js dynamic route segments",
        "A dynamic segment is created by wrapping a folder name in square brackets, such as [id], so it matches a variable value in the path. The matched value is passed to the page and layout through the params prop. A catch-all segment written as [...slug] matches multiple path segments at once.",
    ),
    (
        "Vercel preview deployments",
        "Vercel builds a preview deployment for every git push, giving each commit or branch its own unique URL. This lets you review, test, and share changes before they are promoted to production. Preview deployments are isolated from the production environment.",
    ),
    (
        "Vercel environment variables",
        "Environment variables on Vercel can be scoped to the production, preview, and development environments independently. This lets you supply different values, such as separate API keys or database URLs, depending on where the code runs. The variables are made available to your builds and functions at deploy time.",
    ),
    (
        "Vercel serverless and edge functions",
        "Vercel can run backend code as serverless functions that scale automatically and execute in a chosen region. Edge functions instead run on the edge network close to the user for lower latency, within a more constrained runtime than serverless functions. You pick the model for each route based on its latency and capability needs.",
    ),
    (
        "Vercel Fluid Compute",
        "Fluid Compute is Vercel's execution model that lets a single function instance serve multiple concurrent invocations instead of starting a separate instance per request. By reusing warm instances it reduces cold starts and can lower cost for workloads that spend time waiting on I/O. It is aimed at modern workloads such as AI inference and streaming responses.",
    ),
    (
        "Vercel environments: production, preview, development",
        "Vercel separates deployments into production, preview, and development environments. Production serves your primary domain, preview covers branch and pull request deployments, and development refers to running the project locally. Settings such as environment variables can be configured differently for each environment.",
    ),
    (
        "Vercel AI SDK: generateText and streamText",
        "The AI SDK exposes generateText to produce a complete text result in a single call that resolves once generation finishes. streamText instead returns a stream so you can render tokens incrementally as they arrive. Both accept a model plus a prompt or a list of messages and share a common set of options.",
    ),
    (
        "Vercel AI SDK: defining tools",
        "The AI SDK lets you define tools that a model can call, each described by its parameters and an execute function. The parameters are declared with a schema so the model knows the shape of the arguments to supply. When the model calls a tool, the SDK validates the arguments, runs execute, and feeds the result back into the conversation.",
    ),
    (
        "Vercel AI SDK: model providers",
        "The AI SDK uses a provider abstraction so the same code can target different model providers through one shared interface. You create a model instance from a provider and pass it to functions like generateText or streamText. This lets you switch or compare providers without rewriting your application logic.",
    ),
    (
        "Vercel AI SDK: useChat on the client",
        "useChat is a client hook that manages the state of a chat interface, including the message list and the current input. It sends user messages to a server endpoint and streams the assistant's reply back into the message list. It provides helpers for handling input changes and submitting the form, cutting the boilerplate needed to build a chat UI.",
    ),
    (
        "Vercel eve (Agent Framework)",
        "eve is Vercel's open-source, filesystem-first framework for building durable backend AI agents that run on Vercel, positioned as \"Next.js for agents.\" You define each agent as a directory of files under an `agent/` directory, and eve discovers those files, validates them, compiles a manifest, and serves the runtime as a deployable app that runs on Vercel Functions. It is published as the npm package `eve`, is model-agnostic (works with any model, and connects to tools via MCP servers or any API with a compatible OpenAPI document), and is currently in beta. You scaffold a new project with `npx eve@latest init my-agent`.",
    ),
    (
        "eve Project Structure (agent/ directory)",
        "An eve agent is a set of named files and directories under `agent/` that are auto-discovered by name, so adding a capability is usually just adding a file. The core pieces are `agent/instructions.md` (the always-on system prompt, written in Markdown), `agent/agent.ts` (runtime config such as the model, defined via `defineAgent`), `agent/tools/*.ts` (one typed tool per file, where the filename becomes the runtime tool name), `agent/skills/*` (optional on-demand procedures), `agent/subagents/*` (optional child agents), `agent/channels/*` (platform entry points such as HTTP and Slack), `agent/connections/*` (typed external-service integrations), and `agent/sandbox/*` (the isolated compute environment). Tools and skills are auto-discovered with no manual registration.",
    ),
    (
        "eve Durable Sessions and Turns",
        "In eve, a session is the durable conversation or task started by a channel or HTTP request, and each user message or external event creates a turn during which the agent can call tools, load skills, read/write sandbox files, and delegate to subagents. Sessions run on top of Vercel Workflows, which persist progress as an event log and deterministically replay it to reconstruct state, so a session can survive cold starts, redeploys, and long pauses while waiting for the next message or a tool result. Because turns are long-running and stream incrementally, eve benefits from Fluid Compute, which is enabled by default for new projects.",
    ),
    (
        "eve Sandbox (Sandboxed Code Execution)",
        "Every eve agent has one sandbox: an isolated, bash-style compute environment with its own filesystem. Framework tools such as `bash`, `read_file`, and `write_file` target the sandbox, and authored tools can target it too. On Vercel, the sandbox can run on Vercel Sandbox, using ephemeral microVMs for untrusted or model-generated commands.",
    ),
    (
        "eve Tools, Skills, and Subagents",
        "Tools are typed actions the model can call during a turn, defined one per file in `agent/tools/`, with the filename becoming the tool name the model sees. Skills are larger procedures or reference material the model loads on demand, kept separate from the always-on prompt and installed into `agent/skills/`. A subagent is a child agent the model delegates a focused subtask to; unlike a skill, it runs as a separate agent with fresh conversation history and state. eve offers a built-in `agent` tool that delegates to a copy of the current agent, plus declared subagents that live under `agent/subagents/*` with their own config.",
    ),
    (
        "eve Approvals and Evals",
        "eve provides built-in human-in-the-loop approvals and evaluations. Any action can be configured to require approval, and the agent pauses there and waits, indefinitely if needed, without consuming compute, resuming from where it left off once approved, taking advantage of eve's durable-workflow foundation. Evals are scored test suites that let developers validate agent behavior; you can run `eve eval` locally (or wire it into CI) or point it at a deployed app, so a prompt or model change surfaces what it broke before shipping.",
    ),
    (
        "eve Integration with Vercel Services and the AI SDK",
        "eve builds on existing Vercel primitives rather than requiring separate infrastructure: Vercel Workflows persist session state and resume interrupted work, Vercel Sandbox isolates code execution, AI Gateway routes model requests and handles provider fallbacks (resolving model strings such as `openai/gpt-5.4-mini` and using Vercel OIDC so you don't manage provider API keys directly), and Vercel Connect manages OAuth tokens and API keys for external services. Every eve project also gets Agent Runs in the Vercel dashboard, which shows sessions, turns, tool calls, reasoning, timing, and token usage with no setup. eve emits AI SDK spans, and you can optionally export them to an external OpenTelemetry backend by adding an `agent/instrumentation.ts` file.",
    ),
    (
        "Vercel v0",
        "v0 is Vercel's AI product that turns natural-language prompts into working web applications, marketed as building full-stack web apps with AI. It generates both the user interface and the corresponding code from a description, and users can iterate on results within v0 before copying the code or deploying. Introduced under Vercel's 'Generative UI' approach, which combines frontend development best practices with generative AI, it moved to public beta in October 2023. The product now lives at v0.app, and the older v0.dev domain redirects there.",
    ),
    (
        "v0 Output: React, Tailwind CSS, and shadcn/ui",
        "v0 produces code using open-source technologies, and Vercel's announcement specifically cites React, Tailwind CSS, and shadcn/ui. It is described as trained on best practices for these tools and can generate React UIs built from shadcn/ui components. In practice v0 also works with TypeScript and Next.js, and can produce more advanced React and Next.js features by breaking frontend tasks into steps.",
    ),
    (
        "v0 Workflow and Vercel Integration",
        "A typical v0 workflow is to describe an interface, have v0 generate code, iterate within v0, and then copy the code into your app or deploy it. v0 integrates with the Vercel ecosystem via one-click deployment to production, and it can connect to GitHub to push code directly to a repository. The v0.app homepage describes the product as 'agentic by default,' stating that v0 'plans, creates tasks, and connects to databases as it builds.'",
    ),
    (
        "v0 Platform API",
        "The v0 Platform API is described by Vercel as a text-to-app REST interface that gives developers access to the same infrastructure that powers v0. It generates full-stack web apps from natural-language prompts and returns parsed code files plus a live demo URL, and every app built with it is backed by a v0 chat link for further iteration. It is available as a TypeScript SDK and is used to build products such as website/app builders, embedded UI generation in other tools, and Slack/Discord bot integrations.",
    ),
    (
        "Vercel AI SDK",
        "The Vercel AI SDK is a TypeScript toolkit for building AI-powered applications that works with React, Next.js, Vue, Svelte, and Node.js. It provides a unified provider API so you can switch between models (for example changing the model string from an OpenAI model to an Anthropic model) by changing about two lines of code, plus structured outputs, tool calling, and streaming-first support for text, objects, and UI. It is distributed as an npm package named 'ai', installable with 'npm i ai' (or the equivalent pnpm/yarn/bun command).",
    ),
    (
        "AI SDK Core: generateText and streamText",
        "AI SDK Core provides a unified API to call any LLM. Its two central text functions are generateText, which is intended for non-interactive use cases such as automation tasks and agents that use tools, and streamText, which is intended for interactive use cases such as chat bots and content streaming. Both are typically called with a 'model' and a 'prompt', and both can also produce structured, schema-validated output.",
    ),
    (
        "AI SDK Core: Structured Output and Tool Calling",
        "AI SDK Core provides generateObject and streamObject to generate type-safe JSON constrained to a schema, for example a Zod schema that the model output must conform to; typical uses include extracting information from text, classifying data, and generating synthetic data. It also supports tool calling: you define tools with the 'tool' helper, each with a description, an inputSchema (for example a Zod schema), and an optional 'execute' function, and pass them via the 'tools' option to generateText or streamText so the model can interact with external systems.",
    ),
    (
        "AI SDK UI Hooks (useChat, useCompletion, useObject)",
        "AI SDK UI is a set of framework-agnostic hooks for building chat, completion, and assistant interfaces, abstracting streaming and state management for messages, inputs, loading, and errors. Its main hooks are useChat (real-time streaming of chat messages), useCompletion (streamed text completions that update the UI as they arrive), and useObject (consuming streamed JSON objects for structured data). It is available through framework-specific packages including @ai-sdk/react, @ai-sdk/svelte, @ai-sdk/vue, and @ai-sdk/angular.",
    ),
    (
        "Vercel AI Gateway",
        "Vercel AI Gateway is a unified API that provides access to hundreds of AI models from multiple providers through a single endpoint and a single API key, so you can switch providers and models with minimal code changes. It automatically retries requests to other providers if one fails, supports embeddings, and lets you monitor spend across providers, with no markup on tokens (tokens cost the same as from the provider directly, including when using Bring Your Own Key). It works with AI SDK v5 and v6, the OpenAI Chat Completions API, the OpenAI Responses API, and the Anthropic Messages API.",
    ),
    (
        "AI Gateway OpenAI-Compatible Endpoint",
        "AI Gateway exposes an OpenAI Chat Completions-compatible API at the base URL https://ai-gateway.vercel.sh/v1, implementing the same specification as OpenAI's Chat Completions API so existing OpenAI client libraries work with a base-URL change. Requests authenticate with an 'Authorization: Bearer <token>' header using either an AI Gateway API key (commonly supplied via the AI_GATEWAY_API_KEY environment variable) or a Vercel OIDC token; if both are present, the API key takes precedence. Models are referenced with a 'provider/model' string (for example 'anthropic/claude-opus-4.8'), and supported endpoints include GET /models, GET /models/{model}, POST /chat/completions (with streaming, tool calls, and structured outputs), and POST /embeddings.",
    ),
    (
        "AI Gateway Observability and Spend",
        "AI Gateway logs spend, model usage, and observability metrics that you view in the AI Gateway Overview section of the Vercel dashboard, at both team scope (aggregated across all projects) and project scope. The Usage section shows four charts: Requests by Model, Time to First Token (TTFT), Input/Output Token Counts, and Spend. The Requests section provides summaries grouped by project and by API key (including request count, average tokens, P75 duration, P75 TTFT, and cost) plus a sortable and exportable log of all requests; extended timeframes and further retention require Observability Plus.",
    ),
    (
        "Vercel Functions",
        "Vercel Functions run server-side code without managing a server, scaling automatically with traffic and scaling down to zero when there are no incoming requests. Vercel is framework-aware and automatically detects and optimizes for your framework, routing traffic through its CDN. Functions run in a single region by default (the Node.js runtime defaults to Washington, D.C., iad1) and can be pinned near your data source, with multi-runtime support including Node.js, Python, Go, and more. Pricing is based on active CPU, provisioned memory, and invocations when fluid compute is enabled.",
    ),
    (
        "Vercel Edge Runtime vs Node.js Runtime",
        "The Edge runtime is built on the V8 engine and runs in isolated execution environments that do not require a container or virtual machine, exposing a subset of Web APIs (fetch, Request, Response, Web Streams, Web Crypto) plus a few Node.js modules, but it disallows dynamic code execution such as eval and new Function and cannot access the filesystem. Edge functions execute in the region closest to the request by default and must begin sending a response within 25 seconds, and can continue streaming for up to 300 seconds. Both the Edge and Node.js runtimes run on Fluid compute with Active CPU pricing, and Vercel's docs currently recommend migrating from Edge to Node.js for improved performance and reliability. The Node.js runtime is the default, supports streaming, and gives access to the broader Node.js API surface.",
    ),
    (
        "Running Any Dockerfile on Vercel",
        "Vercel can run any HTTP server directly from a Dockerfile: you add a Dockerfile.vercel file to your project, and Vercel builds the image, stores it in your project's registry, deploys it, and autoscales it on Fluid compute. Because it runs on Fluid compute with Active CPU pricing, you are billed for execution time rather than wall time, so instances idle-waiting on queries or API calls do not incur CPU charges, and instances scale down when traffic stops. Your server must listen on the $PORT environment variable (defaulting to 80) and speak HTTP, and each container is treated as a stateless process that keeps nothing between requests. Vercel lists frameworks and languages such as Rails, Spring Boot, Express, Laravel, ASP.NET, FastAPI, a web server behind nginx, Go, Java, and PHP as compatible.",
    ),
    (
        "Vercel Sandbox",
        "Vercel Sandbox is a compute primitive for safely running untrusted or user-generated code, designed for AI agents, code generation, and developer experimentation. Each sandbox runs in an isolated, ephemeral Linux microVM (Firecracker) with its own filesystem and network, so untrusted code runs without affecting production. Sandboxes run on Amazon Linux 2023 with Node.js and Python runtimes available (node26, node24, node22, python3.13; node24 is the default), each running as the vercel-sandbox user with sudo access. You manage sandboxes through the @vercel/sandbox JS SDK, the Python SDK, or a CLI, and authenticate using Vercel OIDC tokens (recommended) or access tokens.",
    ),
    (
        "Vercel Storage Overview",
        "Vercel offers a suite of managed, serverless storage products that integrate with frontend frameworks. The first-party products are Vercel Blob for large file/object storage and Vercel Edge Config for global, low-latency configuration data. Relational (Postgres), key-value/Redis, NoSQL, and vector databases are not first-party; they are provided through the Vercel Marketplace from third-party providers such as Neon, Upstash, Supabase, and AWS. Blob and Edge Config stores can be brought along when upgrading from Hobby to Pro (or downgrading from Pro to Hobby) via the dashboard Storage section.",
    ),
    (
        "Vercel Edge Config",
        "Edge Config is a global data store that lets you read data in the region closest to the user without querying an external database or upstream server; its data is actively replicated to all regions in the Vercel CDN. The vast majority of reads complete within 15ms at P99, or often less than 1ms. It is designed for data that is read frequently but updated infrequently, such as feature flags, A/B testing, critical redirects, and IP blocking, and it can be read from Middleware and Vercel Functions. Reads require a read access token and writes require an API token; read optimizations are available on the Edge and Node.js runtimes.",
    ),
    (
        "Vercel Blob",
        "Vercel Blob is a scalable object storage service for uploading files at build time or runtime, supporting both private stores (authenticated read access) and public stores (accessible to anyone with the URL); the access mode is fixed at store creation and cannot be changed. It uses Amazon S3 as its underlying storage infrastructure, providing 99.999999999% (11 nines) durability and 99.99% availability. Content is served through a network of 20 regional hubs, and Vercel's CDN caches all blobs for up to one month by default (configurable via the cacheControlMaxAge option). Blob stores can be created in any of the 20 regions, and Vercel recommends multipart uploads for files larger than 100 MB.",
    ),
    (
        "Vercel Marketplace Storage",
        "The Vercel Marketplace connects projects with third-party storage providers to provision databases directly from the Vercel dashboard or CLI, automatically injecting credentials as environment variables and offering unified billing through Vercel. For Postgres you can use providers like Neon, Supabase, or AWS Aurora Postgres, and for key-value stores you can use Upstash Redis; NoSQL and vector databases are also available. Resources can be provisioned with the CLI command 'vercel install <integration>' (for example 'vercel install neon'). For supported Postgres integrations (AWS Aurora Postgres, Neon, Prisma Postgres, and Supabase), you can run SQL queries, edit data, and inspect the schema directly from the dashboard.",
    ),
    (
        "Redis on Vercel (Vercel KV deprecation)",
        "Vercel provides Redis by connecting external Redis databases through the Marketplace rather than as a first-party product, letting you provision and configure a Redis database and have credentials injected into your project as environment variables. Vercel KV is no longer available: existing Vercel KV stores were automatically moved to Upstash Redis in December 2024. For new projects, you install a Redis integration from the Marketplace.",
    ),
    (
        "Vercel CDN (Edge Network / Global CDN)",
        "Vercel's CDN is a globally distributed, framework-aware network that caches content near visitors, routes requests, and runs compute close to your data, and it is included automatically with every deployment. It operates 126+ Points of Presence (PoPs) across 51 countries and 20+ compute-capable Vercel regions, with traffic flowing between PoPs and regions over a private, low-latency network. Because it reads your routing, caching, and rendering configuration at build time, CDN configuration and caching policies are an output of the build for supported frameworks, eliminating the need to manually set Cache-Control headers. Every deployment uses HTTPS with automatically provisioned SSL certificates and TLS 1.2/1.3 support, plus unmetered, always-on DDoS mitigation at no extra cost.",
    ),
    (
        "Incremental Static Regeneration (ISR)",
        "ISR is a caching strategy that serves a fast cached page to visitors while regenerating the page in the background, following the stale-while-revalidate pattern, and Vercel provides fully managed caching and routing for it with frameworks like Next.js, SvelteKit, Nuxt, and Astro. Content can be updated through time-based revalidation (after a set interval) or on-demand revalidation (triggered via an API call), and both run in the background so visitors keep getting the cached version until the new one is ready. The durable ISR cache lives alongside your Function region and persists content for 31 days or until you revalidate, and it is scoped per deployment so each deployment generates its own cache. When you revalidate, all caches across all regions update within 300ms, and Vercel automatically collapses multiple concurrent requests to the same uncached path into a single function invocation per region.",
    ),
    (
        "Vercel Image Optimization",
        "Vercel dynamically transforms unoptimized images to reduce file size while maintaining quality, including resizing for different device sizes and converting to modern formats like WebP and AVIF, and caches the results on the Vercel CDN. It works with framework Image components such as Next.js next/image (using the /_next/image endpoint) and Nuxt and Astro (using /_vercel/image), with query parameters including url (source image), w (width in pixels), and q (quality from 1 to 100). On a request, Vercel reports a cache status of HIT (served from cache), MISS (fetched, transformed, cached, then served), or STALE (served from cache while revalidating in the background). Static/local optimized images are cached for up to 31 days on the Vercel CDN, and once an image is cached it remains served even if the source changes until the cache expires or is invalidated via purging.",
    ),
    (
        "Vercel Routing Middleware",
        "Routing Middleware executes code before a request is processed on a site, running globally before the cache, which makes it effective for adding personalization to statically generated content by rewriting, redirecting, adding headers, or running custom logic before returning a response. It is built on top of Vercel's fluid compute, works with any framework, and is added by creating a middleware.ts (or .js) file at the project root. The default runtime is Edge, but it is also available on the Node.js and Bun runtimes, which you select by exporting a config object with a runtime property. Requests processed by Routing Middleware are subject to limits including a maximum URL length of 14 KB, maximum request body length of 4 MB, a maximum of 64 request headers, and maximum request headers length of 16 KB.",
    ),
    (
        "Vercel CDN Caching and Cache-Control",
        "Vercel's CDN caches complete HTTP responses (pages, API responses, and static assets) per region, and static files are cached automatically for the lifetime of a deployment after the first request. To cache Vercel Function responses you set Cache-Control headers using directives such as s-maxage=N, optionally combined with stale-while-revalidate=Z; Vercel also supports the targeted CDN-Cache-Control header (controls Vercel and other CDN caches separately from the browser) and Vercel-CDN-Cache-Control (controls only Vercel's cache and is not returned to the browser or forwarded to other CDNs). For a response to be cacheable it must use GET or HEAD, lack Authorization and Range headers, use a 200/404/410/301/302/307/308 status code, be under 10MB, and not contain set-cookie or private/no-cache/no-store directives. The x-vercel-cache response header reports the cache state, the maximum cache time is 1 year (best-effort, not guaranteed), and the max cacheable response size is 10MB for non-streaming and 20MB for streaming functions.",
    ),
    (
        "Vercel Deployments and Git Integration",
        "A deployment on Vercel is the result of a successful build of a project, and each deployment gets a unique URL. The most common way to create one is by pushing code to a connected Git repository: once a repo is imported, each commit or pull request automatically triggers a new deployment. Vercel supports GitHub, GitLab, Bitbucket, and Azure DevOps as Git providers. Deployments can also be created via the Vercel CLI, Vercel Drop (drag-and-drop), Deploy Hooks (a unique URL that triggers a build), and the Vercel REST API.",
    ),
    (
        "Vercel Preview Deployments",
        "Preview deployments let you deploy and test changes in a live environment without affecting production. By default Vercel creates a preview deployment when you push a commit to a branch that is not the production branch, open a pull request on GitHub, GitLab, or Bitbucket, or deploy with the CLI without the --prod flag. Each preview deployment gets an automatically generated URL, and links typically appear in the Git provider's PR comments or the Vercel Dashboard. There are two preview URL types: a branch-specific URL that always points to the latest changes on that branch, and a commit-specific URL that points to the exact deployment of a single commit.",
    ),
    (
        "Vercel Deployment Environments (Local, Preview, Production)",
        "Vercel provides three default environments: Local (developing and testing on your machine), Preview (pre-production testing, QA, and collaboration), and Production (the live, user-facing site on the production domain). By default, pushing or merging into the production branch (commonly main) triggers a production deployment, and you can also deploy explicitly with vercel --prod. When a production deployment succeeds, Vercel updates the production domains to point to it. Pro and Enterprise teams can additionally create Custom Environments (such as staging or QA) with their own configuration.",
    ),
    (
        "Vercel Generated Deployment URLs",
        "When you create a preview or production deployment, Vercel automatically generates a unique URL to access that specific deployment, publicly accessible by default unless restricted with Deployment Protection. A commit URL has the structure <project-name>-<unique-hash>-<scope-slug>.vercel.app, where the unique hash is 9 randomly generated numbers and letters, and always shows that exact commit. A branch URL has the structure <project-name>-git-<branch-name>-<scope-slug>.vercel.app and always shows the most recent changes on that branch. If more than 63 characters precede the .vercel.app suffix, they are truncated.",
    ),
    (
        "Vercel Environment Variables Scoped Per Environment",
        "Environment variables on Vercel are key-value pairs configured outside source code, encrypted at rest and visible to users with access to the project. For each variable you select one or more environments to apply it to: Production, Preview, Development, or custom environments. Preview variables apply to any branch that is not the production branch, and can be scoped to all non-production branches or a specific branch, where branch-specific values override others of the same name. Variables can be declared at the team level (available to all projects) or project level, changes only apply to new deployments, and the total size limit is 64 KB per deployment across all variables combined.",
    ),
    (
        "Vercel Instant Rollback",
        "Instant Rollback lets you quickly revert to a previous production deployment, which is useful for swift recovery from production incidents like breaking changes or bugs. Only deployments previously aliased to a production domain are eligible; most preview deployments are not. Pro and Enterprise owners and members can roll back to any eligible deployment, while Hobby users can roll back to the immediately previous deployment. The rollback happens instantaneously by pointing domains back to the selected deployment, and afterward Vercel turns off auto-assignment of production domains until you undo the rollback by promoting a deployment (for example with vercel promote).",
    ),
    (
        "Vercel Deployment Protection",
        "Deployment Protection controls who can access a project's preview and production URLs, configured at the project level by choosing a protection method and a protection scope. Methods include Vercel Authentication (restricts access to Vercel users, available on all plans), Password Protection (Enterprise, or a paid add-on for Pro), Trusted IPs (restricts by IPv4 address, Enterprise only), and Passport (beta, Enterprise). Scopes include Standard Protection (all deployments except production domains, all plans) and All Deployments (all URLs including production, Pro and Enterprise). On the Hobby plan, Vercel Authentication with Standard Protection is available but the production domain remains publicly accessible.",
    ),
    (
        "Vercel CLI",
        "The Vercel CLI lets you manage and configure Vercel projects from a terminal or automated system, and can be installed with npm, pnpm, yarn, or bun (for example npm i -g vercel). Running vercel (or vercel deploy) creates a deployment, vercel --prod deploys to production, and vercel dev replicates the Vercel deployment environment locally. vercel link links a local directory to a project, vercel env manages environment variables (vercel env pull writes them to a local .env file), and vercel pull updates local project settings and variables. It also provides commands such as promote, rollback, redeploy, inspect, and list; in CI/CD you authenticate with a token via the VERCEL_TOKEN environment variable or the --token flag.",
    ),
    (
        "Vercel Observability",
        "Observability lets you monitor and analyze the performance and traffic of your Vercel projects through tracked events and framework-aware insights aligned with your app's architecture. It is available on all plans at no additional cost (with some limitations) and can be viewed at the team or project level. Insight sections include Vercel Functions, External APIs, Edge Requests, Middleware, Fast Data Transfer, Image Optimization, ISR, Build Diagnostics, AI Gateway, Queues, External Rewrites, and Microfrontends. Vercel tracks event types including Edge Requests, Vercel Function Invocations, External API Requests, Routing Middleware Invocations, and AI Gateway Requests, counted at the team level across all projects.",
    ),
    (
        "Vercel Observability Plus",
        "Observability Plus is available on Paid Pro and Enterprise teams and unlocks more granular data exploration, higher limits, and longer retention on top of base Observability. It extends data retention to 30 days (versus Hobby 12 hours, Pro 1 day, and Enterprise 3 days for base Observability), adds the ability to author queries in the dashboard and save them to notebooks, and provides latency (p75) data and breakdowns by path. For teams created or upgraded to Paid Pro on or after April 3, 2026 it is enabled by default; it is billed by usage at $1.20 per 1 million events. Vercel also offers Monitoring, which lets you build dashboards and alerts on top of Observability metrics.",
    ),
    (
        "Vercel Logs (Build and Runtime)",
        "Vercel generates build logs during deployment that show deployment progress, build tool versions, warnings or errors, and details about files and dependencies that were installed, compiled, or built. Runtime logs let you search, inspect, and share your team's runtime logs at the project level from the deployments section of the Vercel dashboard, with retention that depends on your plan and whether Observability Plus is enabled. For longer log storage you can use Log Drains to export log data to external destinations.",
    ),
    (
        "Vercel Web Analytics",
        "Web Analytics provides privacy-friendly insights into a website's visitors, including top visited pages, referrers, and demographics such as location, operating system, and browser. It stores only anonymized data and does not use cookies; visitors are identified by a hash generated from the incoming request that is valid for a single day and then reset, so visitors cannot be tracked between days or across sites. It is built into the Vercel platform and accessible from the project dashboard, supports custom events and feature flag usage, and excludes bot traffic detected via the User-Agent header. Panel data can be exported as CSV (up to 250 entries).",
    ),
    (
        "Vercel Speed Insights",
        "Speed Insights gives a detailed view of a website's performance based on Core Web Vitals using real user monitoring, where the Real Experience Score (RES) is computed from real data points collected on visitors' devices. Tracked metrics include Largest Contentful Paint (LCP), Cumulative Layout Shift (CLS), Interaction to Next Paint (INP), First Contentful Paint (FCP), First Input Delay (FID), Total Blocking Time (TBT), and Time to First Byte (TTFB). Data can be viewed for production and preview environments, and the dashboard lets you adjust the timeframe and select a percentile (P75, P90, P95, P99), as well as break down performance by route or path, HTML element, and country.",
    ),
    (
        "Vercel WAF (Web Application Firewall) and Custom Rules",
        "The Vercel WAF is part of the Vercel Firewall and provides security controls to monitor and control traffic to a site through logging, blocking, and challenging. Custom Rules can log, deny, challenge, bypass, redirect, or rate limit requests based on one or more logical conditions matched against parameters of the incoming request, and conditions can be combined with AND/OR operators. Configuration changes take effect immediately without requiring a redeployment, and rules can also be described in natural language and generated by Vercel. Hobby projects are limited to 3 total custom rules, with higher limits available on Pro and Enterprise. Rules can additionally be defined in vercel.json via the routes property, though vercel.json only supports the challenge and deny actions.",
    ),
    (
        "Vercel WAF Rate Limiting",
        "WAF Rate Limiting is a custom rule action that controls how many requests from the same source can hit an application within a time window, helping protect resources such as API endpoints and control usage costs. You choose a limiting strategy of Fixed Window (all plans) or Token Bucket (Enterprise), with a Time Window that defaults to 60s and a Request Limit that defaults to 100 requests, and you select the source key(s) to count against (IP and JA4 Digest on all plans, plus User Agent and arbitrary Header keys on Enterprise). When the limit is exceeded the follow-up action can be the default 429 response, Log, Deny, or Challenge. Rate limit counters are tracked per-region, so traffic matching a key across multiple regions can exceed the configured single-region limit.",
    ),
    (
        "Vercel DDoS Mitigation",
        "Vercel provides automatic DDoS mitigation for all deployments regardless of plan, blocking incoming traffic when it identifies abnormal or suspicious levels of requests. It mitigates L3, L4, and L7 DDoS attacks by continuously monitoring traffic, filtering out malicious requests while allowing legitimate ones, and dynamically scaling resources to absorb increased traffic. Vercel does not charge customers for traffic blocked by DDoS mitigation, though usage is incurred for requests served before mitigation kicks in or for traffic not recognized as a DDoS event. Enterprise teams receive dedicated DDoS support, Pro and Enterprise customers can use System Bypass Rules to keep essential traffic from being blocked, and system mitigations can be temporarily paused for a project for 24 hours.",
    ),
    (
        "Vercel Attack Mode",
        "Attack Mode is a security feature that protects a site during DDoS attacks by requiring visitors to complete a security challenge before accessing the site, while known bots such as search engines and webhook providers are automatically allowed through. It is available for free on all plans, and requests blocked by Attack Mode do not count toward usage limits. Internal requests from your own Functions and Cron Jobs are allowed through without being challenged, and each Vercel account has its own secure boundary so other accounts cannot bypass it. It is enabled from the project's Firewall > Bot Management settings and is recommended primarily for highly targeted attacks rather than as a permanent setting; search crawlers like Googlebot are not challenged, so it does not harm SEO or search indexing.",
    ),
    (
        "Vercel Environment Variable Encryption",
        "Environment variables on Vercel are key-value pairs configured outside source code, and their values are encrypted at rest while remaining visible to any user with access to the project. Any change to environment variables applies only to new deployments, not to previous ones. Variables can be declared at the team level (available to all projects) or the project level, and the total size is limited to 64 KB per deployment across all variables combined.",
    ),
    (
        "Vercel Sensitive Environment Variables",
        "Sensitive environment variables are environment variables whose values become non-readable once created; when marked sensitive, Vercel stores the value in an unreadable format so it is safe for data like API keys and tokens. They can only be created in the production and preview environments (not development), and apply to both project-level and shared environment variables. During builds, if a sensitive value 32 characters or longer appears in build logs Vercel replaces it with [REDACTED], and the VERCEL_AUTOMATION_BYPASS_SECRET and VERCEL_OIDC_TOKEN system variables are always redacted regardless of length. Users with the owner role can enforce a policy so that all newly created production and/or preview environment variables are automatically sensitive.",
    ),
    (
        "Vercel Workflows",
        "Vercel Workflows is a fully managed platform for building durable applications and AI agents in JavaScript, TypeScript, and Python. It builds on the open-source Workflow SDK (for JavaScript and TypeScript) and on workflow support in the vercel Python SDK. You mark a function as durable with the 'use workflow' directive and individual steps with 'use step', writing async code with familiar language primitives rather than YAML or state machines. Workflows are resumable (can pause for minutes to months and resume from the exact point), durable (survive deployments and crashes via deterministic replays), and observable through built-in logs, metrics, and tracing in the Vercel dashboard. Under the hood, Vercel Functions execute the workflow and step code, Vercel Queues enqueue and execute those routes with reliability, and managed persistence stores all state and event logs. Pricing is usage-based on Events, Data Written, and Data Retained.",
    ),
    (
        "Vercel Cron Jobs",
        "Vercel Cron Jobs are time-based schedules that automate repetitive tasks by invoking Vercel Functions, configured through vercel.json or the Build Output API. To trigger a job, Vercel makes an HTTP GET request to the project's production deployment URL using the configured path. Requests always use the user agent vercel-cron/1.0 and include an x-vercel-cron-schedule header containing the cron expression that triggered the invocation, which is useful for distinguishing schedules when multiple cron jobs share the same path. The cron timezone is always UTC, alternative expressions like MON, SUN, JAN, or DEC are not supported, and you cannot configure both day-of-month and day-of-week at the same time (when one has a value, the other must be *).",
    ),
    (
        "Vercel Queues",
        "Vercel Queues is a durable event streaming system built for serverless applications where you publish messages to topics and independent consumer groups process them in parallel with automatic retries, sharding, and delivery guarantees. Each topic is a durable, append-only log that retains messages until they expire, fanning out to every subscribed consumer group, and new consumer groups can join at any time to replay non-expired history. It supports both push mode (processing on Vercel with push callbacks) and poll mode (running your own workers), automatic scaling, delayed delivery (delaying message delivery up to the retention period), and idempotency keys for deduplication, with a @vercel/queue SDK and an HTTP API. Vercel Queues is the lower-level primitive that powers Vercel Workflows.",
    ),
    (
        "Vercel Deploy Hooks",
        "Deploy Hooks are unique URLs that accept HTTP POST (or GET) requests to trigger a new deployment and re-run the Build Step, uniquely linked to a project, repository, and branch so no authentication mechanism or payload is required. They are commonly used to trigger deployments from a headless CMS on content changes or from third-party cron/scheduler services. Because anyone with the URL can deploy the project, the URL should be treated like a secret and can be revoked and recreated if compromised; builds use the Build Cache by default, which can be disabled by appending ?buildCache=false. Hobby and Pro accounts are limited to 5 deploy hooks per project and Enterprise accounts to 10.",
    ),
    (
        "Vercel Webhooks",
        "Vercel Webhooks are trigger-based HTTP endpoints that receive POST requests with a JSON payload when selected events occur; account-level webhooks can be configured by Pro and Enterprise teams from team Settings, and webhooks can also be created through Integrations. Supported event categories include Deployment events (Created, Succeeded, Promoted, Rollback, Error, Cancelled), Project events, Feature Flag events, and Firewall events such as Attack Detected. Each event payload includes the fields id, type, createdAt, payload, and region. Webhooks should be secured by comparing the x-vercel-signature header of incoming requests against the secret shown once at creation (or the Integration/Client Secret for integration webhooks), and you can create up to 20 custom webhooks per team.",
    ),
    (
        "Vercel Projects and Teams",
        "A Vercel Project represents an application deployed to the platform from a Git repository, grouping that application's deployments and custom domains. Each project has one production deployment plus many preview (pre-production) deployments, and multiple projects can connect to a single repository, which is useful for monorepo setups. Projects live within a team's Vercel dashboard, where you configure settings such as custom domains, environment variables, and deployment protection, and manage observability features like Web Analytics, Speed Insights, and Logs.",
    ),
    (
        "Vercel Custom Domains and DNS Management",
        "Vercel lets you add custom domains to a project and guides you through the DNS configuration needed to point a domain at your deployment. Vercel supports standard DNS record types including CNAME, A, NS, and MX records, and you can either point records at Vercel or delegate a domain by using Vercel's nameservers. Vercel also automatically provisions SSL certificates to secure the connection between a domain and its site, and serves traffic through its global edge network.",
    ),
    (
        "Vercel Marketplace and Integrations",
        "The Vercel Marketplace lets you extend Vercel by connecting third-party services for AI, databases, storage, CMS, commerce, and more, offering two integration types: native integrations and connectable accounts. With native integrations you subscribe to a partner's product directly through the Vercel dashboard without creating a separate account on the provider's site, and billing is managed through your Vercel account. Connectable accounts instead link an existing third-party account, prompting you to log in to that provider and then exposing features and environment variables to your project.",
    ),
    (
        "Vercel Marketplace Storage Integrations",
        "Marketplace Storage Integrations let you provision databases and data stores directly from the Vercel dashboard or CLI. Providers include Neon, Supabase, and AWS Aurora Postgres for Postgres and Upstash Redis for key-value storage, among others. When you install a storage integration, Vercel automatically injects connection strings and credentials into your project as environment variables and bills the resource through your Vercel account. For supported Postgres integrations (AWS Aurora Postgres, Neon, Prisma Postgres, and Supabase), you can run SQL queries, edit data, and inspect schemas directly from the dashboard.",
    ),
    (
        "Vercel Plan Tiers: Hobby, Pro, and Enterprise",
        "Vercel offers three plan tiers. Hobby is free and intended for personal, non-commercial projects, including automatic CI/CD from a Git repo, a global CDN, DDoS mitigation, and a web application firewall. Pro costs $20 per user per month plus usage-based costs and includes a monthly usage credit, team collaboration, and features like advanced spend management. Enterprise uses custom pricing and adds capabilities such as SCIM/directory sync, managed WAF rulesets, multi-region compute, a 99.99% SLA, and advanced support.",
    ),
    (
        "Framework Support on Vercel (Overview)",
        "Vercel has first-class support for a wide range of popular frontend, backend, and full-stack frameworks, ranging from SvelteKit to Nitro, often deployable without any upfront configuration. Deploying with a supported framework gives access to platform features including Vercel Functions, Middleware, multi-runtime support, Incremental Static Regeneration, Speed Insights, Web Analytics, and Skew Protection. For framework authors, the Build Output API is a file-system-based specification for a directory structure (the .vercel/output directory) that lets any framework integrate with Vercel platform features such as Functions, Routing, and Caching.",
    ),
    (
        "Next.js on Vercel (Flagship Framework)",
        "Next.js is a full-stack React framework maintained by Vercel, and Vercel describes itself as the native Next.js platform. While Next.js can be self-hosted, deploying to Vercel is zero-configuration and adds enhancements for scalability, availability, and performance, with framework-aware infrastructure that automatically creates Functions for SSR and enables global ISR content updates. Next.js on Vercel supports Incremental Static Regeneration, Server-Side Rendering, streaming, and Partial Prerendering, which Vercel's docs state is no longer experimental as of Next.js 16 and is built into the Cache Components model (opted into via cacheComponents). It also supports zero-configuration Image Optimization via next/image, Font Optimization, and Vercel OG image generation.",
    ),
    (
        "Zero-Config Framework Detection and Supported Frameworks",
        "Vercel auto-detects most frameworks and applies default build settings, so frameworks can be deployed with minimal or zero configuration. Beyond Next.js, the frameworks featured in Vercel's infrastructure support matrix include SvelteKit, Nuxt, Astro, Remix, Vite, TanStack Start, and Create React App, alongside a large list of additional presets such as Angular, Gatsby, Vue, SolidStart, Hono, Nitro, Express, and Python frameworks like Flask, Django, and FastAPI. Vercel states it is committed to supporting all Vercel features across frameworks and that the support matrix is continually updated over time as it works with framework authors.",
    ),
    (
        "Vercel Platform Features Across Frameworks",
        "Vercel publishes a framework infrastructure support matrix showing which platform features each framework supports. Static Assets, Edge Routing Rules, and Routing Middleware are supported across all listed frameworks (Next.js, SvelteKit, Nuxt, TanStack, Astro, Remix, Vite, and CRA). Server-Side Rendering is supported by the server-rendering frameworks (Next.js, SvelteKit, Nuxt, TanStack, Astro, and Remix), and Streaming SSR is supported by those same frameworks except Nuxt. Incremental Static Regeneration and Image Optimization are each supported by Next.js, SvelteKit, Nuxt, and Astro, while some features such as Runtime Cache are currently Next.js-only.",
    ),
];
