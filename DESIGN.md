# LeagueRecord — design system

This is the reference for all UI work in this app, including features that do not exist yet.
Read it before adding any screen, panel, dialog or control. If something you need is not
described here, build it from the primitives below rather than inventing a new treatment, and
add it here afterwards.

Tokens live in `src/css/tokens.css`. Use the custom properties, never raw hex.

## The idea in one line

A black, square-cornered instrument panel. Chrome is thin, quiet and fixed-height; the video
is the only thing allowed to be large. Every number is monospaced. One accent color, used
sparingly enough that it always means "this is selected or active".

## Rules

1. **Square corners.** `border-radius: 0` everywhere. The only exception is the window shell
   itself at 4px.
2. **Borders, not shadows.** Hierarchy comes from 1px hairlines and slightly different blacks.
   No `box-shadow` anywhere, no gradients, no blur, no glass.
3. **Mono for data, sans for language.** Any timecode, count, score, filename, size, resolution
   or ID is IBM Plex Mono. Sentences and labels a human reads as prose are IBM Plex Sans.
   When in doubt, it is data — this app is mostly data.
4. **Uppercase + tracking for chrome.** Buttons, tabs, column headers and section labels are
   uppercase Plex Mono with `letter-spacing: .08em–.14em`. Never uppercase body text.
5. **One accent.** `--accent` marks selection, focus and the primary action, and nothing else.
   If two things on screen are blue, one of them is wrong.
6. **Semantic colors are reserved.** Green means win, red means loss or recording, gold means
   favourite, and the eight marker colors mean their events. Never reuse them decoratively.
7. **Fixed-height chrome.** Bars are 34 / 42 / 26px. Controls are 26px, table rows 30px,
   dialog headers 30px. Do not introduce new heights; pick the nearest existing one.
8. **Density is the point.** 6 / 10 / 12 / 16px gutters. Nothing gets 24px of padding. This is
   a tool someone opens for thirty seconds.
9. **No motion.** 120ms color and border-color transitions on hover only. Nothing slides,
   fades, scales or spins. A spinner is a text percentage instead.
10. **Icons are text.** `▶ ⏴ ⏵ ⛶ ⌕ ★ ✎ ✕ ↵ ▤ ▦ ↗ ▾`, plus `U+1F50A` (volume) and `U+2702`
    (scissors, Auto-Clip), at 11–13px. If a real icon set gets
    added later, it must be a single-weight monoline set — no filled or duotone icons.
11. **Empty and error states are typographic.** A mono line stating the fact, one sans line of
    guidance, inside a dashed hairline box. No illustrations.
12. **No AI-generated imagery, ever.** The app's own icon is the only image in the chrome.

## Color

Surfaces, darkest first — the stack is `sunken < base < bar < raised`:

| Token | Value | Use |
| --- | --- | --- |
| `--bg-base` | `#000000` | window body, video letterbox, inputs |
| `--bg-sunken` | `#050507` | panels beside/below the video (sidebar, control block) |
| `--bg-bar` | `#08080A` | command bar, dialog bodies |
| `--bg-raised` | `#0E0E12` | status bar, inactive tabs, toasts |

Lines and text:

| Token | Value |
| --- | --- |
| `--line` | `rgba(255,255,255,.10)` — structural divisions |
| `--line-strong` | `rgba(255,255,255,.16)` — control and dialog outlines |
| `--line-soft` | `rgba(255,255,255,.06)` — between rows in a list |
| `--text` | `#FFFFFF` |
| `--text-2` | `rgba(255,255,255,.70)` — unselected rows |
| `--text-3` | `rgba(255,255,255,.50)` — secondary labels |
| `--text-4` | `rgba(255,255,255,.35)` — column headers, disabled, placeholders |
| `--text-5` | `rgba(255,255,255,.30)` — the faintest legible tier |

Accent and status:

| Token | Value | Meaning |
| --- | --- | --- |
| `--accent` | `#5B8CFF` | selection, focus, primary action |
| `--accent-fill` | `rgba(91,140,255,.14)` | selected row background |
| `--on-accent` | `#000000` | text on an accent fill |
| `--win` | `#5FD67A` | victory |
| `--loss` | `#FF5A5A` | defeat |
| `--rec` | `#FF3C3C` | recording, destructive action |
| `--favorite` | `#E8C86A` | starred |
| `--neutral-result` | `rgba(255,255,255,.40)` | remake / unknown |

Event markers — these are the app's signature and must stay stable across features. `--m-*`:

| Event | Token | Value |
| --- | --- | --- |
| Kill | `--m-kill` | `#5FD67A` |
| Death | `--m-death` | `#FF3C3C` |
| Assist | `--m-assist` | `#2F8F57` |
| Structure | `--m-structure` | `#4F7CF7` |
| Dragon | `--m-dragon` | `#D8A13A` |
| Herald | `--m-herald` | `#D24FD2` |
| Atakhan (legacy) | `--m-atakhan` | `#A12A2A` |
| Baron | `--m-baron` | `#8A5CF0` |
| Highlight | `--m-highlight` | `#3FBFB4` |

A new event type needs a new token here, distinguishable from all eight above, plus a legend
entry and a scrubber marker. Do not reuse an existing color for a new event.

Atakhan was removed from League; the token stays because recordings from past seasons still
contain his events (they render on the scrubber and in the timestamps dialog, but the event
has no legend toggle any more). Retired events keep their token for the same reason.

## Type

Two families, bundled locally (the app must work offline):

- **IBM Plex Sans** 400 / 500 / 600 — prose, dialog copy, guidance text.
- **IBM Plex Mono** 400 / 500 / 600 — all data, all uppercase chrome labels.

The whole app fits in four sizes. Do not add a fifth.

| Role | Spec |
| --- | --- |
| Micro label / column header | Mono 600, 9.5px, `letter-spacing:.10em`, uppercase, `--text-4` |
| Chrome control / tab / button | Mono 500–600, 10.5px, `letter-spacing:.06–.08em`, uppercase |
| Body data / table row | Mono 400, 11.5px, `--text-2`, `--text` when selected |
| Timecode / emphasis | Mono 500, 12px, `--text` |
| Prose | Sans 400, 11–11.5px, `line-height:1.55`, `--text-3` |
| Wordmark | Mono 600, 11px, `letter-spacing:.14em`, `--accent` |

Status-bar and legend text may drop to Mono 400 10px. Nothing goes below 9.5px.

## Metrics

| Token | Value |
| --- | --- |
| `--h-titlebar` | 34px |
| `--h-cmdbar` | 42px |
| `--h-statusbar` | 26px |
| `--h-control` | 26px — every button, tab, input and pill |
| `--h-row` | 30px — table rows, dialog headers |
| `--pad-1 … --pad-4` | 6 / 10 / 12 / 16px |
| `--sidebar-w` | 320px |
| `--radius` | 0 |
| `--radius-window` | 4px |

Horizontal padding inside a bar is 10px. Padding inside a dialog body is 14px. Gap between
sibling controls is 8px; between segments of a joined control group, 1px.

## Layout

Every window is a single-column grid of fixed rows with one flexible body:

```
grid-template-rows: 34px 42px 1fr 26px;   /* titlebar, command bar, body, status bar */
```

The body then splits. Player: `grid-template-columns: 1fr 320px`. Library: a 5-column card
grid with 10px gaps. A future settings screen should use `grid-template-columns: 200px 1fr`
(nav / pane) and keep the same four outer rows.

Never let the video pane shrink to add chrome. If a new panel needs room, put it in the 320px
column, in the status bar, or in a dialog.

Auto-Clip output lives in a `Clips` subfolder of the recordings folder and shares the library
with full games — one list, one player, one set of row actions, separated by the `FULL GAME`
and `CLIPS` tabs rather than by a second screen. A clip carries its folder in its id
(`Clips/name.mp4`), so it displays under its bare filename everywhere a name is shown. Clips
count toward the size total in the status bar but are never removed by the age or size
cleanups: the user chose each one by hand.

## Components

**Titlebar** — 34px, `--bg-base`, bottom `--line`. App icon 16px, wordmark, then a faint mono
context word (`v1.9.2`, `LIBRARY`, `SETTINGS`). Window buttons `– □ ✕` right, `--text-4`, 14px
gaps. Draggable region.

**Command bar** — 42px, `--bg-bar`, bottom `--line`, 8px gaps. Left to right: search input,
filter tabs, view toggle, then flexible space, then status pill, then secondary buttons.
New global actions belong here, right-aligned, as 26px outlined buttons.

**Search input** — 26px, `--bg-base`, 1px `--line-strong`, 9px horizontal padding, `⌕` in
`--text-4`, Mono 400 11.5px. Focus: border `--accent`. No radius, no icon button.

**Filter tabs** — joined group, 1px gaps, no radius. Active: `--accent` fill, `--on-accent`
text, weight 600. Inactive: `--bg-raised`, `--text-3`, weight 500. Counts are part of the
label (`ALL 42`). The four tabs are `ALL`, `FAVORITES`, `FULL GAME` and `CLIPS`; the last two
split the library by kind, since a clip and the game it came from are different things to
look for. Filtering stays in the frontend — the tabs never re-query the backend.

**Segmented icon toggle** — 26×26px squares, same active/inactive treatment as tabs.

**Buttons** — 26px (28px in dialogs), 10–12px padding, Mono 500–600 10.5px uppercase.
Secondary: transparent with `--line-strong` border, `--text-2`. Primary: `--accent` fill,
`--on-accent` text, weight 600. Destructive: `--rec` fill, black text. Accent-outline
(`TIMESTAMPS`): `--accent` border and text, transparent fill. Hover raises border and text one
tier; no fill change on outlined buttons.

**Table** — header row of micro labels over `--line`; rows 30px, 7px/10px padding, divided by
`--line-soft`. Right-align numeric columns. Selected row: `--accent-fill` plus a 2px left
`--accent` border and `--text`. Hovered row: `rgba(255,255,255,.03)`. Row actions (`✎ ✕`) sit
in the last column, shown on hover and on the selected row, `--text-3`, 8px apart.

**Card (grid item)** — `--bg-sunken`, 1px `--line`, `--accent` border when selected. 16:9
thumbnail with 6px-inset overlays: result badge top-left (10% tint of the result color, 9px
Mono 600), favourite star top-right, duration bottom-right on `rgba(0,0,0,.7)`. Below:
7px/8px padding, filename Mono 400 11px truncated, then `CHAMPION · K/D/A` Mono 400 10px
`--text-4`.

**Scrubber** — 22px band. 3px track `rgba(255,255,255,.12)`, played portion `--accent`, 2px
white playhead full band height. Markers are 3×15px bars in their `--m-*` color, no radius,
top-aligned 3px into the band. Markers sit above the track, never inside it. When two markers
land within 4px, keep both — do not merge or round.

**Player controls** — one 16px-gap row under the scrubber: transport glyphs 13px white
(inactive ones `rgba(255,255,255,.55)`), then `HH:MM:SS / HH:MM:SS` in Mono 500 12px with the
total in `--text-5`, then flexible space, then speed as `1.00×` and fullscreen `⛶`.

**Status bar** — 26px, `--bg-raised`, top `--line`, Mono 400 10px `--text-4`, 18px gaps. Facts
left, the emphasised one in `--text` or its semantic color; totals right-aligned. This is where
persistent context goes — never a floating badge.

**State pill** — 26px, 10px padding, 7px gaps, 1px border and a 10% fill in its own color, with
a 7×7px square dot. Recording uses `--rec`, saving `--accent`, idle `--line-strong` with
`--text-3`. Always carries a value (`RECORDING · 24:11`, `SAVING · 92%`).

**Dialog** — `--bg-bar` box, 1px `--line-strong` (`--rec` at 40% for destructive), no radius,
no shadow. 30px header: micro caps title left, `✕` right, bottom hairline. 14px body. Actions
bottom-right, 6px gaps, cancel then primary. Backdrop is `--bg-base` at 90%. Width 320–420px.

**Auto-Clip dialog** — a `Dialog` at the narrow end of the width range, opened from the `U+2702`
row action. Body is `.lr-clip-categories`: a two-column grid, 6px row gap and `--pad-2` column
gap, of the same `.legend-item` checkbox rows the marker legend uses, so a category reads with
its own `--m-*` swatch and never a second color. Below it `.lr-clip-summary` — hairline top
border, `--pad-1` of clearance, Mono 500 `--fs-time` `--text` — restating the plan as
`N CLIPS · MM:SS TOTAL`, recomputed on every toggle. During the cut the same line becomes
`CLIPPING · 3/12` and every control in the dialog is disabled; a failure replaces it with the
error text and re-enables them. Actions are CANCEL then CREATE CLIPS.

**Toast** — `--bg-raised`, 1px `--line-strong`, 2px left border in the relevant semantic color,
9px/11px padding, ~262px wide. Mono 600 11px title line, Mono 400 10px `--text-4` detail. One
per event class, not one per event.

**Empty state** — 1px dashed `rgba(255,255,255,.14)`, 22px/12px padding, centred. Mono 11.5px
`--text-3` fact, then Sans 10.5px `--text-4` guidance sentence.

**Checkbox** — native, `accent-color: var(--accent)`, label in Mono 10.5px uppercase.

## Adding a feature

1. Which of the four outer rows does it belong to? Global action → command bar. Persistent
   fact → status bar. Per-recording detail → the 320px column. Transient decision → dialog.
   Anything else needs a reason.
2. Reuse a component above verbatim. A new component is a last resort, and it must be
   black-surfaced, square, hairline-bordered and fixed-height.
3. Data in mono, labels in uppercase mono, prose in sans. Four type sizes only.
4. If it needs a new color, it is either an event (add an `--m-*` token) or it does not need a
   new color.
5. Check the result against `Console UI.dc.html`. If it looks softer, rounder, larger or more
   colorful than that file, it is wrong.
