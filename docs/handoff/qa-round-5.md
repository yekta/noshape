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
- **R5v2-S1-a (asked live, possible P0)** Extrude 1 of all 12 regions keeps the region edges as faces on its top (41 faces); a
  later Boolean (the hole pattern) merges them (18 faces). One of the two is wrong; live asked whether Onshape's multi-region
  extrude merges them. r5/v2s1-failed-hole-pattern-before-after.png.
- **R5v2-S1-b (asked live)** a Hole that fails (tangent to the outer edge: "Hole cut failed: Boolean operation would result in
  non-manifold body.") followed by a Linear pattern of it (Reapply on): the pattern reports **Ok** and cuts the shifted copies.
- CLI notes (not defects): slot = click a centre line, then a width point; a Hole on a Top-plane sketch needs `oppositeDirection`
  and, with several parts, `scope` ("No target parts selected for hole.").
- The hole's row reads "Ø 6 mm THRU" (= live's callout naming) but the Regeneration times panel lists it as "Hole 1".

## v2 Session 2: editing an early feature in a 30-feature part (v0.4.8; r5/v2s2.py; live asked: "qa4 round 5 v2 s2")
Plate 200×100×10 + 14 × (sketch circle + boss Add): 30 features, 1 part (211583.052 mm³, 34 faces). CLI build 23.8 s.
- App: regen panel **total 462 ms** (bosses 17–71 ms each). Edit Extrude 1 (feature 2) 10 → 12: preview settles 0.8–1.3 s after
  Enter, ✓ → result 1.1–1.5 s (burst screenshots, software rendering). Roll to here (Extrude 1): < 0.67 s; rollback bar drag
  down 4 steps: 0.77 s. The bar has no right-click menu ✅ (live); dragging shows the "Rollback bar" pill ✅. Holding the drag at
  the panel's bottom doesn't auto-scroll the list (live: not recorded). r5/v2s2-regen-times.png, v2s2-rollback-drag.png.

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
