# Preferences page (My account › Preferences) vs live

ui2, 2026-10-22, app /tmp/ui2/noshape (tree 5bd50b2 + WIP), window 1919×1079 vs live prefs/00-y0 … 05-y4500
(full page, Free plan), er-9xx (export rules), sc-9xx (shortcut toolbars). Status: ✅ matches, ✗ differs, ⊘ excluded (cloud).

| # | Section / item | Live | Ours | Status |
|---|---|---|---|---|
| P1 | **Language** (first section) | "Display language" select (English) + [Save language] (pale until changed) | missing | ✗ P1 (ui2) |
| P2 | **Theming** | ◉ Light mode "Default color scheme for Onshape featuring dark elements on light backgrounds." / ○ Dark mode [beta] ⓘ "Low luminance color scheme…" + [Save theme] (live dt-000/dk-901…904) | missing; the app has no dark theme | ✗ P2 (ui2 page + ui theme) |
| P3 | **Environment profile settings** ⓘ (after Mouse controls) | Environment profile [Onshape (default) ▾] [Create profile] [Delete profile] (pale); "Match pixel density on high resolution displays (can affect performance)" [Automatic (default) ▾]; [Save profile settings] | missing (capture of its dialogs requested earlier, REQUESTS ui2 22:10) | ✗ P3 (ui2) |
| P4 | **User export rules** ⓘ (after Material libraries) | "You have not added any export rules." + [Add export rule…]; rules table, add / edit dialog with validation, effect on the export dialogs' file names (live er-900…931) | **missing** (the Export dialogs' "View export rules" link scrolls to the page end, which is Material libraries) | ✗ P4 (ui2) |
| P5 | **Onshape Labs** 🧪 (last) | ☐ "Enable users to access experimental, proof of concept features. Learn more." | missing | ✗ P5 (ui2; lead: keep or drop?) |
| P6 | Release management / Numbering scheme settings | not on the user Preferences page (a paid-plan company setting) | two extra sections before Material libraries | ✗ P6 (lead decides: Release is kept as a paid feature; where its settings live) |
| P7 | Shortcut toolbars › Sketch list | starts **Extrude, Revolve**, then Line, Midpoint line, Corner rectangle… (live 04, sc-9xx) | starts Line, Midpoint line… | ✗ P7 (ui2) |
| P8 | Left list | Profile, Emails, Preferences, Notifications, Security, Devices, Applications, Integrations, Developer; Subscription…; Teams | Profile, Preferences | ⊘ (account / cloud pages) |
| P9 | Decimal format, Units, Mouse controls, Part Studio / Assembly settings, Keyboard shortcuts, Toolbars, Drawings, Material libraries | as live | same layout and controls | ✅ |
