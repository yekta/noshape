# P1 Part Studio workflow pass (qa5, 2026-10-25)

App: /root/green/noshape (main ee654502, last green). Live: behaviours.md / live screenshots cited per row. Document built in the
app: sketch rectangle on Top → Extrude → Fillet → Chamfer → sketch points on the top face → Hole → Linear (feature) pattern → Shell,
then edits, undo/redo, rollback and suppress.

| Step | Ours | Live | Status |
|---|---|---|---|
| Sketch on Top (Sketch → Top row) | dialog Top plane, ☑ Show constraints, view stays iso, header "Features (4)" | same, header "Features (5)" (sketch/501) | ✗ P1-1 (ui); header icons grey ✗ P1-2 (ui) |
| Corner rectangle | width/height values stay while the tool is active | same (sketch/06) | ✅ |
| Extrude right after ✓ | field empty and active, 25 mm, ✓ disabled | same (part-studio/1290) | ✅ |
| Enter in Depth | applies, dialog stays; 2nd Enter commits | same (behaviours 252) | ✅ |
| Fillet (Shift+F), 3 vertical edges, R2 | chips "Edge of Extrude 1" ×3, preview, 15270.061 mm³ | radius arrow on the **last** pick | ✗ P1-3 (psb): ours on the first pick |
| Chamfer (tool search), 1 mm, tangent propagation | 7-edge chain chamfered (faces 9 → 16) | — | ✅ |
| Sketch on a preselected face | "Face of Extrude 1" in Sketch plane | same | ✅ |
| Points aligned with the origin | a 3rd point entity at (0,0) + Horizontal constraints | no entity; a Horizontal constraint from each point to the **external** Origin | ✗ P1-4 (skb) |
| Hole on 2 picked points | "Vertices of Sketch 2" (one chip); 2 holes, −633.385 mm³; title "Ø 6.35 mm ↧ 12 mm" ✅ | **two chips "Vertex of Sketch 1"** (one per click; "Vertices of …" only when the sketch row is picked) | ✗ **P1-8** (ui) |
| Linear pattern › Feature pattern | order Part / Feature / Face ✅; chip **"Hole 1"**; Direction pink until picked ✅ | chip **"Ø 6.35 mm ↧ 12 mm"** (the feature's shown name) | ✗ **P1-9** (ui) |
| Shell 2.5 mm (top face) | 8960 mm³, 59 faces | — | ✅ |
| Edit Extrude 1 10 → 15 mm | everything regenerates Ok but Shell returns 4 slivers, status Ok | **Shell 1 ERROR "Could not shell part with selections."**, part stays unshelled (behaviours "qa5 (10-25 p1)" (5)) | ✗ **P1-5** (psb + kern) |
| Undo / Redo / Undo | 8960 ↔ slivers ↔ 8960 | — | ✅ |
| Rollback bar above Shell 1 | Shell greyed italic, part unshelled (14220.74) | same | ✅ |
| Suppress Fillet 1 (row menu) | italic strike-through, sharp corners, rest Ok (8446.23) | same | ✅ |
| Row menus | Extrude: Rename, Edit…, Edit Sketch 1…, Show dimensions, … ✅; Fillet: no "Show dimensions" | Fillet: no "Show dimensions" either (+ Add comment, out of scope) | ✅ |
| Mirror › Feature mirror of the Hole about the Right plane | dropdown Part / Feature / Face mirror; chips "Hole 1", "Right plane"; −158.35 mm³ (two holes through the 2.5 mm floor) | — | ✅ |
| Edit Fillet 1 R2 → R1 (dialog: later features greyed, "Final" ☐) | R1 applied; **Shell 1 Error "Could not shell part with selections."** at any thickness 1–2.5 mm | live? (request (6)) | ✗? **P1-6** (kern, needs live) |
| Undo the R1 edit | back to 8801.649 mm³ | — | ✅ |
| Rename (row menu) "Edge break" | stored "Edge break" | — | ✅ |
| Row menus of Fillet / Chamfer | no "Show dimensions" (Extrude has it) | same | ✅ |
| Delete Extrude 1 (dependents) | Fillet, Chamfer, Sketch 2, Hole, Pattern, Shell, Mirror error, Parts (0); errored rows red, **but hidden Sketch 2 stays grey** | failing sketch row red (behaviours 6432, 6491) | ✗ **P1-7** (ui) |
| Undo the delete | back to 8801.649 mm³ | — | ✅ |
| Extrude › Remove from a sketch under the part (default flips away, as live) | tool grey, no message | red translucent tool + toast "Selected tools and targets do not intersect." (part-studio/234) | ✗ **P1-10** (psb + icons2) |

## P2 assembly core (same session, /root/green/noshape ee654502; plate 60×20×5 with 3 Ø10 holes + Ø10×30 pin from Part Studio 1)
| Step | Ours | Live | Status |
|---|---|---|---|
| Insert Part 1 (first instance) | placed, view fits, free DOF glyph (not fixed) | same (assembly/80, free glyph) | ✅ |
| Insert Part 2, click to place, ✓ | Instances (2) | same | ✅ |
| Cylindrical mate (tool search), pin face then hole face | pin moves into the hole, HUD at the 2nd pick, two connector chips | same (MT10, MT13) | ✅ |
| HUD ✓ | commits Cylindrical 1 (hidden, light grey row) and opens "Cylindrical 2" | "Commit current mate and start a new mate" (behaviours 1329) | ✅ |
| Drag the pin | slides along the axis; nothing fixed, so a sideways drag moves both | — | ✅ (plausible) |
| Edit the mate, type list | Slider, Cylindrical, Revolute, Pin slot, Planar, Ball, Fastened, Parallel; retype renames the row ("Pin slot 1") | same order (behaviours 5941), MT12 | ✅ |
| Undo the retype | back to Cylindrical 1 | — | ✅ |

## P2 drawing (same session)
| Step | Ours | Live | Status |
|---|---|---|---|
| Create Drawing of Assembly 1… (suggested views) | 4 views, pin seen through hole 1, edges correct | — | ✅ |
| Part Studio tab menu | Delete, Rename, Properties…, Show code, Duplicate, Copy to clipboard, Select as document thumbnail, Move to document…, Export… | same (+ Open in new browser tab, browser only) | ✅ |
| Part row › Create Drawing of Part 1… | "Part 1 Drawing 1" tab right after Part Studio 1, 4 views | — | ✅ |
| Dimension on a straight edge (assembly drawing) | "2.362" placed | — | ✅ |
| Dimension on a hole circle | part drawing "Ø.394" ✅; **assembly drawing: click not taken** | — | ✗ **P2-D1** |
| BOM panel (right strip) | "Bill of materials", BOM type ▾ Flattened, Add column ▾, ⋯; Item / Quantity / Part number / Description; rows 1 | 1, 2 | 1 | same columns (behaviours 1270) | ✅ |

## P2 assembly, green 461aefb0
| Step | Ours | Live | Status |
|---|---|---|---|
| Linear pattern (assembly): dialog | Instances, Direction (+ clock), Distance 25 mm, Instance count 2 (+ flip), ☐ Equal spacing, ☐ Centered, ☐ Second direction | A6 (pass 21) | ✅ |
| Pattern the pin 20 mm along the plate edge (flipped) | preview in hole 2; ✓ → "Linear pattern 1 ▸ Part 2 <2>", Instances (3) | — | ✅ |
| Enter twice in the pattern dialog | stays open | Part Studio dialogs commit on the 2nd Enter; assembly? (request p3) | ✗? **P2-A1** |
| Exploded views panel → Add exploded view | "Editing Explode 1 [Done]" banner, Explode 1 row, "Build steps by dragging components" | EXV rows | ✅ |

## P1 extras, green 461aefb0
| Step | Ours | Live | Status |
|---|---|---|---|
| Revolve dialog | Solid / Surface / Thin, New … Intersect, Faces and sketch regions to revolve, Revolve axis (pink until set), ☑ Full revolve | same (behaviours 338, 13863) | ✅ |
| Revolve about a profile line | full cylinder; axis chip "Sketch 4 line" | chip text? ("Edge of Sketch N" in live lists; request p3 (a)) | ? |
| Circular pattern dialog | Axis of pattern, Angle 360 deg, Instance count 4 + ↻, ☑ Equal spacing, ☐ Centered, › ☐ Skip instances | same (behaviours 331) | ✅ |
| Circular pattern of Part 3 about a hole rim | 3 copies around the hole axis; each copy gets a new palette colour | copies' colour? (request p3 (b)) | ? |
| P1-4 re-check on green 62524c30 (skb 7b972911) | points snapped to the origin's horizontal: no visible point at the origin; Hole on "Vertices of Sketch 6" previews only the 2 real points. Data still holds a hidden construction "Origin stand-in" point at (0,0) with the Horizontal constraints on it (sketch_get lists it) | ✅ UI / minor data note for skb (live: no entity, constraint to the external Origin) |
| SEL1 on green 62524c30 | Fillet: a sketch line adds nothing, a plate edge adds "Edge of Extrude 1" | ✅ |
| Instance right-click (Part 1 <1>, mated) | Properties…, Instance properties… / Hide … Fix, Use best available tessellation, Show mates, Hide mates / Edit in context … Switch to Part Studio 1 / Copy Part 1 <1> / Move to new subassembly, Change to version…, Export…, Release… / Zoom to selection … | same order (behaviours 1259, 1357; Add comment out of scope) | ✅ |
| Fix, then drag the fixed plate | ground glyph on Part 1 <1> and on the root row; dragging draws the fix glyph at the cursor and nothing moves | same (behaviours 1210, 1268) | ✅ (cursor shape not checked) |
| Sketch dimension (view normal, rectangle, d on the top edge, place) | value box "78.15024 mm" selected, ± icon, ✓ ✕; 50 + Enter → label "50", width 50 | box at full precision (behaviours 4788 "12.53718 mm") | ✅ |
| Plane (tool search; "Did you mean: Mate connector") | Entities "Top plane", type Offset, Offset distance 25 mm + flip, ☐ Flip normal; type list Offset, Plane point, Line angle, Point normal, Three point, Mid plane, Curve point, Tangent | same (behaviours 332) | ✅ |
| Split (Part tab) of the plate by Plane 1 at 2.5 mm | dialog Part / Face, Parts, surfaces, or curves to split, Entity to split with, ☐ Keep tools, ☐ Trim to face boundaries, ☑ Keep both sides; result Parts 6 → 7 | same (behaviours 329) | ✅ |
| Transform dialog | Entities to transform or copy, ▾ Translate by line (+ flip), Line or points, ☐ Copy part | same first type (behaviours 226) | ✅ |
| Mass and section properties (bottom-right icon), part without material | dialog Part / Face, red "Part 2" chip, blank mass / CoM / inertia, volume 21005.949 mm³, area 4401.292 mm²; inertia header "(mm² kg)"; yellow no-material toast | header "(lb mm²)"; toast: notes disagree | ✗ **P1-14** (ui) / ? |
| Section view (Shift+X), Front plane | dialog Exclude / Include, Section planes (+ plane button), Items to exclude, info "Select a plane, planar face, cylinder, cone, or mate connector."; chip "Section plane 1 (Front)", manipulator arrow + rotate glyph + ring, tan preview cut faces | same (behaviours 359) | ✅ |
| Draft dialog | Neutral plane / Parting line; Neutral plane (+MC); Entities to draft; Draft angle 3 deg + ↻; ☑ Tangent propagation; ☐ Reapply fillet | same (behaviours 327) | ✅ |
| Sweep dialog | Solid / Surface / Thin, New … Intersect; Faces and sketch regions to sweep; Sweep path (pink) + flip; ☐ Extend to full path; Profile control ▾ None; › ☐ Twist; ☐ Scale | same (behaviours 339) | ✅ |
| Loft dialog | Profiles; ⌄ End conditions (Start / End profile condition ▾ None); ☐ Guides and continuity; ☐ Path; › ☐ Connections; ☐ Show isocurves | same (behaviours 340) | ✅ |
| Sketch Trim (m) on a crossing line | the clicked segment past the crossing is removed; the new end is coincident with the other line | — | ✅ |
| Sketch Offset (o) on a line | light-blue offset preview with a hollow arrow manipulator, side not following the cursor | same (behaviours 172: preview + arrow, click the arrow to flip) | ✅ |
| Sketch Use (u) on a plate edge | prompt "Click edges, faces or sketch entities to use (project) them"; a black projected line with use glyphs | — | ✅ |
| n twice | the second n flips the view to the plane's back (Bottom, mirrored labels) | Onshape flips too (as far as known; not in behaviours) | ? |
| View cube face hover | fill #b1d9f1 | #bce2f7 (behaviours 403) | ✗ **P1-15** (ui, minor) |
| Drawing (Part 1) after Split in the Part Studio | Update button saturated (out of date) | same rule | ✅ |
| Drawing Section view tool: one point picked | a vertical cutting line through it ("A–A" arrows), preview "SECTION A - A" follows the cursor | live default tab Vertical (behaviours 1492) | ✅ |
| Group dialog (assembly) | one "Instances" field | same (behaviours 1333) | ✅ |
| Split of an inserted part in the Part Studio | the halves are new parts (ids from Split, renamed Part 6 / 7, others renumbered); the assembly instance of the old part fails "Failed to resolve instance." and its mate / pattern error | live Split re-identifies the same way (old id gone, halves new, renumbered; behaviours "qa5 (10-25 p3)" Split) | ✅ (consistent) |
| P2-D1 on green af9f1e4c (fix 817d9cb1) | assembly drawing, tilted plate: d on the middle hole rim → "Ø.394" preview, prompt "Click to place the dimension, or select a second entity" | — | ✅ **fixed** |
| STEP import ("+" › Import…, bracket.step) | "Import to Onshape" options; Import → tab "bracket" + folder "CAD Imports" | same dialog (behaviours 1051) | ✅ |
| Assembly drawing after the unrelated import | Update ⟳ turns saturated (out of date) | unrelated changes? (request 10-26 p5) | ? |
| Part Studio tab menu (green af9f1e4c) | Delete / Open in new browser tab, Rename, Properties…, Show code / Duplicate, Copy to clipboard / Select as document thumbnail / Move to document… (no Export…) | the newer live captures (behaviours 6561, 7153) list exactly these; the older one (280) also had Export… | ✅ |

## Pass 28, green af9f1e4c (assembly limits, live answers p2/p4/p5)
| Step | Ours | Live | Status |
|---|---|---|---|
| Cylindrical mate dialog | Limits only (no Offset), 4 limit rows Z ≑ / Z ⌶ / rotation min / max, "No minimum" / "No maximum" | same (behaviours 5942, 5959); Simulation connection excluded (simulation) | ✅ |
| Limits Z max `5` + Enter | applies and closes the dialog | 1st Enter applies, 2nd accepts | ✗ **P2-A2** (skb) |
| Drag the pin past the limit | stops at 5.0 mm along the hole connector's Z (it points down); the unfixed plate is pulled along | — | ✅ |
| Shell 1 mm, live's p2 model (depth 10, R1, 4 through holes) | "Could not shell part with selections." | OK 3998.627 mm³, 33 faces | ✗ **P1-16** (kern) |
| Shell 2.5 mm, same | OK 8487.861 mm³, 53 faces | OK 8503.961 mm³, 47 faces | ✗ **P1-16** |
| Mass properties, nothing selected | all parts measured (7178.097 mm³), toast after 3.6 s, hides by itself | field empty, values blank, no toast | ✗ **P1-17** (ui) |
| Assembly drawing after an unrelated STEP import | Update ⟳ bright | stays dim (only referenced elements count) | ✗ **P2-D2** (ui2) |
| Exploded views panel, Add exploded view | 712 px panel; "Editing Explode 1 [Done]", toolbar reduced to undo/redo, active row + rollback bar, "Build steps by dragging components" box, footer Add exploded view | same (behaviours 1834) | ✅ |
| Drag the triad's Z arrow (pin) | "Explode step" dialog: Translation ▾, Instances, ☐ Specify direction, Distance 18.3 mm, ☑ Explode lines, edges field; row "↗ Explode step 1"; info box gone | same | ✅ |
| Distance `25` + Enter, Enter | 1st applies (pin moves), 2nd accepts | Onshape number fields | ✅ |
| Done, then double-click Explode 1 | plain row, unexploded; re-enters editing fully exploded | same | ✅ |
| Step row right-click | Edit… / Rename / — / Delete | same (behaviours 815) | ✅ |
| Esc with no dialog in explode editing | toolbar back but still exploded, row stays active with the rollback bar; the next double-click leaves instead of re-entering | Esc = Done | ✗ **P2-A3** (skb) |
| Assembly circular pattern dialog | Instances, Axis of pattern (+ clock), Angle 360 deg, Instance count 2 (+ ↻), ☑ Equal spacing, ☐ Centered; ✓ pale until Instances is set | same (behaviours 6186, 8704) | ✅ |
| Pin about the middle hole's rim, ✓ | copy in hole 3; "Circular pattern 1 ▸ Part 2 <2>", Instances (3); info bubble "1 instance(s) created." | bubble? (request 10-26 p6) | ? |
| Edit the seed's mate | the pattern copy vanishes until ✓; seed shows z-fighting stripes | instances stay drawn | ✗ **P2-A4** (skb) |
| Mate type → Revolute | Offset, Limits (rotation min / max kept); row "Revolute 1"; Z offset 0 (the pin rises to mid-height) | same (behaviours 5942) | ✅ |
| Fix the plate, drag the revolute pin | rotates only: axis stays on hole 1 (0.002 mm), plate unchanged; the copy follows | behaviours 5948 (1) | ✅ |
| Mate row right-click (hidden Revolute) | Rename, Edit…, Reset / Show, Show all mates, Isolate…, Make transparent… / Edit named position driver mate… / Suppress, Dynamic suppression ▸ / Animate… / Add selection to folder… / Expand ▸, Collapse ▸ / Delete | same (behaviours 1361; Add comment is cloud) | ✅ |
| Suppress the mate | row grey struck-through, not italic; the pin drags freely out of the hole | grey italic struck-through | ✗ **P2-A5** (skb, minor) |
| Ctrl+Z / Ctrl+Y on the suppress | unsuppressed / suppressed again | — | ✅ |
| Circular pattern 1 count 4 (edit), Enter twice | stays open (P2-A1, fix 9bf8475b not in this green) | 2nd Enter accepts | ✗ P2-A1 (fixed on main, pending green) |
| Instance list header after count 4 | "Instances (5)" | top-level rows: "Instances (3)" | ✗ **P2-A6** (skb) |
| Replicate dialog, Instances = Part 2 <1> | Mate to be matched auto-filled "Revolute 1"; Match faces in parts ▾; Parts to find match in (red) | same (behaviours 5969–5970) | ✅ |
| … pending list row while open | none | "Replicate 1" row with an error badge at once | ✗ **P2-A7** (skb) |
| Parts to find match in = plate, ✓ | pins in both free Ø10 holes; "Replicate 1" ▸ Part 2 <5>, <6>, Mate features (2) | same structure (behaviours 5973) | ✅ |
| Linear pattern opened from tool search | red "Linear pattern 1" row with a red dot at once, kept after picking Instances | same (behaviours 5985) | ✅ |
