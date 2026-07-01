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
];
