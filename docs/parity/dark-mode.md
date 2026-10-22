# Dark mode (Preferences › Theming) vs live

ui2 owns it (lead 10-22). Live: behaviours "Dark theme (ui 20 Oct)", dom/dark-tokens-table.md, dom/dark-os-vars-computed.json,
dark/dk-9xx (1:1 captures). **Done = every studio and dialog pixel-close to these captures** (lead condition).

## Mechanism (in main once merged)
- `theme::PALETTE_DARK` + `theme::p()` (palette roles); `theme::DARK_HEX` through the patched gpui `rgb()` / `rgba()`
  (one relaxed atomic load in light mode, binary search in dark, no allocation); `gpui_kit::with_light_colors` for
  parts that stay light (drawing editor); `gpui_kit::rgb_static` for `--static` colours.
- Typing cost (UI thread, 200 keys into the feature filter, --demo): before the hook 4.90 / 4.90 / 4.70 ms per key;
  with the hook, same binary, alternating: light 5.10 / 5.85 / 5.60 / 5.30 / 6.00 / 6.50 (mean 5.73),
  dark 5.60 / 5.15 / 4.60 (mean 5.12). Run-to-run noise (±0.7 ms, shared machine) is larger than any hook cost;
  dark is not slower than light, and light pays only one relaxed load per colour.
- Step 1 state (dk0 shot): chrome, panels, tabs, filter and toolbar background go dark; still open: icons
  (dark glyphs, need the dark asset path from icons2), viewport background and plane colours, then per-capture
  side by side below.

## Side by side (live capture | ours | notes | status)
| capture | ours | notes | status |
|---|---|---|---|
| dk-910-documents | | | ☐ |
| dk-911-documents-row-hover | | | ☐ |
| dk-912-documents-row-selected | | | ☐ |
| dk-913-create-menu | | | ☐ |
| dk-913b-create-menu-hover | | | ☐ |
| dk-914-new-document-modal | | | ☐ |
| dk-915-state | | | ☐ |
| dk-920-partstudio | | | ☐ |
| dk-921-partstudio | | | ☐ |
| dk-922-feature-hover | | | ☐ |
| dk-923-feature-selected | | | ☐ |
| dk-924-feature-rmb | | | ☐ |
| dk-925-tool-tooltip | | | ☐ |
| dk-926-feature-dialog | | | ☐ |
| dk-927-variable-table | | | ☐ |
| dk-928-configurations | | | ☐ |
| dk-929-face-selected-measure | | | ☐ |
| dk-930-face-hover | | | ☐ |
| dk-931-sketch-line | | | ☐ |
| dk-932-sketch-dimension | | | ☐ |
| dk-933-sketch-after | | | ☐ |
| dk-940-assembly | | | ☐ |
| dk-941-mate-dialog | | | ☐ |
| dk-942-bom | | | ☐ |
| dk-950-drawing | | | ☐ |
| dk-951-drawing-properties | | | ☐ |
| dk-960-feature-studio | | | ☐ |
| dk-961-variable-studio | | | ☐ |
| dk-990-restored-light | | | ☐ |

## Plan notes
# Dark mode (lead 10-22: ui2 owns it; coordinate in ui's handoff theme-layout.md)

Live: behaviours "Dark theme (ui 20 Oct)", dom/dark-tokens-table.{md,json} (287 themed tokens), dark/dk-9xx.
Switch applies immediately (no reload). Drawings: only the shell is dark (toolbar, sheet surround, sheet, Drawing
properties stay light). Feature Studio: ACE twilight colours. `--…--static` tokens keep light values.

## Layers
1. **Palette** (theme.rs, ui's file): `PALETTE_DARK` with each field's dark token (comments name the token),
   `theme::set_dark(bool)` (AtomicBool), `p()` returns the active one. ~46 fields; values from the token table:
   app_bg #333, panel #333, panel_raised #333, panel_hover #666, panel_active #555, tool_hover #666,
   tool_hover_group #555, tool_active #134e96?, rail #2c2c2c (left strip measured (44,44,44)), focus #3e94ff,
   text #fff, text_muted #ccc, text_faint #999?, accent #3e94ff, accent_soft #134e96 (selected row (19,78,150)),
   row_hover #0a4780, tooltip …, tab_bar #555 (measured (81,81,81)), tab_inactive …, menu_outline #ccc,
   dialog_bg #333 (95%?), info_fill #094174 / #142434, border #555, border_strong #888, search_fill (85,88,96)…
   Each from the table, then checked against dk-9xx pixels.
2. **Hard-coded literals** (~1800 `rgb(0x…)` / `rgba(0x…)` across every agent's files): a hook in the patched
   gpui (`vendor/gpui-pre/src/color.rs`): `set_hex_theme(Option<fn(u32) -> u32>)` consulted by `rgb()` / `rgba()`
   (alpha kept), plus `with_light_colors(|| …)` (thread-local depth) for subtrees that stay light (drawing
   canvas + its toolbar/panels, the sheet; images; viewport-matched swatches such as appearance colours).
   The map = the token table's majority light→dark value for each light hex (37 of 70 light hexes are
   ambiguous: #fff is background (→#333) and text-on-blue (stays #fff); #1651b0 accent (→#3e94ff) vs primary
   button fill (stays)). Ambiguous call sites switch to `theme::fixed(hex)` (= Onshape's `--static`), found by
   comparing every dk-9xx capture side by side.
3. **Icons:** icons2 captured dark icon sources (icons2-dark-*.png, -sprite.svg): mono icons recolour via
   their `currentColor` / fill → icon tint from the palette; check icons2's notes (behaviours 6244…).
4. **Viewport:** background stays per live (the graphics area #333? live "graphics background … #333";
   default planes fill (76,76,76) edges (100,100,100)) → scene palette `viewport` dark set (noshape_scene).
5. **Feature Studio** ACE-like colours: twilight token set in feature_studio.rs highlighting.
6. Refresh: `set_dark` + `cx.refresh_windows()`; cached slices must re-render (notify every view: add a global
   theme epoch the Slice cache keys on).

## Steps
a. palette + toggle + Preferences Theming wiring (pref `dark_mode`), b. gpui hook + majority map + light scope for
drawings, c. side-by-side pass over dk-910…961 fixing exceptions per area, d. viewport + FS + icons.
Tell ui in theme-layout.md before touching theme.rs / vendor color.rs.

## Icons (detail)
Live: one sprite for both themes; colours from `--os-icon-*` vars (outline-primary #333→#fff, fill-primary
#fff→#333, accent-primary #1651b0→#3e94ff, accent-tertiary #333→#fff; `--static` keep light; full ~70 vars per
theme in dom/icons2-dark-icon-fills.json). Ours: literal fills/strokes (#333 ×985, #000 ×426, #fff ×196, #1651B0
×149 …). → `AppAssets::load("icons/dark/<name>.svg")` = the light SVG with colours rewritten by that var map
(per-icon exceptions from icons2-dark-icon-fills.json where an icon uses a --static var); `assets::icon()` /
`icon_path()` return the dark path while dark (separate paths keep GPUI's raster cache per theme). Tell icons2.

## Lead conditions (10-22)
- Light mode: one relaxed atomic load in rgb()/rgba() (drafts/color_hook.rs: `HEX_THEME_ON` + `#[cold]` slow path).
- Dark: static sorted `(light, dark)` array, binary search, no allocation.
- Measure typing-frame cost before/after: script `typing-bench.sh`: app on --demo, focus the feature filter box,
  `xdotool type --delay 30` 200 chars, read utime+stime of the app (/proc/<pid>/stat, ticks) before/after → CPU ms
  per keystroke; 3 runs each: (a) before the hook (current main), (b) hook in, light, (c) hook in, dark.
  Also NOSHAPE_PERF=1 logs if they carry a UI frame stage.
- Done = every studio and dialog side by side with dark/dk-9xx captures, pixel-close (keep a table in
  docs/parity/dark-mode.md with each capture, our crop, diff notes).
- Baseline (10-22 ~05:40, main 5bd50b2 + WIP, before the hook): UI thread 4.90 / 4.90 / 4.70 ms CPU per key
  (200 keys into the feature filter, --demo; whole process ≈ 120 ms/key = llvmpipe, not comparable).

