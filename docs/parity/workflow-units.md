# Workflow: workspace units (mm ↔ inch) and readouts

qa5, 2026-10-21 pass 4, /tmp/qa5/noshape-1448, "qa5 bracket" (67191.3275 mm³). Live: behaviours.md "Millimetre-document defaults" (~line 211–216), sketch dimension label/box formats (~line 4758).
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | ☰ menu | Rename document, Move to…, Document details…, Restore deleted workspaces…, Copy workspace…, Update workspace…, Workspace units…, Workspace properties…, Switch to view only mode, Print…, Close document | same | ✅ | 0.4.10+ / dd1b5037 10-27 (wf-block) |
| 2 | Workspace units dialog | feature-style panel ✓ ✕; ⌄ Length (Length + Linear acceleration), ⌄ Angle (Angle + Angular velocity), ⌄ Mechanical (Mass, Density, Force, Frequency, Moment, Pressure, Energy), each with "Display decimals" 0.123 | same layout and defaults (metric doc: Millimeter per second squared, Kilogram, Kilogram per cubic meter, Newton, Newton-meter, Pascal, Joule) | ✅ (metric defaults of a metric-created doc not captured live) | 0.4.10+ / dd1b5037 10-27 (wf-block) |
| 3 | Length unit list | Centimeter, Foot, Inch, Meter, Millimeter, Yard | same | ✅ | 0.4.10+ / dd1b5037 10-27 (wf-block) |
| 4 | Inch, ✓; Part 1 volume readout | (3 decimals) | **4.100 in³** (= 67191.3275 mm³) | ✅ | 0.4.10+ / dd1b5037 10-27 (wf-block): 0.406 in³ / 5.027 in² (= 6649.885 mm³ / 3243.532 mm²) |
| 5 | Edit Extrude 1 (depth entered as "8 mm") | ? the edit box shows the value as typed ("8 mm") | **"0.315 in"** (converted) | ? (requested "qa5 units") → now ✅ = live: unfocused converted, focused as typed (row 5 of pass 7 below, dd1b5037) |  |
| 6 | Back to Millimeter | – | 67191.328 mm³ | ✅ | 0.4.10+ / dd1b5037 10-27 (wf-block): 6649.885 mm³ |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Update from live's answer ("qa5 units", behaviours.md ~6753) + re-run pass 8 (2026-10-21 ~20:00, /tmp/qa5/noshape-1924 = target-shared 19:24, copy "qa5-units" of qa5-bracket-ui)
| # | Row | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 5 | Extrude 1 Depth (typed "8 mm"), Inch | unfocused converted ("0.394 in" for 10 mm), **focused the expression as typed ("10 mm")** | unfocused "0.315 in", focused "8 mm" | ✅ | 0.4.10+ / dd1b5037 10-27 (wf-block): unfocused 0.394 in, focused "10 mm" |
| 7 | Sketch dimension labels | converted, 3 decimals, no unit suffix ("1.181", "R0.236") | "3.15" (80 mm), "1.969" (50 mm), no suffix | ✅ (trailing zero trimmed, as mm labels) | 0.4.10+ / dd1b5037 10-27 (wf-block): 1.575 / 1.181, no suffix; Hole row "Ø 0.25 in ↧ 0.472 in" |
| 8 | Mass properties | Volume "4.471 in³", Surface area "32.5 in²" (their part) | Volume 3.935 in³, Surface area 30.121 in²; inertia header "(in² kg)" | ✅ | 0.4.10+ / dd1b5037 10-27 (wf-block): volume / area converted (row 11) |
| 9 | Panel layout | Mass: Pound etc. in live's (imperial-account) mm doc | ours Kilogram, Kilogram per cubic meter, Newton… | ? (live's defaults may come from the account, not the doc) → now ✅ they do: live's account Preferences › Units are imperial (Mass *Pound, Density *Pound per cubic inch …, behaviours 2665) and a new document takes them; ours the same mechanism (Preferences › Units → new documents, account.md row 4), default metric |  |
| 10 | Mass properties panel body | opaque | "Front" plane label shows through the lower right of the panel | ✗ CA3 (same translucency as Change configuration / Insert) → now ✅ closed (pass 21: live's 95 % white panels show through too) |  |
| 11 | Click on empty graphics with Mass properties open | ? (live request "qa5 pass 7" (7)) | the "Part 1" chip is removed from Parts to measure | ? → now ✅ (pass 8: live keeps the chip, ours too) |  |
| – | Variables converted (#height = 0.197 in) | – | not checked (no variables in this doc) | – |  |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (7): an empty-graphics click keeps dialog chips. Ours on 5bd50b2: Mass properties chip kept after clicks at (1400,850) and (800,102) → row 11 ✅.

Row 10 (Mass properties panel translucency): **? pending** ui's comparison (live dialogs are rgba(255,255,255,.95), see workflow-configured-assembly.md CA3).

## Re-run pass 21 (qa5, 2026-10-23)
| 10 / CA3 | closed in workflow-configured-assembly.md (CA3: live's 95 % white panels show what is behind them too; no change) | ✅ closed |
