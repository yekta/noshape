# QA round 5 (qa4, lead 10-24): realistic sessions users hit most, on v0.4.7, side by side with live

Build: tag v0.4.7 (30979bc1), /root/qa4-scratch/bin/noshape-v047b, fresh profile (HOME wiped) per session. Live side asked
in REQUESTS.md ("qa4 (10-24 12:20, QA round 5 …)"; answers in behaviours "qa4 round 5 s1…s5"). Screenshots
/root/qa4-scratch/r5/. Same method as round 4: every difference logged; local ones fixed by qa4 with a test; the rest routed
to qa5-routing.md.

Sessions (lead 10-24 evening, /root/wt/PRIORITIES.md: everyday use first; languages are P3, the dark + German session is
dropped and its findings are backlog):
1. sketch-heavy part (constraints, dimensions, trim/offset) → extrude/revolve → fillet/chamfer/hole → patterns, timed per step;
2. editing early features in a 30-feature part: regen and responsiveness;
3. heavy patterns + delete mid-regen (done: = v0.4.8 gate (a), section below);
4. a 3-part assembly with the common mates;
5. a drawing with views and dimensions of that part.
Earlier in this round (before the change): undo/redo across tabs, configured part + drawing, top-down assembly, dark + German;
kept below as found.

## v2 Session 1: sketch-heavy part, timed per step (v0.4.8; r5/v2s1.py by CLI, then the app; live asked: "qa4 round 5 v2 s1")
Sketch 1: L outline (6 lines), Ø10 circle, slot on a centre line, dims 80 / 50 / Ø10, divider line + Trim 2 stubs, Offset the
outer loop 3 mm in (12 regions, under-defined dof 14) → Extrude 15 (all regions) → Sketch 2 on Front + Revolve 360 → Fillet R2 ×4
→ Chamfer 1 ×2 → Hole Ø6 through ×2 → Linear pattern of the hole ×4 (Reapply) → Circular pattern of the revolve ×6.
- CLI per step (wall, incl. process start): sketch ops 23–120 ms each; features 300–1100 ms each, growing with the count (each
  call reloads and regenerates); 23 steps 5.9–8.7 s. App regen panel: **total 763 ms** (Extrude 1 347, Linear pattern 152, Hole
  129, Fillet 62, Chamfer 34, Revolve 25, Sketch 1 11). Edit Extrude 1 15 → 20: preview ~0.7 s after Enter; ✓ → result between
  0.36 and 0.71 s (parts list back to 7). Open → idle 2.7 s. r5/v2s1-regen-times.png.
- **R5v2-S1-a → FUSE-UNIFY (kern; live: Onshape merges, 8 faces vs our 12 on live's 3-region case)** Extrude 1 of all 12 regions keeps the region edges as faces on its top (41 faces); a
  later Boolean (the hole pattern) merges them (18 faces). One of the two is wrong; live asked whether Onshape's multi-region
  extrude merges them. r5/v2s1-failed-hole-pattern-before-after.png.
- **R5v2-S1-b → HOLE-MSG + PAT-REAPPLY-FAIL (psb; live reports OK and cuts only the succeeding instances, error text "Hole creates invalid geometry.")** a Hole that fails (tangent to the outer edge: "Hole cut failed: Boolean operation would result in
  non-manifold body.") followed by a Linear pattern of it (Reapply on): the pattern reports **Ok** and cuts the shifted copies.
- CLI notes (not defects): slot = click a centre line, then a width point; a Hole on a Top-plane sketch needs `oppositeDirection`
  and, with several parts, `scope` ("No target parts selected for hole.").
- The hole's row reads "Ø 6 mm THRU" (= live's callout naming) but the Regeneration times panel lists it as "Hole 1".

## v3 Session 1 in the app UI (v0.4.8 c7d9d454, fresh profile, doc "r5 s1 ui"; live "qa4 round 5 s1 offset", "qa4 round 5 s1")
Sketch 1 on Top by hand: L outline (Line tool), Ø10 circle, centre line + Slot (width 20 → dimension 10), Offset of the L loop 3 mm
in, Trim, dimensions 80 / 20 / 30 / 50 → Extrude 15 (region inside the offset loop) → Fillet 5 → Chamfer 1 (face) → Hole (tried)
→ Linear pattern of the part. Timings = spinner gone after the action (software rendering): Extrude preview 0.47 s, ✓ 0.33 s;
Fillet preview 0.32, ✓ 0.33; Chamfer preview 0.28–0.46, ✓ 0.32; Hole preview 0.27; Linear pattern preview 0.38, ✓ 0.43 (live:
dialog closes 116–138 ms after ✓, last redraw ~1.8–1.9 s). No P0.
Same as live ✅: one click offsets one line (live (1)); Trim on a slot line deletes it; dimension value box (drag dots, ±, ✓/✕);
Extrude default 25 mm, region pick, slot + circle cut out of the inner region, slot breaks through at x=77 (live's geometry
finding); Hole dialog rows/defaults (Inch tab, 1/4 (0.25), 6.35 / 12 mm, 118 deg); under-defined "−" badge on Sketch 1.
- **R5v3-S1-a → OFF-CONSTR (skb, P1 wrong geometry)** the sketch Offset ties each copy to its source with **Parallel** (+
  Tangent at joins) only (`sketch/src/tools.rs` offset_chain ~1284); live creates an **OFFSET** constraint + a **DISTANCE**
  dimension (shown as a small "3" between the curves). Effect: dimensioning the source afterwards leaves the copy behind, so my
  3 mm loop became 2.61 / 2.82 / 2.80 / 3.0 mm from edge to edge (CLI sketch_get) and the part volume came out 17259 mm³ instead of
  the 3 mm loop's; the offset can't be edited later and the sketch can't become fully defined without 6 extra dimensions.
- **R5v3-S1-b → OFF-BOX (skb)** live: Enter creates the offset at the current value **and opens the value box** (100×26, "5 mm"
  selected, ± Tolerance options, ✓/✕); typing 3 + Enter applies 3; a small "5" label sits on the arrow stem from the first click.
  Ours: Enter commits with no box; the box opens only by clicking the label, which is drawn between the curves and only once
  the gap is ≥ 30 px (5 mm at default zoom = 16–19 px → no label, I had to zoom in), and it has no ±/✓/✕.
- **R5v3-S1-c → OFF-SIDE (skb)** first side: live puts a line's copy **away from the region** (top edge → above) and a circle's
  **outward**; ours offsets a line to its left normal (the L's top edge went **into** the L on first use) and closed curves
  **inward**. Ours also keeps the flipped side for the next Offset (live asked).
- **R5v3-S1-d → OFF-TOAST (skb)** a second, unconnected entity while a preview is pending: live only selects it and shows the
  toast "Can only offset a single chain at once." (preview unchanged); ours adds it and offsets both chains.
- **R5v3-S1-e (skb, minor)** with constraints shown, a click on an edge under its glyph cluster counts as empty space and commits
  the pending offset (I got a partial chain); hiding constraints avoided it. Live behaviour asked (follow-up 2 (3)).
- **R5v3-S1-f (skb, live asked)** the closing Line segment onto the origin, 0.036 mm off vertical, got Coincident but no Vertical
  (the left edge stayed free until dimensioned).
- **R5v3-S1-g (psb, live asked)** Hole "Sketch points to place holes" takes a click on a planar face ("Face of Extrude 1", hole
  red): `part_studio/mod.rs` kind_matches lets PickKind::Face match PLANE_LIKE, which includes MATE_CONNECTOR. Live's filter is
  sketch vertices or mate connectors.
- Note: once, the first click into the graphics after opening Linear pattern selected nothing; the retry worked (not reproduced).

- **Live follow-ups (behaviours "qa4 round 5 s1 follow-up 2")**: R5v3-S1-f is **not a bug** (live also adds only Coincident
  when a line closes onto the origin a hair off vertical); R5v3-S1-g **confirmed** (live's Hole locations take neither a face nor
  a body vertex: no hover highlight, nothing added; ours takes both, since PLANE_LIKE / POINT_LIKE include MATE_CONNECTOR);
  connected picks join one chain ✅ (live too); **new R5v3-S1-h → OFF-CORNER (skb)**: live joins the chain's offset lines with
  **sharp corners** (lines extended to meet, e.g. (35,55); inward too), ours inserts **arcs** at the corners where the copies
  separate (outward convex corners, and the inward copy of the L's reflex corner); OFF-SIDE confirmed: live does **not**
  remember the flip between offsets.

## v3 Session 2 in the app: live's own 30-feature part (v0.4.8 c7d9d454; r5/v3s2.py builds it by CLI; live "qa4 round 5 v2 s1 / s2")
Same features as live's "v2 s2 30 features": plate 100×60×10, Plane 1 (Top + 10), 6 × (boss sketch + Ø8×5 Boss Add), Hole points
+ Hole (2 × Ø6 through), Fillet 1–4 R3 corners, Chamfer 1–3 0.5 on boss tops, Linear pattern 1 (Boss 6 × 2, Reapply), Fillet 5–7
R1 on boss tops, Chamfer 4 1 mm (top loop, tangent-propagated), Chamfer 5 (bottom edge): all OK, 60779.484 mm³, 48 faces.
"Features (34)" ✅ (live counts the 4 default items too). Timings (spinner gone; live = last redraw on its software-GL box):
| step | ours | live |
|---|---|---|
| ⏱ total at load | **585 ms** (Boss 1 152, Hole 53, Extrude 1 44, Linear pattern 42, Fillet 1 39) | **171 ms** (Hole 18, Boss 5 9) |
| edit Extrude 1 10 → 14: Tab → preview | 0.28–0.50 s | first redraw 327 ms, last 2.9 s |
| ✓ | 0.35 s; ⏱ total after 472 ms (Fillet 5–7 66–82 ms) | dialog closes 201 ms, last redraw 4.28 s; ⏱ 132 ms |
| Roll to here (Extrude 1) | 0.39 s | 1.73 s |
| rollback bar to the top (Parts (0)) | 0.35 s | 2.24 s |
| roll forward (Roll to here Chamfer 4, then drag) | 0.38 s / 0.40 s | 3.60 s / 4.38 s |
Same as live ✅: editing rolls back to the feature with later rows grey italic and the "Final" toggle; Roll to here greys
the rows below a 7 px bar; dragging shows the "Rollback bar" pill; Parts (0) with the bar on top; the Extrude 1 menu order.
- **R5v3-S2-a → REGEN-SPEED (perf/kern)** now on the identical model: 585 ms vs live 171 ms at load (3.4×), 472 vs 132 after the
  edit; Boss 1 alone 152 ms on load (the other bosses 11–18), Hole 53 vs 18.
- **R5v3-S2-b → EDIT-TYPING-PREVIEW (psb/ui2)** typing in a dialog number field re-runs the preview on every keystroke (typing
  "14" over 12 redrew the plate at 14 before Tab; `feature_dialog.rs` on_text_changed emits DialogEvent::Changed per
  InputEvent::Change); live: "Typing does not update the preview … re-runs when the field commits (Tab or Enter)".
- **R5v3-S2-c → ROLL-LAST (psb)** with the bar above the last feature, its right-click menu has no "Roll to here"
  (`part_studio/mod.rs` ~3927 hides it whenever `is_last`); live used exactly that (Roll to here on Chamfer 5) to roll back to
  the end. Our menu of a rolled-back feature also lacks "Add appearance to feature…" (live asked which items it has).
- **R5v3-S2-d (psb, minor, live asked)** releasing the dragged bar over the Default geometry rows leaves it where it was;
  releasing just above Sketch 1 works (live: "To the top, under Default geometry").
- Note (cosmetic, synthetic input): after clicking a context-menu item, the row under the pointer kept its hover tint after the
  pointer left the list, until the list was hovered again.

- **Live follow-up (behaviours "qa4 round 5 s2 rollback follow-up")**: rolled-back features' menu is Rename | Add selection to
  folder… | Suppress | Dynamic suppression ▸ | (Add comment) | **Roll to here | Roll to end** | Delete (the last feature too);
  live drops Edit…, Zoom to selection, Show dependencies…, Add appearance… for them. The feature just above the bar has **Roll
  to end** (and "Section view…") but no Roll to here; with the bar at the end the last feature has neither. Ours: rolled-back
  rows keep Edit… / Zoom to selection / Show dependencies…, no "Roll to end" anywhere, and the last row lacks Roll to here →
  ROLL-LAST widened. R5v3-S2-d **confirmed**: live's drop indicator clamps to the first legal slot (below "Right"), so a drop
  over Origin or the Default geometry header lands at the top → ROLLBACK-DROP-TOP (psb).

## v3 Session 3 in the app: 3-part assembly by hand (v0.4.8 c7d9d454; parts by r5/v3s3.py, assembly built in the UI)
Plate 100×60×10, pin Ø10×40, block 20³ (round 4 S2's parts). Insert dialog (Current document › Part Studios, click part →
click to place, "Inserted: 3") ✅; DOF glyph + tooltip "Has degrees of freedom (6)" / "(1)" ✅; row menu → Fix (ground glyph on
the row and on Assembly 1) ✅; Revolute / Fastened via tool search: mate connector inference points on hovered faces (centroid,
edge midpoints, vertices) and the in-canvas popup (type ▾, flip, reorient, Solve, ✓ ✕) ✅; new mates are listed grey with
"(Hidden)" in the tooltip (= live's mate tooltips, behaviours ~1946).
- **R5v3-S3-a → MATE-FLIP-AXIAL (asb, P2 wrong placement)** "Flip primary axis" has no effect on Revolute, Cylindrical, Planar
  (and by the same code Pin slot, Parallel): their residual only uses `[z.x, z.y]` (`assembly/solver.rs` ~571–579), which an
  anti-parallel Z also satisfies, so the solver keeps whichever orientation is nearer the instance's pose. In the app the pin's
  top face mated to the plate's top face hung through the plate, and toggling the flip (popup or dialog, before and after ✓) left
  it there. CLI (r5/v3s3-flip.py): pose identical with flip_primary false/true for revolute, cylindrical, planar; fastened
  honours it (false → pin upside down on top, z 10..50; true → hanging, z −30..10). Fix idea: snap the moving instance to the
  flipped frame when the flip changes / the mate is made, and give the Z residual a sign (e.g. add 1 − z.z or use log_rot of the
  Z-to-Z rotation).

## v3 Session 4 in the app: drawing with views and dimensions (v0.4.8 c7d9d454; the v3 s2 30-feature part)
Part 1 › Create Drawing of Part 1… → Create Drawing dialog (templates noshape/ANSI/ISO/JIS, ANSI_A_INCH preselected, Options
Suggested views | No views) → ANSI_A_MM, No views → Insert view (orientation Top, 1:1) → click places the base view, then the
projected Front (below) and Right (right, bosses pointing left = third angle) follow the cursor; Esc ends ✅. Centre marks on
every circle, tangent edges of the R3 corners drawn ✅. Dimensions: plate top edge → **94** (the straight edge between the R3
corners, extension lines at the tangent points) ✅; Diameter (Shift+D) on a hole → **Ø6** with leader ✅; point-to-point between
two boss centre marks → **25** ✅.
- **R5v3-S4-a → DRW-DIM-2EDGES (asb, P2)** the smart Dimension (D): first click on an edge, second click on a **parallel edge of
  the same view** → instead of taking it as the second entity it places the first edge's length dimension at that click. 3/3
  tries (Front view top + bottom edge → "94" laid on the bottom edge; Top view left + right edge → "54" laid on the right
  edge), clicks on the exact edge pixel rows. `scene/drawing_tools/smart.rs` dimension_takes_more → smart_takes(c, first) must
  be returning false there (smart_pick(c) none / not the edge). Point-to-point picks work. Live's smart dimension takes the
  second edge and dimensions between the two.

## v2 Session 2: editing an early feature in a 30-feature part (v0.4.8; r5/v2s2.py; live asked: "qa4 round 5 v2 s2")
Plate 200×100×10 + 14 × (sketch circle + boss Add): 30 features, 1 part (211583.052 mm³, 34 faces). CLI build 23.8 s.
- App: regen panel **total 462 ms** (bosses 17–71 ms each). Edit Extrude 1 (feature 2) 10 → 12: preview settles 0.8–1.3 s after
  Enter, ✓ → result 1.1–1.5 s (burst screenshots, software rendering). Roll to here (Extrude 1): < 0.67 s; rollback bar drag
  down 4 steps: 0.77 s. The bar has no right-click menu ✅ (live); dragging shows the "Rollback bar" pill ✅. Holding the drag at
  the panel's bottom doesn't auto-scroll the list; live autoscrolls → ROLLBACK-AUTOSCROLL (psb). Live (server ⏱): 171 ms total for
  its 30 features, bosses ≤ 9 ms each (ours 17–71) → REGEN-SPEED (perf/kern). r5/v2s2-regen-times.png, v2s2-rollback-drag.png.

## v2 Session 4: 3-part assembly with the common mates (v0.4.8; round 4's r4/s2.py rebuilt; compared with live in round 4)
Plate + block + pin, Fastened / Revolute / Slider, BOM Flattened 3 rows (= round 4 S2, all matching live then). In the app the
assembly opens with the mates solved; dragging the block slides it along the Slider axis and the pin follows ✅ (a press that
arrives with the motion in the same instant box-selects instead: synthetic-input timing, not a product issue).
r5/v2s4-slider-drag.png. The mate list shows REST/CLI-made mates grey (hidden): the live question on hidden vs shown mates is
still open (round 4).

## v2 Session 5: drawing with views and dimensions (v0.4.8)
Covered by the configured-part drawing above (Create Drawing › Suggested views, Dimension / Diameter tools, Update after model
changes, dangling dimension) and v0.4.8 gate (f) (dual dimensions, PNG export, note leaders): findings DRW-CFG, DRW-DANGLE-DOT,
DRW-DIMMENU, DRW-CMARK, DRW-DUAL (asb; P2 drawings).

## Session 4: undo/redo across tabs (done first: no build needed)
Document "qa4 r5": Part Studio 1 Sketch 1 (rectangle) + Extrude 1 → Assembly 1 › Insert Part 1 → Part Studio 1 Ctrl+Z →
Assembly 1 Ctrl+Z / Ctrl+Y → Part Studio 1 Ctrl+Y → rename the Part Studio tab, Ctrl+Z → delete Assembly 1, Ctrl+Z.
- **Undo is per tab** in ours: Ctrl+Z in Part Studio 1 undid Extrude 1 (not the later assembly insert); Assembly 1's undo
  button was greyed until its own insert, Ctrl+Z there removed the instance, Ctrl+Y restored it; Part Studio Ctrl+Y redid
  the extrude and the instance resolved again. Tooltips "Undo (cmd+z/ctrl+z)". ✅ = live (behaviours "qa4 round 5 s4": per element, tooltips never name the operation).
- After the Part Studio undo the instance turns red with an error dot (part missing) ✅ (expected).
- **R5-S4-a** the bottom-right readout kept **"Area: 1535.933 mm²"** for the picked face after its part was undone, and even
  with **Instances (0)** after the assembly undo (r5/s4-stale-area-readout.png, s4-asm-undo-insert.png). The Part Studio
  already refreshes (qa5 RD1); the assembly cached the measure per selection → **fixed by qa4** (`assembly_view.rs`
  `apply_result`: picks that no longer resolve drop out and the readout is recomputed after every evaluation;
  `assembly/analysis.rs` `pick_exists`; test in `measure_between_instances`).
- Tab rename: Ctrl+Z in the studio undid the studio's last feature, the tab name stayed ✅ = live.
- Deleting a tab: immediate, no confirmation, Ctrl+Z doesn't restore it ✅ (live behaviours ~1565).
- Selecting the Right plane shows "Area: 4000000000000.000 mm²" on v0.4.7 = qa5's MEAS1, fixed on main 78109ebc.

## Session 1: configured part S/M/L + drawing, revised (r5/s1.py builds the part by CLI; drawing in the app; live "qa4 round 5 s1")
Part: Size S/M/L → #len 60/80/100; rectangle #len × 40, Extrude 20; Ø10 at (#len/2, 20) Through all. Geometry follows the
configuration in the app ✅ (the CLI's volumes don't: CFG-MASS, routed in round 4). Drawing: Part 1 › Create Drawing of Part 1…,
ANSI_A_MM, Suggested views; Dimension 60 (S, see a) and Ø10; Extrude 1 20 → 25; delete Extrude 2.
- **R5-S1-a** (→ DRW-CFG, asb) the drawing ignores the Part Studio's configuration: created with Size **M** active, the views are **60** long
  (S, the default); live: "The drawing takes the Part Studio's current configuration" (80). v0.4.7 and v0.4.8 alike.
- **R5-S1-b** (→ DRW-CFG) changing a drawing view's configuration: ours **view menu › Configuration ▸ Size ▸ S/M/L** (between Show/hide and
  Tangent edges); live: **"Change configuration..."** (between Align views and Switch to Part Studio 1) opening a panel
  (document / branch, tree "Part Studio 1" + Size [M ▾], part thumbnail, ✓ ✕) that changes all views of that part; View
  properties shows it read-only ("Configurations: Size: M"). r5/s1-view-menu-configuration-submenu.png.
- (b) ✅ no automatic update; the toolbar Update button turns yellow (ours (250,182,1), live ≈ (255,197,26)), tooltip
  "Update from this workspace (ctrl+q)"; after Update the views grow, the dimensions follow. r5/s1-drawing-out-of-date.png,
  s1-drawing-updated-depth25.png.
- **R5-S1-c** dangling dimension (Extrude 2 deleted, Update): ours **red #DC2828** text/leader with a **black arrowhead** and a
  status line "1 dangling annotation(s)"; live **orange-red #DD5228** text and leader with a **filled orange-red dot** at the
  attachment point, no tooltip (no status text noted). Colour **fixed by qa4** (`drawing_scene.rs` `DANGLING_RGB`, text and
  lines; test `dangling_annotations_are_live_orange_red`); the dot → DRW-DANGLE-DOT (asb). r5/s1-dangling-dim.png.
- **R5-S1-d** (→ DRW-DIMMENU, asb) diameter dimension right-click: ours Foreshorten · Show as radius · Edit… · Detach from geometry · Paste as table |
  Clear selection · Zoom to fit · Delete; live (dangling and healthy alike) **Edit… · Paste · Paste as table · Unbreak dimension
  · Add comment · Clear selection · Zoom to fit · Delete** (behaviours 1536 / 13189). v0.4.8 the same (drawing_tools.rs 1689,
  drawing_more.rs Foreshorten). r5/s1-dangling-menu.png.
- **R5-S1-e** (→ DRW-CMARK, asb) Suggested views: our Top view has **no automatic centre mark** on the hole (ANSI_A_MM, v0.4.7 and v0.4.8); live's
  Top view has one (ISO_A3 here; behaviours ~1537 ANSI too). A CLI-inserted view does get one.
- Dimension (d) on the hole rim: one click → Ø preview → place ✅ (= live); a click on the circle's quadrant point picks the point
  (my first two tries).

## Session 3: heavy patterns + delete during regen (= v0.4.8 gate item (a), the user's fins; live "qa4 round 5 s3")
Our side ran as gate (a) on v0.4.8 (core-workflow.md "v0.4.8 gate"): fins ×400 / ×400, results = live; regen 10.6 s total.
- **R5-S3-a** (= gate (a)) a Delete during regeneration takes 7–9 s to settle; live: "deletes during regeneration are never
  refused. They apply at once (sub-second) and the regeneration restarts." Routed with the gate (perf/psb, lead).
- **R5-S3-b** (→ PS-NOTCOMPUTED, psb) the row menu of a feature that is still regenerating: ours the full menu (Rename, Edit…, Add selection to
  folder…, Suppress, Dynamic suppression ▸, Zoom to selection, Show dependencies…, Delete); live a reduced menu "Rename | Add
  selection to folder… | Suppress | Dynamic suppression ▸ | Add comment | Delete" (no Edit, no Roll to here) while the row is
  not computed; and with a feature dialog open the rolled-back rows have no Delete ("Rename | Suppress | Dynamic suppression ▸ |
  Add comment"). Live also: Ctrl+click on a not-computed row doesn't add it to the selection, Shift+click range does.
- Live's silent pitfall (2nd direction off the plate → OK with one row): not re-checked on ours yet.

## (dropped, P3) Dark mode + German (v0.4.8, Preferences UI; live "qa4 round 5 s5"): backlog
Dark: Part Studio, assembly, BOM = live (gate (e)). German (dialog strings vs live):
- **R5-S5-a** (= gate (e)) toolbar tooltip titles ("Extrude (shift+e)"), Extrude dialog (Thin, Intersect, Direction, Starting
  offset, Second end position, "Entwurf"…).
- **R5-S5-b** (→ I18N-DIALOGS, ui2) Fillet: "Verrunden 1" (live Verrundung 1), "Full round" (Volle Rundung), "Entities auf fillet" (Zu verrundende
  Elemente), "Tangent propagation" (Tangentenfortsetzung), "Measurement" (Messung), "Control" (live "Strg"), "Asymmetric",
  "Partial fillet", "Variable fillet", "Allow edge overflow", "Smooth fillet corners" (live Asymmetrisch, Partielle Verrundung,
  Variable Verrundung, Kantenüberlauf erlauben, Glatte Verrundungsecken). r5/s5-de-dark-fillet.png.
- **R5-S5-c** (→ I18N-DIALOGS) Mass properties: "Parts auf measure" (Zu messende Bauteile), "Mate connector for reference frame"
  (Verknüpfungsverbindung für Referenzrahmen), „calculation variance“ anzeigen (Berechnungsabweichung anzeigen); the rest
  matches. r5/s5-de-dark-massprops.png.
- **R5-S5-d** (→ I18N-DIALOGS / DARK-DRW) part menu: "Drawing of Part 1… erstellen" (live „Zeichnung erstellen von „Part 1" …"); the rest matches (no
  Kommentieren: comments are cloud). Create Drawing: title "Drawing: erstellen" (live "Zeichnung erstellen:"), "Owned by me"
  (Gehört mir); in dark mode the selected "No views" tile's label is unreadable (light on light blue).
  r5/s5-de-dark-part-menu.png, s5-de-dark-create-drawing.png.
- **R5-S5-e** (→ DARK-DRW, ui2) drawing in dark mode: sheet and canvas stay light ✅; our drawing toolbar row is dark with a **white block at
  its right end**; live's capture (drawing/qa4-r5-s5-12) shows the drawing toolbar light (white, dark icons) while top bar,
  left strip and tabs are dark (live's text lists "toolbar background" as dark: conflicts with its own screenshot).
  r5/s5-de-dark-drawing-toolbar.png.
- Preferences (gate (e) off-gate): units sentence English, heading "Voreinstellungen" (live "Preferences"), "Dezimalstellen"
  (live "Dezimalstellen anzeigen"). Live's own untranslated list (Light-Modus, Blind, lb mm², Lxx, unit names, title block)
  = ours.

## Session 2: top-down assembly, Edit in context (v0.4.8 app; Base by CLI; live "qa4 round 5 s2") — partial
Base 100×60×20 → Assembly 1: Insert, Fix ✅ (instance menu has Fix; root/instance fix icons) → toolbar "Create Part Studio in
context" (last button, tooltip = live) → panel "Origin of new Part Studio" → ✓ → new Part Studio tab, context bar "Assembly 1
displayed | Insert and go to assembly ▾ | ✕", ☐ Show internal geometry, ☑ Select transparent geometry, slider ✅ → Sketch on the
box top face: "Assembly contexts" section with [Context 1 Assembly 1 ▾] [•••], orange bar "Context 1 of Assembly 1 | … |
Done", plane chip "Face of Context 1" ✅; Sketch/Extrude rows get the orange in-context arrow ✅.
- **R5-S2-a** the "Origin of new Part Studio" field opens **prefilled with "Origin"** from the toolbar; live's opens empty from
  the toolbar ("With nothing picked, ✓ just closed the panel") and prefilled only from the Origin row's menu (assembly/542) →
  **fixed by qa4** (`assembly_behaviors.rs` `PsOrigin`; toolbar / tool search open it empty, ✓ with nothing picked closes;
  test `ps_origin_tests`). r5/s2-origin-panel.png.
- **R5-S2-b** the context bar's button reads **"Hide/show instances"** as soon as the context exists; live keeps **"Insert and go
  to assembly ▾"** until the new part is in the assembly (then "Hide/show instances") → **fixed by qa4** (`part_studio/in_context.rs`: the amber
  bar keeps the Insert split while no instance comes from this studio; test `insert_pending_tests`). r5/s2-context-sketch.png.
- **R5-S2-c** (→ CTX-USE, skb) **Use (u) on the context's top face projects nothing** (no hover highlight on the face), and Use on its four top
  edges (in the sketch plane) gives "**That geometry can't be used in this sketch**"; an edge of the context that is not in the
  plane (bottom edge, picked through the transparency) is projected. Live: Use on that face projected its 4 outer edges (offset 0)
  → the Lid. r5/s2-use-refused.png. (Not yet checked: Use of in-plane edges on a normal, non-context face.)
- Not run yet (the session got tangled after R5-S2-c): Insert and go to assembly (live: "Select items to insert", Lid as Part 1
  <2>, placed where modelled, not fixed, no mates), Base → 120 (live: blue out-of-date dot "Update available for a context.",
  ••• › Update context → Lid 120 × 60 × 5, 36000 mm³). To redo.
- Also: green toast 'New assembly context "Context 1" created.' (live) not seen in my screenshot (taken 3 s after); to recheck.
