# Preferences page (My account › Preferences) vs live

ui2, 2026-10-24. App = ui2 build of 10-24 13:50 (tree 2d3d475e + the Preferences fixes below), the
Preferences window sized to 1920×1080; live = prefs/00-y0 … 05-y4500 (light, full page), 1860–1919
(profiles, Labs, export rules), sc-9xx (shortcut toolbars), dk-901…904 (dark, Theming → Units).
Live has the cloud subscription banner on top (+53 px), so positions are compared from each
section's rule. Status: ✅ matches, ✗ differs, ⊘ excluded (cloud), ? waiting on live.

## Light

| # | Section | Live | Ours | Status |
|---|---|---|---|---|
| P1 | Language | Display language select + [Save language] (pale until changed) | same; the label/select gap was 3 px short (fixed: control 25 px under the label) | ✅ |
| P2 | Theming | ◉ Light mode / ○ Dark mode [beta] (?) with 12 px `#777` notes, [Save theme] | same; notes were 11 px in the body colour, the radio a 13 px ring with a blue dot (fixed: 12 px `#777`, Bootstrap 14 px `#0d6efd` disc with a white dot, off `#ababab`) | ✅ (±2 px) |
| P2b | Disabled Save buttons | `#1651b0` at 40 % = `#a2b9df`, white text | `#c6d5ec` (25 %) (fixed) | ✅ |
| P3 | Decimal format | as live | as live | ✅ |
| P4 | Units | 11 unit rows + Time format, 72 px pitch, (?) per row; selects with Chrome's 8×5 `#777` arrow 10 px in | same layout; the selects had a "▼" text glyph 24 px in and long names ran under it (fixed: the shared `#777` caret, the value truncates) | ✅ |
| P5 | Mouse controls | View settings + table + 3 checkboxes + [Save mouse controls] | same (select caret fixed as P4) | ✅ |
| P6 | Environment profile settings | profile select + [Create profile] [Delete profile]; pixel density select; [Save profile settings] | same; label/select gap fixed as P1 | ✅ |
| P7 | Part Studio settings / Assembly settings | one checkbox each + Save | same | ✅ |
| P8 | Keyboard shortcuts | tabs General…Drawing + [Reset all], list + legend | same; live's list shows Chrome's permanent scrollbar, ours an overlay one | ✅ (scrollbar style is app-wide) |
| P9 | Shortcut toolbars › Sketch | Onshape's 56-row list: **Extrude, Revolve**, Line, …, "Center point rectangle", "Use", "Fillet", "Chamfer", …, Measure | started at Line with our tool names ("Center rectangle") (fixed: the live list and wording; ticks map to our tools) | ✅ list; ✗ ticking Extrude / Revolve / Construction / Section view / Mass and section properties / Measure has no effect on our sketch S toolbar yet |
| P10 | Toolbars, Drawings | [Reset to defaults]; Dark/Light view select + Save | same | ✅ |
| P11 | Material libraries | Library / Document / Owner at 425 px columns, rows 42 px, [Add material library…] 50 px under the row | columns split the width (≈384 px), rows 48 px, the button 34 px under (fixed to live's metrics) | ✅ |
| P12 | User export rules | (?) heading; "You have not added any export rules." 43 px under the rule; [Add export rule…] at 107 px | text 31 px, button 68 px (fixed) | ✅ |
| P13 | Onshape Labs | heading with the blue flask (on or off), checkbox + "Learn more." | flask only when Labs was on (fixed: always, live 1870/1919) | ✅ |
| P14 | Left list | Profile, Emails, Preferences, Notifications, …, Teams | Profile, Preferences | ⊘ (account / cloud pages) |
| P15 | Release management / Numbering settings | company settings, not on this page | in Company settings (separate window) | ✅ |

## Dark (live dk-904, then the full-page dk-pref-y0 … y4500 and states, behaviours "ui2 (10-24 11:30)")

| # | Item | Live | Ours | Status |
|---|---|---|---|---|
| D1 | Page, headings, selects (`#333` bg, `#888` border, `#777` arrow), checkbox, disabled Save (`#283f65` / `#858585`) | | same (pixel-sampled) | ✅ |
| D2 | Theme notes | `#bbbbbb` | white (fixed) | ✅ |
| D3 | Checked radio | `#0d6efd` disc + white dot; off ring `#878787` | light-mode style (fixed) | ✅ |
| D4 | Select arrow | `#777` (the note says white; the pixels are `#777`) | `#999` through the dark palette (fixed: static `#777`) | ✅ |
| D5 | (?) help icons | light `#d6d6d6` ring and "?" | light: #333 at 0.2 = #d6d6d6 on white; dark: dark/help-gray.svg solid #d6d6d6 (icons2 4722be7c) | ✅ |
| D6 | Focused select | 1 px `#3e94ff` border + 1 px `#90cef1` ring outside | 2 px `#3e94ff` (fixed) | ✅ |
| D7 | Open select list | `#333` list, `#858585` border, current row `#99c8ff` with black text, 27 px rows 15 px under | `#1967d2` / white, `#767676` border (fixed) | ✅ |
| D8 | Disabled select (pixel density, default profile) | `#666` on a `#ccc` border, `#777` caret | `#888` border, `#999` caret (fixed) | ✅ |
| D9 | Material libraries | rule `#555` under the header, Remove `#1651b0` | `#4b5761`, Remove `#3e94ff` (fixed) | ✅ |
| D10 | Modals (Create profile, Add export rule, Labs) | frame `#3c3c3c`, rules `#495057`, 12 px ✕ of 1.5 px strokes; "A unique profile name is required." `#ff6b6b` | light `#dee2e6` frame and rules, 8 px ✕, pink error (fixed; the ✕ in light too, the rules app-wide through DARK_HEX) | ✅ |
| D11 | Shortcut toolbars › Sketch list | no hover; ticks blue; select 17 px under the rule, list 6 px under it and 205 px high, Save 10 px under; icon and label 10 px apart | 1 px, 1 px, 204 px, 12 px; 8 px apart (fixed, light too: prefs/04 has the same offsets) | ✅ |
| D12 | Save after unticking again | stays enabled (the form stays dirty until saved) | went pale again (fixed: dirty until saved) | ✅ |
| D13 | Add export rule dialog | each label (bold + 18 px (?)) above its control: select 34 px, convention an ACE line 40 px high with 12 px mono text 17 px in, "N characters remaining" 14 px right under; placeholders blue (`#cda869` in dark) | labels in a 130 px column left of the controls, a 34 px field with 14 px text (fixed, light too: er-902) | ✅ |
| D14 | Labs consent | 24 px padding, 13 px text on 19.5 px lines, paragraphs 10.5 px apart, 16 px semibold subheadings, 5 px bullets (the same in light, 1873) | 10 px padding, 12 px text on 17 px lines, big ● bullets (fixed) | ✅ |
| D15 | Selects' left edge | x 261 | x 260 (Chrome places the box on a half pixel) | ✅ (±1 px) |
