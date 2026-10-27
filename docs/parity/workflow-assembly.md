# Workflow: multi-part assembly (a CAD user's session, step by step against live Onshape)

Document "qa5 assembly" (mm). Part Studio 1 = Part 1 plate 100×60×10 with Ø10 holes at (25,30) and (75,30),
Part 2 shaft Ø10×40, Part 3 pin Ø10×20, Part 4 block 20×10×10 (built with noshape-cli as setup; script
/root/qa5-scratch/mkasm.py). Imported part: g25-b.step (an Onshape STEP export, "Part 1"), copied to
/tmp/qa5/import/bracket.step. Everything after that by mouse and keyboard.
Live reference: behaviours.md "Assembly" (toolbar, Insert dialog, Insert placement in an empty assembly,
Instances/triad, Mates, Exploded views, BOM panel). Not covered yet (request "qa5 (10-21 assembly workflow)"):
Replicate, assembly linear/circular pattern dialogs, Slider/Cylindrical specifics.
Status: ✅ matches live, ✗ differs (finding #), ? live data pending.

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206.

## 1. Import and insert

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1.1 | + › Import… → STEP | file prompt, then the Import dialog | path prompt (no-native-dialog mode), "Import to Onshape" dialog: Import to this document / Combine to a single Part Studio, ☑ Import appearances, ☐ Y axis up, ☐ composite, ☐ join surfaces | ✅ (see kern G38h for live's dialog) |
| 1.2 | Import | Part Studio tab named after the file; the new tabs go right after the open tab (SPECS 10-27, behaviours Import "inserted right after the active tab") | tab "bracket" (Import 1 → Part 1) + folder tab "CAD Imports"; the new tab is placed after Part Studio 1, not at the end | ✅ (= live with Part Studio 1 open) |
| 1.3 | Assembly, i | "Insert parts and assemblies", Current document › Part Studios, filters, tree with thumbnails, "Inserted: 0", "Undo to remove instances" | same | ✅ |
| 1.4 | Click Part 1 (empty assembly) | instance at identity, **amber (selected)**, row selected, view zooms to fit it | at identity, but **not amber, row not selected**; the zoom-to-fit crops the plate at the right edge | ✅ (skasm 10-27: amber + row selected; fitted beside the Insert dialog, GUI :103); was ✗ A1 |
| 1.5 | Click Part 2, move to graphics, click | follows the cursor amber; drops at the click | same | ✅ |
| 1.6 | Bracket (Part 1 of "bracket") | "Part 1 <2>": live numbers by name across parts, lowest free number (behaviours "skasm (2026-10-27) instance numbering") | instance named "Part 1 <2>" (`next_instance_name`: by name, lowest free) | ✅ |
| 1.7 | ✓, F | – | 5 instances | ✅ |
| 1.8 | Part 1 <1> › right-click | menu (skb list) | same items (Add comment omitted, cloud) | ✅ |
| 1.9 | Fix | ground glyph on the row and on Assembly 1 | same | ✅ |

## 2. Mates and mate connectors

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 2.1 | Revolute: shaft bottom edge, hole 1 top edge | connector snaps to the circle centre; 2nd pick jumps the part, **amber preview**, HUD | snaps, jumps, HUD; the moved part is drawn normally (not amber) | ✅ (GUI green 8165fca6, skasm 10-27: the moved part amber while the dialog is open); was ✗ A2 |
| 2.2 | Dialog ✓ | "Revolute 1" row, **new mates hidden: light grey row, tooltip "… (Hidden)"** | row in normal dark text | ✅ (GUI green 8165fca6, skasm 10-27: the new mate row light grey, "(Hidden)"); was ✗ A3 |
| 2.3 | Drag the shaft | rotates about the axis only | stays on the axis | ✅ |
| 2.4 | Cylindrical: pin bottom edge, hole 2 top edge; drag up | slides + rotates | aligned (2nd connector landed at the hole's bottom edge); drag slides along the axis | ✅ / ? |
| 2.5 | Slider: block top face → plate top face; HUD flip primary axis | flip turns the block over | block stays inside the plate; CLI: **rotation identity after the flip** (flip had no effect) | ✅ (T: assembly `slider_flip_primary_turns_the_part_over`); was ✗ A4 |
| 2.6 | Fastened: bracket → plate; flip, flip back | flips | flips correctly | ✅ |
| 2.7 | Mate connector tool hover on a face | **orange outline on the hovered face, white dots only at that face's inference points, small triad + ring** (assembly/31-*) | no face outline, **dark dots on many edges of other faces**, large triad without ring | ✅ (GUI :103 skasm 10-27: orange outline on the hovered face, small triad + ring at the nearest inference point, no dots on other parts); was ✗ A5 |
| 2.8 | Place, ✓ | "Mate connector 1" under Mate features | same | ✅ |

## 3. Replicate, patterns

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 3.1 | Replicate dialog | standard feature dialog (✓ ✕) (pending capture) | custom panel: ALL-CAPS headers "SEED INSTANCE", "MATE TO BE MATCHED", a "Use selection" button, yellow hint text, a blue **OK** button, no ✓ | ✅ (GUI green 8165fca6, skasm 10-27: Replicate is a standard feature dialog ✓ ✕); was ✗ A6 |
| 3.2 | Seed Part 3 <1> (Cylindrical 1), OK | one copy per free matching hole | "3 match(es) found": **two pins stacked inside hole 1 (where the shaft is) and one on top of the seed in hole 2** (z-fighting stripes) | ✗ A7 |
| 3.3 | Ctrl+Z | removes it | removed | ✅ |
| 3.4 | Assembly linear pattern dialog | standard feature dialog with preview | same custom panel style (INSTANCES / DIRECTION, boxed inputs, OK); **no preview** | ✅ (GUI green 8165fca6, skasm 10-27: Linear pattern a standard feature dialog); was ✗ A6, A8 |
| 3.5 | Direction = plate front edge, 25 mm × 3 | copies along the edge | the edge pick becomes "Mate connector of Part 1 <1>" and copies go **along its Z (the face normal, −Y)**, off the plate | ✅ (T: assembly `linear_pattern_along_an_edge`); was ✗ A9 |
| 3.6 | Assembly circular pattern, axis = shaft face, 4 × 360 | 3 copies around the axis | 3 copies around the shaft axis; tree "Circular pattern 1 (3)" | ✅ / ? |
| 3.7 | Button tooltip after clicking it | hides | "Assembly circular pattern" tooltip stays over the graphics | ✗ A10 |

## 4. Explode, BOM

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 4.1 | Strip › Exploded views | panel, centred Add exploded view, Learn more link | same | ✅ |
| 4.2 | Add exploded view | banner "Editing Explode 1 [Done]", reduced toolbar, Explode 1 + rollback bar, info box | same | ✅ |
| 4.3 | Select pin, drag Z arrow | "Explode step" dialog: Translation ▾, Instances, ☐ Specify direction, Distance, ☑ Explode lines, field; info box gone; row "Explode step 1" | same | ✅ |
| 4.4 | ✓, Done | unexploded graphics, banner gone | same | ✅ |
| 4.5 | BOM panel | BOM type Flattened, Add column ▾, ⋯; Item / Quantity / Part number / Description | same; Part 3 quantity 6 | ✅ |
| 4.6 | Double-click Part number, "PN-001", Enter | commits, selection moves one row down | same | ✅ |
| 4.7 | Type into the selected cell | typing starts editing | **nothing happens** (typed text lost) | ✅ (GUI :103 skasm 10-27: "PN-002" typed into a selected cell lands whole, Enter commits); was ✗ A11 |

## Findings (2026-10-21 run)

| # | Finding | Repro | Owner |
|---|---|---|---|
| A7 | **Replicate places duplicates**: two copies stacked in an occupied hole and one on top of the seed; should be one per free matching target, never the seed's own | §3.2 | skb |
| A9 | **Assembly linear pattern goes the wrong way** when the direction is an edge: uses the auto mate connector's Z (face normal) instead of the edge direction | §3.5 | skb |
| A4 | Slider: HUD "Flip primary axis" has no effect (instance rotation unchanged) | §2.5 | skb |
| A6 | Replicate / linear / circular pattern dialogs are custom panels (caps headers, OK button, no ✓, boxed inputs), not Onshape feature dialogs | §3.1, 3.4 | skb + ui2 |
| A8 | Assembly patterns show no preview while editing | §3.4 | skb |
| A1 | First inserted instance not selected/amber; zoom-to-fit crops it | §1.4 | skb |
| A2 | Mate preview: moved part not amber | §2.1 | skb |
| A3 | New mates not hidden by default (row not light grey) | §2.2 | skb |
| A5 | Mate connector inference look: no face outline, dark dots everywhere, big triad without ring | §2.7 | skb / ui2 |
| A10 | Toolbar tooltip stays after clicking the button (dialog open) | §3.7 | ui2 |
| A11 | BOM: typing into a selected cell doesn't start editing | §4.7 | skb |

## Update from live's answer (behaviours.md "qa5 assembly", 10-21)

| # | Row | Live | Verdict |
|---|---|---|---|
| 2.1 | Revolute | connector on a cylindrical face lands at mid-height; shaft keeps +Z; drag rotates only | ✅ (we picked edges) |
| 2.4 | Cylindrical | pin mid-height connector → hole top centre; drag slides + rotates | ✅; our 2nd pick on the hole's top edge landed at its bottom → ? |
| 2.5 | Slider | slides along the connectors' Z (face normal), Limits fields "No minimum"/"No maximum" | A4 stays (flip had no effect) |
| 2.7 | Mate connector hover | white inference dots on the hovered face + triad preview; dialog: [On entity \| Between entities], Origin entity, ☐ Realign, ☐ Move | A5 confirmed |
| 3.1 | Replicate dialog | Instances; Mate to be matched (auto-filled from the instance); [Match faces in parts \| Match faces in features \| Match individual faces]; Parts to find match in | A6 confirmed |
| 3.2 | Replicate result | **one** new pin in hole 1 (the only other Ø10 face), row "Replicate 1" with Part 3 <2> + Mate features (1) | A7 confirmed |
| 3.4–3.5 | Linear pattern | Instances, Direction, Distance 25 mm, Instance count **2**, ☐ Equal spacing, ☐ Centered, ☐ Second direction; plate edge → copies along X | A6, A9 confirmed |
| 3.6 | Circular pattern | Instances, Axis, Angle 360, count 2, ☑ Equal spacing, ☐ Centered | ✅ result |
| list | Instance list | pattern rows appear while the dialog is open; header counts top-level rows ("Instances (7)") | ✅ (qa5 P2-A6 / P2-A7, 9703d9b9, verified 21d6a89e; skasm 10-27 saw the pending "Replicate 1" row) |
| 4.5 | BOM | Flattened default; Item/Quantity/Part number/Description | ✅ |

## Re-run pass 3 (2026-10-21 14:55–15:00, /tmp/qa5/noshape-1433 = target-shared 14:32; new Assembly 2 + Assembly 1)

| # | Row | Ours | Status |
|---|---|---|---|
| 1.4 | First insert into an empty assembly | **amber + row selected now** (the "Assembly 2" root row is highlighted too); zoom-to-fit still crops the plate (right and bottom off-screen, dialog over it) | ✅ (skasm 10-27: amber + row selected; fitted beside the Insert dialog, GUI :103); was ✗ A1 (partly fixed) |
| 2.1 | Revolute 2nd pick | moved part previews **amber** | ✅ A2 fixed |
| 2.2 | New mate row | tooltip "Revolute 1 (Hidden)" + eye-off icon ✅, but the row text is still #333333 (live light grey) | ✅ (GUI green 8165fca6, skasm 10-27: the row light grey); was ✗ A3 (partly fixed) |
| 2.5 | Slider flip primary axis | block still inside the plate (only its top face z-fights at the plate top) | ✅ (T: assembly `slider_flip_primary_turns_the_part_over`); was ✗ A4 |
| 2.7 | Mate connector hover on the plate top | orange outline + white inference dots on that face ✅; **extra white dots on other parts** (bracket edges, shaft bottom) | ✅ (GUI :103 skasm 10-27: orange outline on the hovered face, small triad + ring at the nearest inference point, no dots on other parts); was ✗ A5 (partly fixed) |
| 3.1 / 3.4 / 3.6 | Replicate / Linear / Circular pattern dialogs | standard feature dialogs with live's fields (dom/ui2-asm-*) ✅; Replicate adds red hints "Select the instances to pattern (seed)" / "Select the geometry to find match in" that live doesn't show | ✅ A6 fixed / ✗ **A14** (hint text) |
| 3.2 | Replicate Part 3 <1> (Cylindrical 1) in Part 1 | one copy in hole 1 → "Replicate 1 ▸ Part 3 <7>" ✅; the new mate "Cylindrical 2" goes to the top-level Mate features (live nests "Mate features (1)" under the Replicate row) | ✅ (T: assembly `replicate_on_matching_holes`, qa5 A12 assert); was ✅ A7 fixed / ✗ **A12** |
| 3.4 | Linear pattern preview | grey preview copy | ✅ A8 fixed |
| 3.5 | Direction = plate front edge | copy along the edge, 25 mm | ✅ A9 fixed |
| 3.7 | Toolbar button tooltip after click | hides | ✅ A10 fixed |
| 4.7 | Type into a selected BOM cell | editing starts but the **first character is lost** ("PN-002" → "N-002") and is committed on its own: Esc leaves "P" in the cell; Delete doesn't clear a cell | ✅ (GUI :103 skasm 10-27: "PN-002" whole; Enter commits and moves down); was ✗ A11 (changed) |
| – | Relation toolbar button | live: Gear / Rack and pinion / Screw / Linear relation tools (feature dialogs) | **custom "Relation" panel**: tabs Gear / Rack & pinion / Screw / Linear, "Select mate" dropdowns, Lead per radian, blue **OK** | ✗ **A13** |
| – | Tab position of a new Assembly made with + while Part Studio 1 was active | after Part Studio 1 (stays there) | was placed after Part Studio 1, **later shown at the end of the tab bar** (after a BOM edit / undo / tab switches) | ✗ **DOC5** |

## Update from live's answer (behaviours.md "Assembly relation dialogs", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| – | Relation tools | **four toolbar buttons** (Gear / Rack and pinion / Screw / Linear relation); **one feature dialog** (220 px, "<Type> 1" ✓ ✕): unlabelled type select (Gear, Rack and pinion, Screw, Linear), **Mate(s)** query (chips like "⟳ - z - Revolute 1"), one value row (Relation ratio 1 / Distance per revolution 25 mm), ☐ Reverse direction; row "Gear 1" etc. at the end of Mate features | one button opening a custom "Relation" panel with tabs, "Select mate" dropdowns, "Lead per radian 1 mm", blue OK | ✗ **A13 confirmed** (skb) |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| A1 first insert | amber + selected ✅, fit crops plate | ✗ still (partly) |
| A3 hidden mate rows | #333333 | ✗ still |
| A4 slider flip | no effect | ✗ still |
| A5 MC hover dots | dots on other parts | ✗ still |
| A11 BOM typing | first char lost ("QA1" → "A1") | ✗ still |
| A12 Replicate mate placement | "Cylindrical 2" top level | ✗ still |
| A13 relations | custom Relation panel | ✗ still |
| A14 Replicate red hints | still shown | ✗ still |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
Pass 6 (17:11) had found A12 fixed (mate nests under "Replicate 1 ▸ Mate features (1)").
| Row | Ours | Status |
|---|---|---|
| A1 first insert fit | as tight as live (ins-902: live's block also runs under the right strip) | ✅ fit |
| A15 (new) pending first instance look | face (229,163,54) with **black edges** vs live (242,182,72) with amber edges (255,192,76); **a transform triad at the instance** (live none); root row "Assembly 5" highlighted with the instance row (live only the instance row) | ✗ **A15** (skb / ui2) |
| A3 hidden mate rows | light grey | ✅ **fixed** |
| A13 relations | four toolbar buttons; Gear → "Gear 1" dialog: type select, Mate(s), Relation ratio 1, ☐ Reverse direction | ✅ **fixed** |
| A11 typing into a BOM cell | "QA1" kept | ✅ **fixed** (Ctrl+Z doesn't revert the BOM edit; live ?) |
| A14 Replicate red hints | none | ✅ **fixed** |
| A16 (new) | "solving…" label at the top right for as long as the Replicate dialog is open (nothing selected); gone on cancel | ? A16 (live request filed) |
| A4 Slider flip | block still inside the plate (Front view) | ✗ still |
| A5 mate connector hover | dots only on the hovered face, small triad + ring | ✅ **fixed** |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (behaviours.md ~6868): no "Solving…" text with an empty Replicate; BOM edits are not on the undo stack.
| Row | Ours | Status |
|---|---|---|
| A15 selected instance | (242,182,73) top, amber edges, only the instance row highlighted | ✅ **fixed** |
| A16 "solving…" label | gone | ✅ **fixed** |
| A19 (new) empty Replicate | title red ✅; **no pending red "Replicate N" row with error badge in the list, no badge on the root row, no red border on "Parts to find match in"** (live has all three) | ✗ **A19** (skb) |
| A11 BOM edit + Ctrl+Z | not undone | ✅ (= live) |
| A4 Slider flip | block still inside the plate | ✗ still |
| A18 repeat insert, ✓ without entering graphics | (5.000025, 2.500025, 1.250025) | ✅ **fixed** (live exact 5 / 2.5 / 1.25) |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status |
|---|---|---|
| A19 empty Replicate | pending red "Replicate 1" row + badge, root badge, red border on "Parts to find match in" | ✅ **fixed** (m80) |
| A4 Slider flip | not re-run: the "qa5 assembly" doc was lost with /tmp/qa5/home | – |

## Re-run pass 14 (2026-10-22, main aa75fdf, app /tmp/qa5/noshape-0503; doc rebuilt: /tmp/qa5/d14/asm.noshape via /root/qa5-scratch/pass14/mkasm2.py = mkasm + plate fixed, block, shaft)
| Row | Ours | Status |
|---|---|---|
| A4 Slider (block top → plate top) + HUD flip primary axis | flip turns the block over: it sits on the plate (p02, p03) | ✅ **fixed** |

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
| Row | Ours | Status |
|---|---|---|
| A1 fit | F fits all three instances; the far block runs under the right strip as in live ins-902 | ✅ |
| A10 toolbar tooltip after click | gone once the dialog opens (b01) | ✅ **fixed** |
| A11 BOM typing into a selected cell | "QA1" stored, Enter moves down (b05) | ✅ **fixed** |
| A13 relations | tool search "relation": Gear relation, Screw relation, Linear relation, Rack and pinion relation (b10) | ✅ **fixed** |
| A6 pattern dialogs | "Linear pattern 1" feature dialog (✓ ✕, red title, pending red row with badge): Instances, Direction (+ clock), Distance 25 mm, Instance count 2 (+ flip), ☐ Equal spacing, ☐ Centered, ☐ Second direction (b11) | ✅ **fixed** |
| A8 pattern preview | grey preview copy while editing (b14) | ✅ **fixed** |
| A9 direction from an edge | the copy moves along the picked edge (implicit "Mate connector of Part 1 <1>" on the edge), not the face normal; it goes toward −X (b14) | ✅ direction / ? sign |
| A5 MC inference | hovering the hole rim: rim highlighted, connector at the hole centre, no dots on other parts (b16) | ✅ **fixed** |
| DOC5 new Assembly tab position | + › Create Assembly with Part Studio 1 active → "Assembly 2" right after Part Studio 1, stays there after tab switches (t01) | ✅ **fixed** |
| A15 / A18 | fixed in pass 8 (rows in workflow-configured-assembly.md are stale) | ✅ |

## skb P3 check (2026-10-24 evening, agent/skb after the drag-speed batch)
| Row | Live | Ours | Status |
|---|---|---|---|
| PT1 pattern field chips | Direction "Edge of Part 1 <2>", Axis of pattern "Face of Part 2 <1>", Mirror plane "Mate connector" (dom/dialog-asm-{linear,circular}-pattern-filled, -mirror-filled) | were "Mate connector of Part 1 <1>" for all three; now the entity ("Edge of Part 1 <1>") for Direction/Axis, "Mate connector" for the mirror plane (perf/p21, p22) | ✅ **fixed** |
| PT2 pattern follows its seed | copies keep their pattern placement relative to the seed | circular pattern of a revolute pin (6): dragging the seed turns every copy by the same −19.2° (CLI list), 1.6 ms per drag solve | ✅ |
| A9 sign | (behaviours "skb (10-24) linear pattern edge direction") the B-rep edge direction (tangent at parameter 0), a planar face's normal; ⇄ negates; ⇄ is kept when the Direction is re-picked | same rule (our edge direction, ⇄ negates, ⇄ kept on re-pick). Same 40×20×10 block via the CLI (/root/skb-scratch/pat/mk.py): (a) bottom front edge +X ✅, (d) top face +Z ✅; but **(b) top back edge +X (live −X) and (c) vertical edge x40 y0 +Z (live −Z)**. The difference is the **kernel's edge orientation** of an extruded body: Parasolid's cap edges follow the sketch curves on both caps, and its side edges run top → bottom; OCCT's don't | ✗ **A9b** (kernel, not skb: reported to the lead) |
| LI1 instance list clicks (behaviours "skb (10-24) section base snap + move to origin" (3)) | plain click adds (a selected row toggles off); Shift+click selects the range from the last click over the visible rows; Ctrl+click opens the row menu, selection unchanged; Esc doesn't clear | was: plain click replaced the selection, Shift/Ctrl toggled one row. Now as live (perf/c1–c2, r2); Esc never cleared (Space does) | ✅ **fixed** |
| LI2 pattern row position | in the instance order where its first instance is ("<3>, Linear pattern 1, <5>") | was after every instance; now in place (perf/r1) | ✅ **fixed** |
| LI3 Shift-range over a pattern row | the pattern row itself is selected too | now the range runs over the visible instance and pattern rows; a pattern row in it is selected with the instances (perf/li3) | ✅ **fixed** |
| MO1 Move to origin | first item of the triad's right-click menu only; the triad point (bounding-box centre, or the clicked point) goes to the origin, rotation kept; a fastened instance doesn't move | same (code: triad menu centre item, `triad_action`, solve with targets) | ✅ |
| SV1 Section view… from a face's menu | "Section plane 1 (Top)" parallel to the face, manipulator at its centroid | was an empty "Section planes" (the hover is gone by the menu click); now the selected face seeds it (mates/sv06) | ✅ **fixed** |
| SV2 section base drag snapping (behaviours "skb (10-24) section base snap + move to origin" (1)) | vertex ≤ 22 px; a face bounded by one circle → its centre; an edge of the face under the pointer ≤ 35 px → nearest point (edge orange, connector triad + pale disc); on the face → the face point; off the parts → free; on release the plane moves through the snapped point | same (`section_base_snap`, CPU ray pick + B-rep edge→face adjacency): back edge in-plane keeps z 5 (sv07/08); bottom front edge (z 0) moves the cut to z 0 on release (sv12/13) | ✅ |
| SV3 cut exactly on a part's face | the cap is drawn hatched at the base (skb-sv/bottom-edge-released) | nothing drawn for the coplanar cut (renderer capping, not skb) | ✗ (renderer) |
| HV1 face hover (no tool) | the face's boundary edges orange, no fill (behaviours "Assembly selection model" (1)) | was nothing (edges did highlight; same on main 538981d5); now the boundary is outlined (mates/hv-fix) | ✅ **fixed** |
| RM1 right-click an unselected instance | the instance menu (no Clear selection when nothing is selected), e.g. Section view… cutting parallel to the right-clicked face (live skb-move-origin-menu-graphics.png) | was the general (empty-space) menu; now the instance menu, Section view… from it uses that face (mates/rm01, rm02) | ✅ **fixed** |
| DR1 DRAG-RUNAWAY (qa4 round 5 v3 s10; live: only the dragged cube moves, ≈21 ms a step) | dragging the top cube of a 10/19-cube Planar stack moves only that cube | was: the column leant, and at 19 cubes the others flew ±2.5 million mm. Now least change: the dragged instance alone is solved unless moving the rest of its group gets the drag closer (a one-step check with the mates rigid), and undragged free bodies get a stay term when the whole group must move. qa4's v3s10n.py NCUBES=19 in the app: a 150 px drag moves only Part 2 <19> (70 mm), the 18 others unchanged; solve 3 ms/step (p50, app log). Tests: `planar_stack_drag_moves_only_the_dragged_cube`, `coupled_drag_leaves_free_bodies_put`; perf `drag_speed` (`--ignored`): 19 cubes 2.2 ms, 20-link chain 18 ms | ✅ **fixed** |
| CX1 CHIP-X-SELECTED (qa4) | a pattern dialog chip's × removes the instance while it's selected | the field re-took it from the selection; now × also deselects it (perf/cx02, cx03) | ✅ **fixed** |

## Release re-check v0.4.8 (qa5, 2026-10-25 04:00, release asset noshape 0.4.8; doc built with v048/tools/a12.py + a12b.py: plate with three Ø10 holes, Ø10 × 30 pin, Cylindrical 1 in hole 1, Replicate over the plate)
| Row | Ours | Status |
|---|---|---|
| A12 Replicate mate placement | two copies (Part 2 <2>, <3>); tree: "Replicate 1 ▸ Part 2 <2>, Part 2 <3>, Mate features (2)"; top-level "Mate features (1)" keeps only Cylindrical 1 (light grey, hidden as A3) | ✅ |
