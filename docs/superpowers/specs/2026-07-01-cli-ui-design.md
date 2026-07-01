# Codelight CLI — UI Design

Date: 2026-07-01
Status: approved; v0.1 shell implemented

## Decision

**Layout B2 — conversation + deploy-monitor rail.** Two columns: the conversation on the
left, a context rail on the right whose job is to be a **deploy monitor**. It surfaces the
build → preview lifecycle so the product's core bet ("a session ends with a live preview URL")
is visible, not buried in the chat log. Borderless except one hairline divider between the two
columns. No triangle mark — the brand is the `codelight` wordmark only.

Rejected: A (single column — no place for the preview payoff), C (three-pane IDE — needs
diff/log/deploy data we do not have until v0.2+, too heavy for v0.1).

## Visual system

Truecolor palette (terminal must support 24-bit):

| Token | Hex | Use |
|-------|-----|-----|
| accent | `#6c7cff` | caret, links, active, wordmark "light" |
| ok | `#5bd6a4` | success marks, live |
| warn | `#e0b341` | running / building |
| err | `#f0616d` | failures |
| fg | `#e9e9ee` | body text |
| muted | `#767a86` | labels, role names |
| faint | `#3c414e` | placeholders, dividers |
| url | `#93a7ff` | preview URL (underlined) |

- **Wordmark:** `code` (fg) + `light` (accent), bold.
- **Conversation:** lowercase muted role labels (`you`, `codelight`); fg body; tool rows render
  as `VERB   target  <mark>` where the mark is `✓` (ok) / `✗` (err) / `●` (warn, running).
- **Status bar:** `● ready|thinking · step N/20 · <project>`.
- Structure comes from whitespace, type, and glyphs — not frames.

## Layout

Vertical: `header(2) / body(min) / input(1) / status(1)`.
Body horizontal: `conversation(min) / rail(34)`, rail separated by a single left hairline.

## Rail: deploy-monitor state machine

- **Idle** — `PREVIEW ○ no preview yet` + `PROJECT` + `SESSION` (step count).
- **Building** — streaming build steps, active step in amber. *(v0.2)*
- **Ready** — `DEPLOYMENT ready · <time>`, steps `✓`, `PREVIEW ● live` + URL + `open · copy · logs`.
- **Failed** — `compile ✗`, the actual error, `logs · retry · fix`. *(v0.2)*

The `failed → fix` action closes the write → deploy → inspect → iterate loop: the monitor
surfaces the build error and the agent can act on it directly.

## Implemented now (v0.1)

- Full two-column layout + visual system, conversation rendering, tool lifecycle, status bar.
- Rail **Idle** (real) and **Ready** (via `--demo`).
- `codelight --demo` seeds a sample session to preview the UI offline (no Gateway key needed).
- Verified with `TestBackend` render tests.

## Deferred (v0.2+)

- Rail **Building**/**Failed** states — need the deploy tool + build-log stream (`vc-mcp`).
- Real project context (`vc-context`: framework + versions in header/rail).
- Cost/budget meter — needs `CostTracker`.
- Changed-files / diff section; rail focus-toggle to collapse for pure chat.

## Not doing (YAGNI)

- Three-pane IDE layout; markdown rendering of assistant text (later polish).
