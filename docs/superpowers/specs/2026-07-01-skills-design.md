# skills — Skill System Design

Date: 2026-07-01
Status: approved; building

## Goal

Let Codelight incorporate **Agent Skills** (the `SKILL.md` open standard) and apply them via
progressive disclosure, so the agent applies current, expert Vercel patterns on demand. Skills
come from bundled built-ins and local directories; the skills.sh registry is a future source.

## The SKILL.md standard (what we implement)

A skill is a directory with a `SKILL.md` (YAML frontmatter + Markdown body) and optional
`scripts/`, `references/`, `assets/`. Frontmatter: `name` (kebab-case, <=64) and `description`
(<=1024, "what and when"). Progressive disclosure has three levels:
1. **Advertise** (~100 tokens/skill): `name` + `description` in the system prompt.
2. **Load** (<5k tokens): the full body, on demand.
3. **Resources**: `scripts/`/`references/`/`assets/` files, on demand.

## Architecture

New crate `skills` (-> `types`, `tools`). No cycles.

- `Skill { name, description, body, dir: Option<PathBuf> }` — body held eagerly (files are small);
  `dir` present for disk skills so resources can be read lazily.
- `SkillRegistry`
  - `load()` — parse bundled built-ins (`include_str!`) + scan local dirs:
    `./.vercelcode/skills/`, `~/.vercelcode/skills/`, `./.claude/skills/`. Each subdir containing
    a `SKILL.md` is a skill; last-loaded wins on name collision (local overrides built-in).
  - `advertise() -> String` — a `## Available skills` section (`- name: description`).
  - `get(name)`, `resource(name, rel_path)` (with a path-traversal guard: reject `..`/absolute).
- Frontmatter parse: split the leading `---...---` block, `serde_yaml::from_str` into
  `{ name, description }`; the remainder is the body.

## Tools (progressive disclosure L2/L3)

In `skills`, implementing `tools::Tool`, holding `Arc<SkillRegistry>`:
- `load_skill(name)` -> the full `SKILL.md` body (from memory).
- `read_skill_resource(name, path)` -> a resource file's contents (disk skills only; guarded).

## Bundled skills

~3 concise `SKILL.md` skills authored from the already-verified docs corpus:
`nextjs-app-router`, `vercel-preview-deploys`, `vercel-ai-sdk`.

## Wiring

- `agent`: `Agent::set_skills(&advertisement)` replaces the system message with
  `SYSTEM_PROMPT + "\n\n" + advertisement`. The base prompt gains a short "Skills" note: when a
  task matches a skill's description, call `load_skill` before doing non-trivial work.
- `cli`: build the `SkillRegistry`, wrap in `Arc`, register the two skill tools, and call
  `agent.set_skills(registry.advertise())`.
- `agent_spike`: same wiring, for a live end-to-end test.

## Deferred (needs auth)

skills.sh live `search_skills` / `add_skill`: the `skills.sh/api/v1` endpoints return 401
(Vercel OIDC). Revisit once auth is available; until then users run `npx skills add ...` into a
local skills dir and Codelight loads it.

## Testing

- Unit: frontmatter parse (name/description/body); registry load from a temp dir; `advertise`
  formatting; resource path-traversal guard rejects `..`.
- Live: an `agent_spike` run where a Vercel question makes the agent call `load_skill`.
