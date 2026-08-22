Replace the app logo with the new "Capture frame" mark.

The new logo lives in `logo/` at the repo root. It is a flat geometric mark — four white
corner brackets around a red record dot — drawn on a 100-unit grid with 12-unit strokes and
square terminals. It matches the app's new design system (see `DESIGN.md`): square corners,
one flat shape, no gradients, no shadows, no rounded container.

Source files:

- `logo/logo.svg` — primary, white brackets + `#FF3C3C` dot, transparent background
- `logo/logo-light.svg` — for light backgrounds, black brackets + `#E01E1E` dot
- `logo/logo-mono.svg` — single-color, uses `currentColor` for both shapes
- `logo/logo-lockup.svg` — mark + `LEAGUERECORD` wordmark in IBM Plex Mono 600
- `logo/icon-source.png` — 1024×1024, `#0E0E12` background, mark inset 18%; this is the
  master for generating platform icons
- `logo/logo-16.png`, `logo/logo-32.png`, `logo/logo-256.png` — transparent, full-bleed
- `logo/tray-white-32.png`, `logo/tray-black-32.png` — single-color tray variants

Do the following:

1. Regenerate the full Tauri icon set from the master:

   ```
   npx tauri icon logo/icon-source.png
   ```

   This overwrites everything in `src-tauri/icons/` — `32x32.png`, `128x128.png`,
   `128x128@2x.png`, `icon.png`, `icon.ico`, `icon.icns`, all `Square*Logo.png`, `StoreLogo.png`,
   and the `android/` and `ios/` sets. Confirm the generated `icon.ico` contains the usual
   16/32/48/256 sizes and that `32x32.png` still reads clearly at 100% zoom.

2. Point the system tray at the new mark. Find the tray icon setup in `src-tauri/src/` (the
   `TrayIconBuilder` / `tray_icon` code) and make sure it uses the regenerated icon. On Windows
   and Linux the tray sits on a dark background, so `logo/tray-white-32.png` is the right
   choice if a dedicated tray asset is wanted; on macOS use `tray-black-32.png` as a template
   image so the OS can invert it. Copy whichever variants you use into `src-tauri/icons/` and
   reference them from there — do not read from `logo/` at runtime.

3. Use the mark in the frontend titlebar. Copy `logo/logo-32.png` (or `logo/logo.svg`, which is
   smaller and scales better) into `src/` alongside the other frontend assets, and render it at
   16×16 in the titlebar next to the `LEAGUERECORD` wordmark, with a 10px gap. If the current
   `src/index.html` has no titlebar yet, this happens as part of building the titlebar from
   `DESIGN.md` — do not invent separate logo styling for it.

4. Update any favicon or `<link rel="icon">` reference in `src/index.html` to the new file.

5. Remove the old icon source from the repo if one exists outside `src-tauri/icons/`, and
   delete `src/css/BeaufortW01-Bold.ttf` along with its `@font-face` rule in
   `src/css/base.css` — the new identity does not use Beaufort anywhere.

Constraints:

- Do not add a rounded-rectangle container, drop shadow, gradient, glow, or background plate
  to the mark. The only background it ever sits on is the flat `#0E0E12` square already baked
  into `icon-source.png`.
- Do not recolor the dot. `#FF3C3C` is the recording red from the design system and carries
  meaning; the brackets are white (or `currentColor` in the mono variant) and nothing else.
- Do not scale the mark inside `icon-source.png`. The 18% inset is deliberate so platform
  masking on macOS and Windows does not clip the brackets.
- Keep the aspect ratio at 1:1 everywhere. The lockup is the only asset that is not square.

When done, show me: the regenerated `src-tauri/icons/32x32.png` and `icon.ico` at 100%, the
titlebar with the mark in place, and the tray icon in the system tray.
