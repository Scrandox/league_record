# Handoff: LeagueRecord UI overhaul ("Console" direction)

## Overview

A visual overhaul of the LeagueRecord desktop app (Tauri + TypeScript + video.js). Same
information architecture and feature set as today — recordings list, video player with event
markers, metadata display, rename/delete, timestamps — rebuilt on a deliberate design system
instead of ad-hoc CSS.

The chosen direction is **Console**: pure black surfaces, square corners, one blue accent,
IBM Plex Sans for labels and IBM Plex Mono for every piece of data. Chrome is thin and
fixed-height; the video gets whatever space is left.

Two other directions ("Studio", "Cut") were explored and rejected. They are in
`Overhaul.dc.html` for reference only — do not implement them.

## About the design files

The `.dc.html` files in this bundle are **design references**, not production code. They are
static HTML/CSS prototypes showing intended look, spacing and states. Nothing in them should
be copied wholesale into the app.

The task is to reproduce these designs inside the existing LeagueRecord frontend
(`src/index.html`, `src/css/*.css`, `src/ts/*.ts`, video.js player + `videojs-markers`),
using the app's existing structure and libraries. The design uses no framework and no
component library, so it maps directly onto the current plain-CSS setup.

## Fidelity

**High fidelity.** Colors, type, sizes and paddings are final and exact. Recreate them
faithfully. Two things are deliberately placeholder:

- Video frames and library thumbnails are striped placeholders — real frames come from the app.
- No AI-generated imagery exists anywhere in the design; the only image asset is the app's own
  icon (`src-tauri/icons/32x32.png`).

## The important file

**`DESIGN.md` is the deliverable that matters.** It is the design system written as rules
Claude Code can follow for features that do not exist yet. Copy it to the repository root and
reference it from `CLAUDE.md` (see `CLAUDE-md-snippet.md`). `tokens.css` is the machine-readable
half of the same thing — real CSS custom properties plus the primitive classes, ready to drop
into `src/css/`.

Order of work:

1. Add `src/css/tokens.css` and import it first in `src/index.html`.
2. Add `DESIGN.md` at the repo root and the snippet to `CLAUDE.md`.
3. Rebuild the existing screens against the tokens: titlebar, command bar, recordings list,
   player controls, status bar, dialogs.
4. From then on, every new feature is designed by reading `DESIGN.md` — not by inventing.

## Screens in this bundle

`Console UI.dc.html` contains four labelled frames. Exact values for each are in `DESIGN.md`.

**01 Player — default view.** 1280×800 window, four fixed rows: 34px titlebar, 42px command
bar, flexible body, 26px status bar. Body is a two-column grid, `1fr / 320px`. Left column is
the video (16:9, centred) above a control block with the marker scrubber. Right column is the
recordings table (`NAME / R / K/D/A`), with the marker legend and TIMESTAMPS button pinned to
its bottom. Match metadata lives entirely in the status bar as a mono run:
`FAKER#KR1 · AHRI · 11/3/8 · 241 CS · 34 WS · RANKED SOLO · VICTORY`, with
`42 RECORDINGS · 14.62 GB` right-aligned.

**02 Library — grid view.** Same chrome. Body is a 5-column grid of 16:9 cards, 10px gap,
12px padding. Cards carry a result badge (W/L/R) top-left, favourite star top-right, duration
bottom-right, and two mono lines below the thumbnail (filename, then `CHAMPION · K/D/A`).
Toggled from the ▤/▦ pair in the command bar.

**03 Dialogs & panels.** Rename, Delete, Timestamps, and the state inventory (recording /
idle / saving pills, toast, empty list). Dialogs are bordered boxes with a 30px caps header
bar, not rounded cards. Delete borders and buttons use the recording red; everything else
uses the accent.

**04 Tokens.** The palette, marker colors, type and metrics rendered as a legend. It exists so
the swatches can be checked against `tokens.css` by eye.

## Interactions

Behaviour is unchanged from the current app except where noted:

- Clicking a table row or a grid card loads that recording into the player.
- ▤/▦ in the command bar switches list and grid views. The current app has no grid view; this
  is new UI over existing data.
- The search field filters by filename and champion. New.
- `ALL / FAVORITES / RANKED` are filter tabs with live counts. Replaces nothing — new.
- Row actions (✎ rename, ✕ delete) appear on hover and on the selected row. In the current app
  they are hover-only, which makes them hard to hit; showing them on the selected row too is
  the fix.
- Marker legend entries toggle marker visibility, same as today's eight checkboxes.
- Timestamps rows seek the player; `COPY ALL` copies the list, as today.
- Keyboard shortcuts stay as they are.
- No animation beyond 120ms color/border transitions on hover. Nothing slides, fades or scales.

## State

No new persistent state beyond what exists. Additions are view-local:
`viewMode: "list" | "grid"`, `filterTab: "all" | "favorites" | "ranked"`, `searchQuery: string`,
`recordingState: "idle" | "recording" | "saving"` (already known from the backend; the design
just surfaces it).

## Assets

- `assets/app-icon-32.png` — copied from `src-tauri/icons/32x32.png`. The only image.
- IBM Plex Sans and IBM Plex Mono. Loaded from Google Fonts in the prototypes; **bundle them
  locally** in the real app the way `BeaufortW01-Bold.ttf` is bundled today, since the app must
  work offline.
- Beaufort is dropped entirely.

## Files

- `DESIGN.md` — the design system. Goes to the repo root.
- `tokens.css` — CSS custom properties + primitives. Goes to `src/css/`.
- `CLAUDE-md-snippet.md` — text to paste into the repo's `CLAUDE.md`.
- `Console UI.dc.html` — the chosen direction, all screens. Open in a browser.
- `Overhaul.dc.html` — all three explored directions. Reference only.
- `Current UI.dc.html` — today's UI recreated from `src/css/*`, for before/after.
- `assets/app-icon-32.png`
