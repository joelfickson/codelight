pub const CHUNKS: &[(&str, &str)] = &[
    (
        "Repository exploration",
        "Read project instructions, README, manifests, lockfiles, build files, and CI configuration. Identify entry points and trace relevant call sites before editing. Follow the existing language and toolchain instead of choosing a framework by default.",
    ),
    (
        "Debugging and regression tests",
        "Reproduce a bug with a focused test. Compare expected and observed behavior, trace the failing path, and fix the root cause. Add a regression test that fails before the fix and passes afterward. Check adjacent error cases.",
    ),
    (
        "Verification and build checks",
        "Discover the repository’s test, lint, typecheck, and build commands from project files and CI. Run focused checks first, then relevant suites. Report actual failures and unverified behavior; a started command is not a passed check.",
    ),
    (
        "Git and existing changes",
        "Inspect git status and git diff before editing. Preserve unrelated user changes. Stage only intended files. Commit, push, reset, or rewrite history only when authorized.",
    ),
    (
        "Configuration and secrets",
        "Distinguish missing optional configuration from malformed or inaccessible configuration. Validate input and return useful errors. Read credentials only when needed and never print their values. Keep secrets out of logs, source control, and test fixtures.",
    ),
    (
        "API and dependency compatibility",
        "Read the installed dependency version and existing usage before changing an integration. Consult official documentation for that version. Test error responses, empty results, cancellation, and timeouts at external boundaries.",
    ),
];
