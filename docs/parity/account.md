# My account › Preferences parity with Onshape

Onshape's user Preferences page (user menu › My account › Preferences,
`/user/settings`) and its state in noshape (the "My account" window,
`crates/app/src/preferences_window.rs` and the section files named below).

Sources: live prefs/00–05 (the whole 4824 px page), dom/preferences-full.txt
and .json (every control with its options), behaviours.md "My account ›
Preferences, full page".

Status: ✅ works · 🔧 fixed/added in this pass · ⏳ partial · ❌ missing · ☁ cloud (excluded).

## Sections, in live's order

| # | Section (live) | Controls | Status |
|---|---|---|---|
| 1 | Language | Display language ▾ [繁體中文, Français, English, Deutsch, 简体中文, 日本語, 한국어, Español, Italiano], Save language | ❌ Needs a localization system (every UI string) and translations: its own workstream (lead, 20 Oct). Not built as a half version (no English-only select). → now ✅ built with translations (app-core i18n; preferences.md P1, f9185639) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 2 | Theming | ◉ Light mode / ○ Dark mode (beta, ?), descriptions, Save theme | ❌ Routed to ui (theme.rs). → now ✅ Light / Dark mode with Save theme (preferences.md P2; dark-mode.md) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 3 | Decimal format | ☐ Use comma as decimal separator, Save decimal format | ⏳ ui2 (with psb for expression parsing and display). → now ✅ applied (user_units comma_decimal; preferences.md P3) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 4 | Units | 11 × (default unit ▾ + Display decimals ▾ + ?), Time format ▾ 12 hour / 24 hour, Save changes | 🔧 `units_prefs.rs`: new documents start with these units; document times follow the time format. → now ✅ preferences.md P4 (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 5 | Mouse controls | View settings ▾, the 3D / 2D table, 3 checkboxes, Save mouse controls | ✅ `mouse_prefs.rs` (see navigation.md §5). |
| 6 | Environment profile settings | Environment profile ▾ + Create / Delete profile; Match pixel density on high resolution displays ▾ Automatic / On / Off; Save profile settings | ❌ Routed to perf (renderer). → now ✅ profiles, Create / Delete, pixel density (preferences.md P6, c82818b7) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 7 | Part Studio settings | ☐ Display parts list properties, Save | ✅ |
| 8 | Assembly settings | ☐ Display instance list properties, Save | ✅ |
| 9 | Keyboard shortcuts | tabs, the list with Edit / Reset, legend, Reset all | ✅ |
| 10 | Shortcut toolbars | ▾ Sketch / Part Studio / Assembly / Drawing, tool checklist + preview, Save shortcut toolbar settings | ⏳ Sketch and Part Studio (Save applies the draft). Assembly and Drawing S toolbars are not customizable yet (skb / asb). Live's Sketch list starts with Extrude, Revolve and ends with Section view, Mass and section properties, Measure; ours lists sketch tools only. → now ✅ Sketch / Part Studio / Assembly / Drawing lists, live's Sketch list incl. Section view / Measure (preferences.md P9; SKETCH-S-INSPECT verified 87ddb793) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 11 | Toolbars | Reset to defaults | 🔧 resets the Part Studio and Assembly toolbar customizations. → now ✅ preferences.md P10 (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 12 | Drawings | Background color of model space ▾ Dark view (default) / Light view, Save drawing settings | ⏳ `drawing_prefs.rs` (asb): saved, not applied yet. → now ✅ applied: a Light view model space in drawings (drawing_view.rs light_model_space; preferences.md P10) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 13 | Material libraries | table Library / Document / Owner + Remove; Add material library… | ⏳ table as live (ui2 look); live's Remove on "Onshape Material Library" and Add material library… are missing (psb). → now ✅ Remove and Add material library… modal (material_prefs.rs, material_libraries.rs; preferences.md P11) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 14 | User export rules | "You have not added any export rules." + Add export rule… | ❌ Routed to kern (export). → now ✅ Add / Edit export rule (prefs_export_rules.rs; preferences.md P12) (skasm 10-31 re-check of the code against preferences.md, the current sheet) |
| 15 | Onshape Labs | ☐ Enable users to access experimental, proof of concept features. Learn more. | ☁ Cloud / AI (Co-complete): excluded. |

Not in live's user Preferences but on our page: "Release management" and
"Numbering scheme settings" (Onshape keeps them in Company settings, paid
plans; here they are local settings, psb).
