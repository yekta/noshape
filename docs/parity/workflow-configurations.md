# Workflow: configurations & variables (a CAD user's session, step by step against live Onshape)

Document "qa5 config" (mm): Part Studio 1 = 40×20×10 block (Sketch 1 + Extrude 1, made with noshape-cli,
/root/qa5-scratch/mkblock.py), Assembly 1. Everything else by mouse/keyboard.
Live reference: behaviours.md "Part Studio Variable table + measure (x) + Create variable popup", "Configure
features UI and parameter context menus", "Variable Studio tables in a Part Studio", "Drag cues … configured
insert", "Input behaviour › Commit, units and decimals".
Status: ✅ matches live, ✗ differs (finding #), ? not verified.

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206.

## 1. Variable table

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1.1 | Strip (x) › Variable table | section "▾ Part Studio 1 ⋯", Name / Variable type / Value / Description, italic Name add row, footer Create Variable Studio ▴ | same, plus an empty-state illustration | ✅ |
| 1.2 | Name "2x" | pink cell, red border, tooltip "Variable name must be a letter followed by a string of letters or numbers."; Enter does nothing | same | ✅ |
| 1.3 | "w" Enter | Variable feature at the rollback bar, "(x) #w = 0 mm", row w / Length / 0 mm | same | ✅ |
| 1.4 | "w" again | created as w_0 + info bubble "Renamed some new variables to avoid name conflicts." | same text; **the bubble never goes away** (still there 5 min later) and later **covers the feature dialog header (title, ✓ ✕)** | ✗ C5 |
| 1.5 | Value "30" Enter | "30 mm" | "30 mm" | ✅ |
| 1.6 | Type list | Length, Angle, Number, Any, Measured, From import | same | ✅ |
| 1.7 | Value "#h * 3" (h from a Variable Studio) | accepted | accepted; list "#w = #h * 3" | ? (list label) |
| 1.8 | Double-click Extrude 1 **while the Variable table panel is open** | edit dialog opens | **row only selected, no dialog**; the keys typed next went to the viewport ("#" = Shift+3 → Left view) | ✗ C4 |

## 2. Create variable popup, configuration variable

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 2.1 | Extrude Depth "#d" Enter | "Create variable" popup next to the field: Feature / Configuration; Length / Angle / Number / Any; Name "d"; Value = old value selected | same (core F6 fixed) | ✅ |
| 2.2 | Configuration tab | Length / Angle / Integer / Real / Text; Name, Default 10 mm, Minimum 0 mm, Maximum 100000 mm | same | ✅ |
| 2.3 | ✓ | configuration variable created at once; "Configurations" box above the feature list (d 10 mm); Depth shows "#d" | same | ✅ |
| 2.4 | Configurations box: d "25" Enter | "25 mm" (bare number gets the unit) | "25" (no unit); geometry updates to 25 | ✗ C8 |
| 2.5 | Configuration panel (strip) | tabs Configurations / Configured properties, Filter inputs + icons + ⋯; input sections | same; d section "d (#d) Default: 10 mm" with Usages: Extrude 1 Depth | ? |

## 3. List input and Configure features

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 3.1 | Add configuration input (List) | new input named **"Configuration"** (assembly flow) | named **"List 2"** | ✗ C1 |
| 3.2 | Name rows | Default (blue, left bar), add row "Name"; blue notice "No features are configured / Right click sketch dimensions, features, or feature parameters to configure them." | same; added "Large" | ✅ |
| 3.3 | + Configure features | yellow banner "Select sketches or features to configure [Done]", toolbar greyed | same (core F21 fixed) | ✅ |
| 3.4 | Click Extrude 1 | dialog in configure mode: **only ✕**, extra ☑ Unsuppressed, banner "Select parameters to configure" | banner ✅, ☑ Unsuppressed ✅ (after the first click); the Depth field shows **"25 mm"** (evaluated) instead of "#d" | ✗ C3 |
| 3.5 | Click Symmetric | configured at once: dotted amber outline, grey **"Configured"** tag, column "Extrude 1 / Symmetric", toast "Extrude 1 Symmetric added to Size" | outline, column and toast ✅; **no "Configured" tag**; the click also **ticked the checkbox and the preview went symmetric** while both table rows stay ☐ | ✗ C2 |
| 3.6 | Done; tick Large; select Large | part becomes symmetric | yes | ✅ |
| 3.7 | Toast position | top centre of the window (814,85 at 1920) | centred over the graphics area, covering the feature dialog's header and ✓ ✕ at 1440 px | ✗ C5 |

## 4. Variable Studio

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 4.1 | + › Create Variable Studio | tab with table, Insert Variable Studio, ☑ Insert into all Part Studios and Assemblies | same | ✅ |
| 4.2 | Row h = 15 | "15 mm" | same | ✅ |
| 4.3 | Part Studio Variable table | inserted table "Variable Studio 1" above "Part Studio 1" | same | ✅ |

## 5. Configured part in an assembly

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 5.1 | Insert dialog, Part Studio row | configuration inputs under the row, "Set to last viewed configuration" button; inserts with the default configuration | inputs (d, List 2 ▾) + a configuration button | ✅ / ? |
| 5.2 | List 2 = Large, insert Part 1 | instance in Large | instance symmetric about the origin (Large) | ✅ |
| 5.3 | Instance menu | (live list for configured instances not captured) | adds "Change configuration…" → custom panel with chips and a blue **Apply** button (not an Onshape dialog) | ? C7 |
| 5.4 | Plane rows Top / Right highlighted and the plane drawn orange after tab switches / typing in a panel, without clicking them | – | seen twice, no clean repro yet | ? C6 |

## Findings (2026-10-21 run)

| # | Finding | Repro | Owner |
|---|---|---|---|
| C2 | Configure mode: clicking a checkbox parameter also toggles it (preview changes) while the configuration table keeps the old values; no "Configured" tag | + Configure features › Extrude 1 › click Symmetric | psb |
| C4 | Feature-row double-click doesn't open the edit dialog while the Variable table panel is open (subsequent typing hits viewport shortcuts) | open (x) panel, double-click Extrude 1 | psb |
| C3 | In configure mode the Depth field shows the evaluated value (25 mm) instead of the expression #d | §3.4 | psb |
| C5 | Info bubbles/toasts never auto-dismiss and sit over the feature dialog header (✓ ✕) at 1440 px; same as SM10 | §1.4, §3.7 | ui |
| C8 | Configuration variable field in the Configurations box: bare "25" stays unitless (live appends "mm") | §2.4 | psb / ui (shared field) |
| C1 | New List configuration input named "List 2" (live "Configuration") | §3.1 | psb |
| C7 | "Change configuration…" panel is a custom OK/Apply panel (verify live's instance configuration UI) | §5.3 | skb / ui2 |
| C6 | Default planes get selected (row highlight + orange plane) after tab switches; repro pending | §5.4 | psb |

## Update from live's answer (behaviours.md "qa5 configurations", 10-21)

| # | Row | Live | Verdict |
|---|---|---|---|
| 3.1 | List input names | first **"Configuration"**, second **"List input 1"** | C1 confirmed |
| 3.5 | Configure mode, click Symmetric | **does not toggle** (stays unchecked, preview unchanged); only configured; **no "Configured" tag on checkbox rows** | C2 confirmed for the toggle; the missing tag is correct |
| 3.4 | Depth = #d display | unfocused shows the value ("10 mm"), focused shows "#d" | ours showed the value → **C3 withdrawn** |
| 5.3 | Change configuration… | feature-style dialog "Change configuration" ✓ ✕: document + branch, Part Studio row with thumbnail + [▦ ▾], one row per input (List ▾, Length field), greyed [Generate], part list | C7 confirmed; instance menu also has "Select instances with same configuration", "Select instances with same part and same configuration" (ours: not checked) |
| 2.4 | "25" + Enter in the variable box | "25 mm" | C8 confirmed |

## Re-run pass 3 (2026-10-21 14:55, /tmp/qa5/noshape-1433 = target-shared 14:32)

| # | Row | Ours | Status |
|---|---|---|---|
| 1.4 / 3.7 | Info bubble "Renamed some new variables…" | **auto-dismisses now (<12 s)**, but still centred over the graphics and **clipped under the open right panel** ("Renamed some" visible); the configure toast is clipped the same way | ✗ C5 (partly fixed) |
| 1.8 | Double-click Extrude 1 with the Variable table open | edit dialog opens | ✅ C4 fixed |
| 2.4 | "25" Enter | "25 mm" | ✅ C8 fixed |
| 3.1 | New List inputs (doc already has "List 2") | next one "Configuration" ✅, the one after **"List 4"** (live "List input 1") | ✗ C1 (partly fixed) |
| 3.5 | Configure mode, click Draft (unconfigured checkbox) | stays unchecked, column "Extrude 1 / Draft" added, toast | ✅ C2 fixed |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| C1 third List name | "List 4" | ✗ still |
| C5 bubble | clipped under the right panel | ✗ still |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
Pass 6 (17:11) had found C5 fixed (bubble at the window's top centre, unclipped, auto-dismisses); not re-checked.
| Row | Ours | Status |
|---|---|---|
| C1 List input names | "Configuration", then "List input 1" | ✅ **fixed** |
