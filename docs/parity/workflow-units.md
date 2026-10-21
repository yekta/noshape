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
