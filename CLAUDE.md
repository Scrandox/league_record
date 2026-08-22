## UI work — read DESIGN.md first

This app has a design system. `DESIGN.md` at the repo root is the single source of truth for
every visual decision: palette, typography, metrics, component specs, and the rules for adding
something new. `src/css/tokens.css` is its machine-readable half.

Before writing or changing any UI:

- Read `DESIGN.md`. Follow the "Adding a feature" checklist at the end of it.
- Use the `--*` custom properties from `src/css/tokens.css`. Never hardcode a hex value, font
  size, or bar height in a component stylesheet.
- Reuse the primitive classes (`.lr-btn`, `.lr-row`, `.lr-tab`, `.lr-pill`, `.lr-dialog`, …)
  before writing new CSS. A new component is a last resort.
- Non-negotiables: square corners, hairline borders instead of shadows, IBM Plex Mono for all
  data and IBM Plex Sans for prose, one blue accent reserved for selection/focus/primary
  action, four fixed bar heights, no motion beyond 120ms hover transitions, no gradients, no
  AI-generated imagery.
- Event marker colors (`--m-*`) are stable API. A new event type gets a new token plus a legend
  entry and a scrubber marker; it never borrows an existing color.
- `design_handoff_console_ui/Console UI.dc.html` is the visual reference. Open it and compare.
  If your result looks softer, rounder, larger, or more colorful than that file, it is wrong.

If a feature needs a treatment `DESIGN.md` does not cover, build it from the primitives, then
add the new component's spec to `DESIGN.md` in the same commit. Keeping that file current is
part of the work, not documentation debt.
