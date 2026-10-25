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

## v3 Session 10 on v0.4.8 (release tarball): an everyday assembly (P2; r5/v3s10b.py six mates, r5/v3s10.py / v3s10n.py planar stacks of 19 / N cubes; drag rate with r5/dragfps.sh = ffmpeg x11grab 30 fps without the pointer, distinct frames counted)
Six mates by CLI on a fixed plate: DOF Fastened 0, Revolute 1, Slider 1, Cylindrical 2, Planar 3, Ball 3 ✅ (DOF glyph tooltips
"Has degrees of freedom (n)"); first instance lands at the origin, not fixed (= live); dragging a revolute instance turns it about
its axis ✅. Group (tool search "Group": Instances field) → "Group 1" row; Planar + Ball cubes grouped → 1 DOF (the remaining
rotation about the vertical through the ball point) ✅. BOM panel: Flattened, Item / Quantity / Part number / Description (= live's
default columns), rows 1 × plate, 6 × cube ✅. Assembly Linear pattern: Instances, Direction + MC button, Distance 25 mm, Instance
count + flip, Equal spacing, Centered, Second direction; ✓ → "Linear pattern 1" row with Part 2 <7>, <8> ✅. Exploded views panel
(right strip) → Add exploded view → amber "Editing Explode 1 · Done" bar, the "Build steps by dragging components" box, triad
on a picked instance, dragging its arrow → "Explode step" dialog (Translation, Instances, Distance 54.3 mm, Explode lines) + dashed
explode line + "Explode step 1" row ✅. Mate flips: MATE-FLIP-AXIAL (skb) stands.
Drag rate (software GL on Xvfb, so absolute numbers are low): 7 instances, a Planar cube: **10.0 updates/s**, longest gap 0.17 s;
20 instances (19 cubes Planar-stacked), the top cube: **3.9 updates/s**, longest gap 0.67 s, and the assembly blows up (below).
- **R5v3-S10-a → DRAG-RUNAWAY (skb, P2 severe)** dragging the top cube of a Planar-mated stack moves the **other** free cubes: with
  3 cubes only the dragged one moves ✅; with 10 the whole column shifts and leans; with 19 a 25–150 px drag flings the cubes to
  ±2.5 million mm in X/Y (assembly_list after the drag: Part 2 <1> at (−2491492, −2917166, 10)), every mate still "ok" (Planar
  allows any in-plane position) — the solver satisfies the mates but lets the free in-plane DOF of the undragged bodies wander
  (no pull toward their current pose). Ctrl+Z restores. Repro: r5/v3s10n.py with NCUBES=19, open, drag the top cube.
- **R5v3-S10-b → CHIP-X-SELECTED (skb)** in the assembly Linear pattern dialog, a seed chip's × doesn't remove the instance while it
  is still selected in the instance list (the field re-captures it from the selection); after clearing the selection the × works.
- Note: "Explode" isn't in tool search (exploded views are a right-panel feature; live likely the same, not asked).

## v3 Session 9 on v0.4.8 (release tarball): interaction (r5/v3s9.py: Part 1 = block 60×40×20 with a Ø10 hole and an R5 vertical fillet, 46321.903 mm³; Part 2 = cylinder Ø16×30, 6031.858 mm³; live "Selection colours", "Box select", "Right-click menus", "Measure & mass properties", "Section view", "Isolate / Make transparent")
Nothing felt slow: hover, picks, box selection, Select other, menus and panels were all immediate; section drag updated live.
Same as live ✅: face hover = light-orange boundary only (hole loop too); edge hover = thick light orange; vertex hover = dot;
edge click → amber + "Length: 60.000 mm"; click at the edge midpoint selects the point ("Point: 30.000 mm 0.000 mm 20.000 mm");
window box (light-blue fill, blue border) selects only what's fully inside (whole block); crossing box (light-yellow fill,
dashed amber border) selects what it touches; face and edge right-click menus = live's lists (Add comment = cloud); Measure:
"Min dist: 22.000 mm" readout, panel with chips, Measure type / units / reference CS and rows (Area 3824.060 mm² = top face
2316.1 + cylinder side 1508.0 ✓); Mass and section properties (Part | Face, fields, "One or more parts do not have a material
defined." toast, Volume 52353.761 = both parts); Section view from a face's menu: "Section plane 1 (Face of Extrude 1)", tan
hatched preview cut on both parts, drag → inline "-18.543 mm" box, ✓ → hatched in the part colour, empty-space menu gains
"Edit section view…" (reopens with the plane and offset) and "Turn off section view"; Isolate / Make transparent dialog (✕ only,
Transparency controls + checkbox + slider, Component controls "Expand: distance" + slider, ?), Isolate ghosts the rest and the
planes, Make transparent ghosts the picked part; Hide → eye-slash + grey row, "Show all" in the empty-space menu.
- **R5v3-S9-a → BOX-PREHIGHLIGHT (psb)** while dragging a crossing (right→left) box nothing it touches is pre-highlighted;
  live pre-highlights everything it touches light orange (planes included).
- **R5v3-S9-b → SELECT-OTHER (psb/ui2)** ours: header "Select other" without ✕, a grey type column ("Edge"/"Face") before each
  name, a row preselected blue, hover = grey row background, no footer; live (sketch/231): bold header + red ✕, bordered list
  with plain rows ("Edge of Sketch 2", "Face of Sketch 2", …), the hovered row **bold** (no fill), "?" footer.
- Pending live ("qa4 round 5 s9 interaction"): a global selection filter (ours has none), "Hide other parts" / "Hide all parts" in
  the parts-row menu (ours has both; live's recorded list doesn't), the Measure panel's default unit in a mm document (ours
  Millimeter; live's notes say Inch), axis icons in the Point readout (ours none), the section manipulator (ours: arrow only;
  live notes: arrow + rotate glyph + ring), the edge midpoint marker (ours shows a filled dot anywhere along the hovered edge).

- **Live answers (behaviours "qa4 round 5 s9 interaction")**: no global selection filter in live either (= ours ✅); "Hide other
  parts" / "Hide all parts" are in live's parts-row menu too (= ours ✅); the Measure panel defaults to **Millimeter** in a mm
  document (= ours ✅; the older live note saying Inch was wrong). **READOUT-AXES (ui/psb)**: live's status-bar "Point:" shows a grey
  letter + coloured axis arrow before each value (X red ↘, Y green ↗, Z blue ↑, values dotted-underlined); ours shows bare values.
  **SECTION-MANIP (psb)**: live's section-plane manipulator = ring at the plane point + shaft + hollow arrowhead + two crossing
  rotate handles at the tip (each highlights blue on its own); ours draws the arrow only. **MIDPOINT-MARKER (psb/ui)**: hovering an
  edge anywhere shows a small **dark hollow ring** at its midpoint; near the midpoint it becomes an **orange hollow ring** and the
  edge **loses** its highlight; a selected midpoint is a yellow-orange ring (#FAC012 / #FFD65B centre); ours draws a filled
  orange dot and keeps the edge highlighted.

## v3 Session 8 on the published v0.4.8 (tag 2bf78a67 = gate c7d9d454 + two CI-test commits; release tarball, not a local build): the feature list on the 30-feature part (r5/v3s2.py; live "Feature-list row states", "ui (10-22 d) feature-row tooltips")
Timings (spinner gone): Suppress Extrude 1 0.56 s, Unsuppress 0.50 s, valid drag reorder (Plane 1 above Sketch 1) 0.62 s,
undo / redo / undo of it 0.37 / 0.55 / 0.57 s, Delete Boss sketch 1 0.41 s and its undo 0.63 s, folder Suppress 0.54 s and its
undo 0.45 s, rename undo 0.45 s; edit / Roll to here / bar drags as in v3 s2 (same app code: 0.35–0.40 s).
Same as live ✅: row hover highlights the feature's geometry in the viewport; tooltips follow live's format
`<name>[ [Type]][ (Hidden)][ did not regenerate properly: <msg>]` ("Boss 3 [Extrude] <- Boss sketch 3 [Sketch]", "Boss 1
[Extrude] did not regenerate properly: No merge scope selected."); suppressed row italic + strike-through; error rows red with
the red ⓘ on "Features …"; plain clicks add rows to the selection; Delete with the pointer over the viewport deletes the
selected feature with no confirmation and dependents go red; inline Rename (name selected, Enter commits); "Add selection to
folder…" → "Folder name" box prefilled "New folder" → collapsed "› New folder (2)"; suppressing a folder suppresses its members;
every action undoes and redoes.
- **R5v3-S8-a → ERR-MISSING-REGION (kern/psb, live asked)** after deleting a boss's sketch the boss says "A selected sketch region
  no longer exists" (no final period; `regen/eval.rs` ~636); that isn't an Onshape string: live's empty-input messages are the
  "Select …" ones (e.g. "Select edges or faces to fillet." for a Fillet whose edges went away, behaviours ~7155), here presumably
  "Select face or sketch region to extrude.".
- **R5v3-S8-b (psb, live asked)** an invalid drag reorder (Boss 4 above its own sketch) shows the normal blue drop line while
  dragging, then a red toast "Cannot reorder: Boss 4 depends on Boss sketch 4." and leaves the list unchanged; live's handling
  pending.
- **R5v3-S8-c (psb)** the multi-row menu lacks "Edit sketch appearance…" when a sketch is among the selected rows (live: Add
  selection to folder… | Section view…, Suppress, Dynamic suppression ▸ | Clear selection | Edit sketch appearance…, Add
  appearance to feature… | Delete).
- **R5v3-S8-d (psb, live asked)** expanding a folder (click on its row) is recorded as an undo step on ours (the first Ctrl+Z
  after it only collapsed the folder).
- Pending live: whether hovering geometry highlights its feature row (ours doesn't), whether the dependency tooltip shows on
  an unselected row (ours does), the folder row menu (ours: Expand, Rename | Suppress, Unsuppress | Expand all, Collapse all |
  Unpack folder), and what Suppress of a base feature does to Add bosses (ours: red "No merge scope selected.").
- Note (cosmetic, seen twice now): after a context-menu click the row under the pointer keeps its hover tint.

- **Live answers (behaviours "qa4 round 5 s8 feature list")**:
  - **REORDER-INVALID (psb)**: live **does** an invalid reorder (Boss 2 above its sketch; Sketch 1 below Extrude 1): no
    refusal, no toast, the same drop indicator; the dependent goes red ("Boss 2 [Extrude] did not regenerate properly: Select face
    or sketch region to extrude." / "1 missing selection") with its downstream features. Ours refuses with a red toast "Cannot
    reorder: … depends on …".
  - **ERR-MISSING-REGION confirmed**: live's text after deleting the sketch is "Select face or sketch region to extrude." plus a
    second tooltip line **"1 missing selection"**; Chamfer 1 on Boss 1 goes red too. One Ctrl+Z restores both (= ours).
  - Suppress Extrude 1: bosses "No merge scope selected." (= ours ✅), fillets "Select edges or faces to fillet." / "1 missing
    selection", Parts (0); suppressed row tooltip "Extrude 1 (Suppressed)". **SUPPRESSED-MENU (psb)**: live's menu of a suppressed
    feature is Rename | Add selection to folder… | Unsuppress | Dynamic suppression | (Add comment) | Roll to here | Delete; ours
    keeps Edit…, Edit Sketch 1…, Show dimensions, Zoom to selection, Show dependencies…. **SKETCH-UNCONSUMED (psb)**: live shows
    Sketch 1 dark again once its consumer is suppressed; ours kept it hidden (eye-slash).
  - Hover: row hover highlights the geometry, geometry hover highlights no row (= ours ✅). **TOOLTIP-PARENT (psb, minor)**: live's
    dependency part has no type tag on the parent ("Boss 2 [Extrude] <- Boss sketch 2"); ours adds "[Sketch]".
  - Rename: double-click = edit (= ours), Esc cancels, **blur commits**, an **empty name reverts to the default name** (ours not
    checked yet).
  - **FOLDER-NAME-INLINE (psb)**: "Add selection to folder…" names the new folder **inline in the row** (input prefilled "New
    folder"), not in a separate "Folder name" box (that box belongs to the header's New folder button). **FOLDER-MENU (psb)**: live
    = Rename | Add selection to folder… | Suppress (or Unsuppress) | Dynamic suppression | Roll to here | Expand | Collapse |
    Delete | Unpack folder; ours = Expand, Rename | Suppress, Unsuppress (both at once) | Expand all, Collapse all | Unpack folder.
    Dragging a feature onto a folder row makes it the first child (not tested on ours yet).
  - **FOLDER-EXPAND-UNDO (psb)**: live's undo steps are the user's model actions (suppress, drags, folder creation, renames …);
    expanding/collapsing isn't one. Ours records the expand as an undo step.

## v3 Session 7 in the app: feature dialogs in everyday use (v0.4.8 c7d9d454; r5/v3s7.py: block 60×40×20 + revolve profile/axis on Front + hole points; live "Extrude dialog states", "Fillet", "All other Part Studio feature dialogs")
Same as live ✅: Extrude End type list (Blind, Up to next, Up to face, Up to part, Up to vertex, Through all) with the search
input and bold-match filtering ("thr" → **Thr**ough all), Enter picks; Through all hides Depth; Up to face adds "Up to face"
+ mate-connector button + Offset distance and hides Symmetric; title tooltip "Extrude 2 did not regenerate properly: Select face
or sketch region to extrude."; Draft = inline "3 deg" + ↻; Symmetric hides Second end position; Second end group (End type +
flip, Depth, Draft); Starting offset (End type + Depth); Thin (Mid plane, Thickness 1 5 mm + flip, Thickness 2 0 mm, …);
Surface (New | Add, "Sketch curves to extrude", no Draft); Remove adds ☐ Merge with all + Merge scope "Part 1"; a Remove tool
pointing away from the part = translucent dark red + blue toast "Selected tools and targets do not intersect." ×.
Revolve (Solid/Surface/Thin, New…Intersect, regions, Revolve axis + MC button, ☑ Full revolve; unticked: End type + ↻, Revolve
angle 30 deg, Symmetric, Second end position; an empty required axis field turns pink; 30° preview with a rotate handle).
Fillet (Edge | Full round, Measurement [Radius, Width], Control [Distance, Conic, Curvature], Radius 5 mm, Asymmetric, Partial,
Variable, ☑ Allow edge overflow, Smooth fillet corners; chip tooltip "Edge of Extrude 1"; chip right-click → "Clear all").
Chamfer (Measurement [Offset, Tangent], Chamfer type [Equal distance, Two distances, Distance and angle], Distance 5 mm,
☑ Tangent propagation). Shell (☐ Hollow, Faces to remove, Shell thickness 2.5 mm + flip, arrow handle; 25 mm → "Shell 1 did
not regenerate properly: **Could not shell part with selections.**" = live). Circular pattern (Part/Feature/Face pattern, New…,
Entities, Axis + MC, Angle 360 deg, Instance count 4 + ↻, ☑ Equal spacing, ☐ Centered, Skip instances) and Mirror (Part
mirror, Entities to mirror, Mirror plane + MC) field lists. Linear pattern / Hole checked in s1. Previews 0.32–0.90 s.
- **R5v3-S7-a → EXT-DIRECTION (psb/ui2)** ticking Direction: live makes the new "Extrude direction" field the active one (blue),
  turns the title red and the group's chevron red-brown until it's picked (live dialogs/extrude-23); ours leaves the field
  white, the title black and the chevron grey (the red-brown chevron seems to mark a group with an unfilled required field:
  live's Second end chevron, all filled, is grey).
- **R5v3-S7-b → EXT-2ND-DEPTH (psb/ui2)** the Second end position Depth row has a › expander and a ± tolerance icon on ours;
  live's has neither (dialogs/extrude-22).
- **R5v3-S7-c → SEL-REJECT-TOAST (psb)** clicking an entity the active field doesn't accept shows an invented blue toast
  ("Select sketch regions or planar faces.", part_studio/behaviors.rs filter_description); live simply doesn't highlight or
  accept it, no toast (live "qa4 round 5 s1 follow-up 2" (1)).
- **R5v3-S7-d → CHIP-HOVER (psb/ui2)** hovering a selection chip leaves its edge amber (255,169,10); live highlights it
  yellow-green #f2ff80.
- **R5v3-S7-e → FILLET-OVERFLOW (kern, live asked)** one top edge of the 60×40×20 block: R ≤ 20 volumes match the exact
  fillet ((1−π/4)·R²·60 removed: R5 47678.1, R20 42849.6); R 21…45 still report **OK**, as a quarter cylinder clipped by the
  bottom face (R21 42322.1, R25 40002.7, R45 25607.6 with 5 faces); a ball that size can't touch the 20 mm front face, so live
  probably fails or overflows differently (r5/v3s7-fillet.py).
- **R5v3-S7-f (kern/psb, live asked)** Chamfer 25 on the 20 mm face: ours "Failed to chamfer selections."; the model already
  uses live's "Distance of chamfer on face too large." for other over-size chamfers.
- Note: Remove on a picked face defaults into the part (25 mm then removes the whole 20 mm block); live's recorded case used a
  sketch region whose default pointed away; asked for the face case.

- **Live answers (behaviours "qa4 round 5 s7 dialogs")**: R5v3-S7-e **retracted**: live also reports OK for R 15…45 with the
  same volumes and face counts (R21 42322.112, R25 40002.735, R45 25607.644 = ours to the printed decimals), so FILLET-OVERFLOW
  is not a bug. R5v3-S7-f → **CHAMFER-OVERFLOW (kern)**: live's Chamfer 25 on the 20 mm face is **OK, 30000.000 mm³** (the leg
  overflows the face like the fillet); ours fails "Failed to chamfer selections.". Remove on a picked face defaults into the
  part in live too (= ours). EXT-DIRECTION confirmed with values: on ticking, the "Extrude direction" field is active (border
  rgb(22,81,176), fill rgb(222,241,251)), title red rgb(170,37,45), chevron red-brown rgb(189,48,57) (it was the disabled grey
  rgb(215,215,215) before ticking); after a pick: title dark, chevron grey rgb(102,102,102), focus moves to the next field.

## v3 Session 6 in the app: every sketch constraint, driven/driving, colours, deleting, glyph menus, dragging (v0.4.8 c7d9d454; r5/v3s6.py: one sketch with free geometry per constraint, coordinates jittered so the line tool infers nothing; live "Constraints", "qa4 round 5 s5 driven dims")
Applied by selection + shortcut, each with its glyph ✅: Coincident (endpoint–endpoint, point–circle), Concentric (Shift+O),
Parallel (B), Tangent (T), Horizontal (H), Vertical (V), Perpendicular (Shift+L), Equal (E), Midpoint (Shift+M), Symmetric
(Shift+Q, axis last), Fix (Shift+J: the line turns black = fully defined). Glyph hover (yellow box, entity highlighted) ✅;
deleting a constraint by glyph + Delete or glyph menu → Delete ✅ (geometry stays); dragging a circle carries its point-on-curve
coincident point ✅; a fixed line doesn't move when dragged ✅; empty-space right-click menu = live's (Confirm Sketch 1 | Copy
sketch | Show all, Curve/surface analysis… | Select ▸, Select other… | Zoom to fit, View normal to sketch plane) ✅; driven ↔
driving toggles via the dimension menu ("Change to driving dimension" once driven) ✅; constraint tools picked with nothing
selected stay active as a pick tool (live) ✅.
- **R5v3-S6-a → CON-NORMAL-ORDER (skb)** Normal (Shift+K) with the circle picked first fails: status "Can't add constraint:
  invalid constraint: e50 is not a line". `scene/sketch_mode.rs` make_constraint (~5946) builds `Normal { line: a, curve: b }`
  as picked; Midpoint and Symmetric already reorder by kind. (Live order-independence asked, follow-up (3).)
- **R5v3-S6-b → CON-MSG (skb)** constraint errors show internal ids ("e7 is not a line", "e50 …") in the status bar and the
  message stays there after later, successful actions; live doesn't accept a non-matching pick in the first place (no hover).
- **R5v3-S6-c → DRIVEN-LOOK (skb; live "qa4 round 5 s5 driven dims")** driven dimensions draw grey **in parentheses**
  "(23.07)", "(Ø10)"; live: grey **plain** "Ø10". A redundant dimension (auto-driven) gets **no value box** on ours; live opens
  the normal box ("10 mm" selected, ±, ✓/✕).
- **R5v3-S6-d → GLYPH-MENU (skb; same live answer (3))** right-click on a constraint glyph: ours = one centred "Delete" button
  (custom list, sketch.rs render_dim_menu); live = Confirm Sketch | Copy sketch | Show all, Curve/surface analysis… | Select,
  Select other… | Zoom to fit, View normal to sketch plane | **Delete sketch entity**.
- **R5v3-S6-e (skb, live asked (1))** Vertical on a line that is already Horizontal collapses it to a zero-length "line" (both
  glyphs, no error, nothing red).
- **R5v3-S6-f (skb, live asked (2))** Tangent line–circle and dragging the circle's centre change the circle's **radius** (r 4 →
  ~7 on applying; ~7 → ~15 when the centre was dragged 12 mm) rather than moving the line.
- **R5v3-S6-h → CON-CURVATURE (skb)** Curvature between two splines from the Spline tool is refused: "Can't add constraint:
  invalid constraint: curvature constraint not supported for e6" (CLI, /tmp/qa4/r5/v3s6b.noshape); `sketch/equations.rs`
  curv_spec (~785) only handles Line, Arc and control-point BSpline, not the interpolated spline the Spline tool makes. Pierce
  not tested (needs a 3D curve).
- **R5v3-S6-g (skb, live asked (5))** right-click on a sketch line: ours = Confirm | Copy sketch entities, Transform sketch
  entities | Curve/surface analysis…, Section view… | Clear selection, Select ▸, Select other… | Zoom to fit, Zoom to
  selection, View normal to sketch plane | Delete sketch entity (= live's earlier capture sketch/31); live's 10-24 capture (a
  Part Studio with other sketches) adds Construction, Extrude…, Revolve…, Hide other sketches, Hide all sketches, Align view
  with vertical axis and has no Section view….

- **Live answers (behaviours "qa4 round 5 s6 constraints")**: (1) V on a Horizontal line: live **adds it anyway**, toast "Sketch
  could not be solved.", line + both glyphs red, title red, the line **does not collapse** → **CON-HV-COLLAPSE (psb)** confirmed
  (ours collapses it to zero length silently). (2) Tangent line–circle: live **moves the circle, radius kept** (r 4); ours grows
  the radius → **CON-TANGENT-APPLY (psb)**; dragging the centre afterwards grows the radius in live too (= ours ✅). (3) Normal
  is order-independent in live (circle first works; the centre lands on the line) → CON-NORMAL-ORDER confirmed. (4) Fully defined
  = black line, dots and dimension, plain sketch icon (= ours for the fixed line). (5) Line menu with one sketch: Confirm |
  Copy sketch entities, Transform sketch entities | **Construction** | **Extrude…, Revolve…** | Curve/surface analysis… | Clear
  selection, Select, Select other… | (Add comment) | Zoom to fit, Zoom to selection, View normal to sketch plane, **Align view
  with vertical axis** | Delete sketch entity; no Section view… → **SK-LINE-MENU (psb)**.

## v3 Session 5 in the app: sketch constraints and dimension editing (v0.4.8 c7d9d454; doc by r5/v3s5.py: Variable #v = 10 mm + free lines and circles; live "Sketch dimension display + Enter/Esc", "qa5 dim conflict", "Constraints")
Same as live ✅: new box "62 mm" selected, ± and ✓/✕; `1 in` → label **25.4**, reopens as **`1 in`**; `2*5` → **ƒ 10** with the
tan just-applied background; `#v` → grey **ƒx 10**; Diameter box "16 mm" → **Ø10**; typed value + Esc → Esc #1 reverts to the
committed value and blurs (box stays), Esc #2 closes; the Dimension tool stays active after Enter; Equal by selection + E
(glyph "=" on both, line re-solved to length 40); glyph hover (yellow-orange box, both entities pre-highlighted), click selects
(orange glyph, yellow-green entities), Delete removes the constraint; **unsolvable** (32 → 50 against Equal 40): toast "Sketch
could not be solved." top centre with ×, sketch title + feature row red, red error icon on the Features header, conflicting
entities / dimension / glyphs red; setting 32 back clears everything within 0.5 s and restores the shape.
- **R5v3-S5-a → DIM-EXPR-UNIT (skb)** a bare expression `2*5` reopens as `2*5`; live wraps it with the document unit:
  **`(2*5) mm`** (the feature dialog already does this via quantity_field::commit_text).
- **R5v3-S5-b → DIM-AUTOCOMPLETE (skb)** the sketch dimension box has no autocomplete: live drops down "in" / "isInteger(x)"
  for `1 in` and "(x) #var 10 mm" / "(x) New variable…" for `#var`; ours shows nothing (`VarCompletion` is only wired into
  feature_dialog.rs).
- **R5v3-S5-c → DIM-ESC-UNTOUCHED (skb)** Esc on an **untouched** box (new or reopened) only blurs it (a second Esc closes);
  live closes it at once (lead's table: "Esc: closes the box … a new dimension is still created, with its measured value"); the
  two-step Esc is only for a box whose text was changed.
- **R5v3-S5-d → DIM-UNSOLVED-GEOM (skb)** while unsolvable, ours redraws the line at a half-way solve (49.6 mm tall) while the
  "50" dimension still points at the old endpoints; live keeps the last solved shape. The edited "50" stays tan, not red
  (live: the edited R2 turns red with the rest).
- **R5v3-S5-e → DIM-MENU (skb)** right-click on a sketch dimension: ours = a small custom button list ("Change to driven
  dimension", "Delete dimension"; labels centred, `part_studio/sketch.rs` render_dim_menu ~2441); live = the standard sketch
  context menu: Change to driven dimension | Confirm Sketch 1 | Copy sketch | Show all, Curve/surface analysis… | Select ▸,
  Select other… | (Add comment) | Zoom to fit, View normal to sketch plane | Delete dimension.
- **R5v3-S5-f → VAR-KEYWORD (psb/api)** a Variable can't be named `var` ("Variable name must be a letter followed by …";
  `variable_table.rs` is_identifier rejects FS_KEYWORDS); live's own test Part Studios use **`#var`** (behaviours ~4759), so
  variable names (always written `#name`) must not be checked against the FeatureScript keyword list, and the message is wrong
  for a keyword anyway.
- A redundant second Ø on the same circle is created as a grey driven "(Ø10)" without a box (live: an extra radius dimension
  is drawn grey; parentheses asked).

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

- **Live answer (behaviours "qa4 round 5 s3 mate flip")**: live's default Revolute top-face → top-face also hangs through the
  plate (Z axes aligned; = ours) and **Flip primary axis swaps it** (pin stands upside down, t (0,0,50)) for Revolute,
  Cylindrical and Planar alike → MATE-FLIP-AXIAL confirmed. Live's default also **follows the connectors' axes, not the part's
  pose** (pin laid on its side first → same result), which ours (nearest pose) doesn't.

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
