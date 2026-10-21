# Workflow: workspace units (mm ↔ inch) and readouts

qa5, 2026-10-21 pass 4, /tmp/qa5/noshape-1448, "qa5 bracket" (67191.3275 mm³). Live: behaviours.md "Millimetre-document defaults" (~line 211–216), sketch dimension label/box formats (~line 4758).
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | ☰ menu | Rename document, Move to…, Document details…, Restore deleted workspaces…, Copy workspace…, Update workspace…, Workspace units…, Workspace properties…, Switch to view only mode, Print…, Close document | same | ✅ |
| 2 | Workspace units dialog | feature-style panel ✓ ✕; ⌄ Length (Length + Linear acceleration), ⌄ Angle (Angle + Angular velocity), ⌄ Mechanical (Mass, Density, Force, Frequency, Moment, Pressure, Energy), each with "Display decimals" 0.123 | same layout and defaults (metric doc: Millimeter per second squared, Kilogram, Kilogram per cubic meter, Newton, Newton-meter, Pascal, Joule) | ✅ (metric defaults of a metric-created doc not captured live) |
| 3 | Length unit list | Centimeter, Foot, Inch, Meter, Millimeter, Yard | same | ✅ |
| 4 | Inch, ✓; Part 1 volume readout | (3 decimals) | **4.100 in³** (= 67191.3275 mm³) | ✅ |
| 5 | Edit Extrude 1 (depth entered as "8 mm") | ? the edit box shows the value as typed ("8 mm") | **"0.315 in"** (converted) | ? (requested "qa5 units") |
| 6 | Back to Millimeter | – | 67191.328 mm³ | ✅ |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Update from live's answer ("qa5 units", behaviours.md ~6753) + re-run pass 8 (2026-10-21 ~20:00, /tmp/qa5/noshape-1924 = target-shared 19:24, copy "qa5-units" of qa5-bracket-ui)
| # | Row | Live | Ours | Status |
|---|---|---|---|---|
| 5 | Extrude 1 Depth (typed "8 mm"), Inch | unfocused converted ("0.394 in" for 10 mm), **focused the expression as typed ("10 mm")** | unfocused "0.315 in", focused "8 mm" | ✅ |
| 7 | Sketch dimension labels | converted, 3 decimals, no unit suffix ("1.181", "R0.236") | "3.15" (80 mm), "1.969" (50 mm), no suffix | ✅ (trailing zero trimmed, as mm labels) |
| 8 | Mass properties | Volume "4.471 in³", Surface area "32.5 in²" (their part) | Volume 3.935 in³, Surface area 30.121 in²; inertia header "(in² kg)" | ✅ |
| 9 | Panel layout | Mass: Pound etc. in live's (imperial-account) mm doc | ours Kilogram, Kilogram per cubic meter, Newton… | ? (live's defaults may come from the account, not the doc) |
| 10 | Mass properties panel body | opaque | "Front" plane label shows through the lower right of the panel | ✗ CA3 (same translucency as Change configuration / Insert) |
| 11 | Click on empty graphics with Mass properties open | ? (live request "qa5 pass 7" (7)) | the "Part 1" chip is removed from Parts to measure | ? |
| – | Variables converted (#height = 0.197 in) | – | not checked (no variables in this doc) | – |
