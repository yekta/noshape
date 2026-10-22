# Workflow: Variable Studios driving a Part Studio (table order, name clashes, own variables)

qa5, 2026-10-21 pass 8, app /tmp/qa5/noshape-2256 (main 5bd50b2), doc qa5-vs.noshape (setup /root/qa5-scratch/pass8/mkvs.py + UI):
Part Studio "vs t" = 10×10 rectangle on Top extruded by `#w`; Variable Studios "vs all" (w = 10 mm, Insert into all ☑), "vs1" (w = 15 mm), "vs2" (w = 20 mm).
Live: behaviours.md "Variable Studio tables in a Part Studio: order and name clashes" (psb 10-20 t, ~line 2759), "Variable Studio cell right-click" (~1874).
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | CLI `variable_create` into a Variable Studio tab | – | **error "tab `vs all` is a Variable Studio, not a Part Studio"**: the agent API can create Variable Studio tabs but can't fill them | ✗ **API3** (api) |
| 2 | Variable Studio layout | "Insert Variable Studio" button, ⌄ table ⋯, Name / Variable type / Value / Description, italic "Name" row, footer "☑ Insert into all Part Studios and Assemblies \| ? ▴" | same; a new studio opens with the box ticked | ✅ (default tick ?) |
| 3 | "vs all" only | depth 10 | 10 mm cube; "vs all" table shown automatically | ✅ |
| 4 | Variable table › Insert Variable Studio › vs1, vs2 | new tables go below the inserted ones, above the studio's own table | vs all → vs1 → vs2 → vs t; dialog lists the studios, ✓ on inserted ones, Close | ✅ |
| 5 | Which w wins | lowest inserted table (vs2, 20) | 20 mm | ✅ |
| 6 | Shadowed rows | ⓘ in the Name cell, tooltip "Overwritten in another variable table."; winner none | same (vs all and vs1 rows) | ✅ |
| 7 | vs2 ⋯ menu | Copy table \| Rename \| Switch to vs2 \| Move up \| Collapse all tables \| Edit reference… \| Remove reference | same | ✅ |
| 8 | Move up (vs2 above vs1) | vs1 wins (15), at once | 15 mm, ⓘ moves to vs2 | ✅ |
| 9 | Drag vs1 header above vs2 | dark-blue pill ghost with the name; 1 px blue drop line with a small circle at its left | same; drop → 20 mm | ✅ |
| 10 | Own Variable `#w = 5 mm` before Extrude 1 | wins (5); all inserted w rows ⓘ; Extrude 1 stays black | 5 mm, ⓘ on all three, Extrude black | ✅ |
| 11 | Variable dialog: type the value, Enter | (Enter accepts the feature) | **Enter does nothing**; ✓ needed | ✗ **VS1** (psb; live check requested) |
| 12 | Drag the Variable row below Extrude 1 (or Extrude 1 above it) | ? (live: a Variable *after* the Extrude has no effect) | drop line shown, then **refused** with a red toast "Cannot reorder: Extrude 1 depends on Variable 1." (CLI feature_reorder: same error) | ? **VS2** (live check requested) |
| 13 | New Variable `#w = 30 mm` created after Extrude 1 | no effect | still 5 mm | ✅ |
| 14 | Reopen the document | order kept | vs all, vs1, vs2, vs t kept | ✅ |
| 15 | Own-table rows vs inserted rows | same table style | own rows: smaller value font, type ▾ shifted (p9-vs23) | ? VS3 (ui2, minor) |

## Update from live's answer "qa5 pass 8" (behaviours.md, 10-21) + re-check 10-22 00:15 on /tmp/qa5/noshape-2330 (target-shared 23:30)
| Row | Live | Ours | Status |
|---|---|---|---|
| 11 VS1 | Enter #1 in Value commits and normalises ("8" → "8 mm", fully selected), dialog stays open; Enter #2 accepts and closes (same two-step Enter as every feature dialog) | identical: Enter #1 → "8 mm" selected, title "#w = 8 mm", dialog open; Enter #2 closes, row "#w = 8 mm" (shots v03, v04) | ✅ **VS1 withdrawn** (pass 8 expected one-step Enter) |
| 12 VS2 | the reorder is **allowed, with no message** in both directions; Extrude 1 then fails red "Extrude 1 did not regenerate properly: Variable not found, is suppressed, or has not been set." (Parts (0)), or resolves silently to a Variable Studio's w when one is in scope; Ctrl+Z undoes it in one step | refused with the red toast "Cannot reorder: Extrude 1 depends on Variable 1." (UI and CLI) | ✗ **VS2** (psb) |
| 2 (default tick) | "Insert into all Part Studios and Assemblies" is ☑ only for the **first** Variable Studio in the workspace; every later one comes up ☐ | 4th studio (+ › Create Variable Studio in qa5-vs, which has vs all/vs1/vs2) comes up **☑** (shot v06) | ✗ **VS4** (psb), new |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status |
|---|---|---|
| VS2 reorder Variable below Extrude 1 (CLI feature_reorder on a copy of qa5-vs) | allowed, no error; Extrude 1 OK (w resolved from the in-scope Variable Studio), 2000 mm³ | ✅ **fixed** (UI drag not re-checked) |
| VS4 4th Variable Studio | "Insert into all…" ☐; inserted right after the active tab (= live) | ✅ **fixed** (v11) |
