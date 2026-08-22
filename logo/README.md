# LeagueRecord logo — "Capture frame"

Four corner brackets around a record dot. Drawn on a 100-unit grid, 12-unit strokes, square
terminals, one circle. Flat two-tone, so it survives a 16px tray icon and a monochrome
taskbar.

Colors are design-system tokens (`DESIGN.md`): brackets `--text` (`#FFFFFF`), dot `--rec`
(`#FF3C3C`), icon plate `--bg-raised` (`#0E0E12`).

## Files

| File | Use |
| --- | --- |
| `logo.svg` | Primary — white brackets, red dot, transparent |
| `logo-light.svg` | Light backgrounds — black brackets, `#E01E1E` dot |
| `logo-mono.svg` | Single color via `currentColor` — inherits text color |
| `logo-lockup.svg` | Mark + `LEAGUERECORD` wordmark, IBM Plex Mono 600, .14em tracking |
| `icon-source.png` | 1024×1024 master for `tauri icon`. Opaque plate, 18% inset |
| `logo-16.png`, `logo-32.png`, `logo-256.png` | Transparent, full-bleed, for the frontend |
| `tray-white-32.png` | Tray on dark (Windows, most Linux) |
| `tray-black-32.png` | macOS template image — the OS inverts it |

`PROMPT-for-claude-code.md` is the instruction to hand to Claude Code.

## Rules

- Never add a rounded container, shadow, gradient, glow or background plate. The only plate is
  the flat `#0E0E12` square already baked into `icon-source.png`.
- Never recolor the dot — `#FF3C3C` means recording.
- Never change the 18% inset in `icon-source.png`; platform masking clips the brackets without
  it.
- Minimum size 16px. Below that, use a single filled square in `--rec`.
- Clear space around the mark is 12 grid units (12% of the mark's width) on all sides.
- The lockup is the only non-square asset. Do not re-space the wordmark; tracking is .14em.

## Regenerating platform icons

```
npx tauri icon logo/icon-source.png
```

Overwrites all of `src-tauri/icons/`. Re-run this rather than editing individual PNGs.
