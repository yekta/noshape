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
| 1.2 | Import | Part Studio tab named after the file | tab "bracket" (Import 1 → Part 1) + folder tab "CAD Imports"; the new tab is placed after Part Studio 1, not at the end | ? |
| 1.3 | Assembly, i | "Insert parts and assemblies", Current document › Part Studios, filters, tree with thumbnails, "Inserted: 0", "Undo to remove instances" | same | ✅ |
| 1.4 | Click Part 1 (empty assembly) | instance at identity, **amber (selected)**, row selected, view zooms to fit it | at identity, but **not amber, row not selected**; the zoom-to-fit crops the plate at the right edge | ✗ A1 |
| 1.5 | Click Part 2, move to graphics, click | follows the cursor amber; drops at the click | same | ✅ |
| 1.6 | Bracket (Part 1 of "bracket") | ? | instance named "Part 1 <2>" | ? |
| 1.7 | ✓, F | – | 5 instances | ✅ |
| 1.8 | Part 1 <1> › right-click | menu (skb list) | same items (Add comment omitted, cloud) | ✅ |
| 1.9 | Fix | ground glyph on the row and on Assembly 1 | same | ✅ |

## 2. Mates and mate connectors

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 2.1 | Revolute: shaft bottom edge, hole 1 top edge | connector snaps to the circle centre; 2nd pick jumps the part, **amber preview**, HUD | snaps, jumps, HUD; the moved part is drawn normally (not amber) | ✗ A2 |
| 2.2 | Dialog ✓ | "Revolute 1" row, **new mates hidden: light grey row, tooltip "… (Hidden)"** | row in normal dark text | ✗ A3 |
| 2.3 | Drag the shaft | rotates about the axis only | stays on the axis | ✅ |
| 2.4 | Cylindrical: pin bottom edge, hole 2 top edge; drag up | slides + rotates | aligned (2nd connector landed at the hole's bottom edge); drag slides along the axis | ✅ / ? |
| 2.5 | Slider: block top face → plate top face; HUD flip primary axis | flip turns the block over | block stays inside the plate; CLI: **rotation identity after the flip** (flip had no effect) | ✗ A4 |
| 2.6 | Fastened: bracket → plate; flip, flip back | flips | flips correctly | ✅ |
| 2.7 | Mate connector tool hover on a face | **orange outline on the hovered face, white dots only at that face's inference points, small triad + ring** (assembly/31-*) | no face outline, **dark dots on many edges of other faces**, large triad without ring | ✗ A5 |
| 2.8 | Place, ✓ | "Mate connector 1" under Mate features | same | ✅ |

## 3. Replicate, patterns

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 3.1 | Replicate dialog | standard feature dialog (✓ ✕) (pending capture) | custom panel: ALL-CAPS headers "SEED INSTANCE", "MATE TO BE MATCHED", a "Use selection" button, yellow hint text, a blue **OK** button, no ✓ | ✗ A6 |
| 3.2 | Seed Part 3 <1> (Cylindrical 1), OK | one copy per free matching hole | "3 match(es) found": **two pins stacked inside hole 1 (where the shaft is) and one on top of the seed in hole 2** (z-fighting stripes) | ✗ A7 |
| 3.3 | Ctrl+Z | removes it | removed | ✅ |
| 3.4 | Assembly linear pattern dialog | standard feature dialog with preview | same custom panel style (INSTANCES / DIRECTION, boxed inputs, OK); **no preview** | ✗ A6, A8 |
| 3.5 | Direction = plate front edge, 25 mm × 3 | copies along the edge | the edge pick becomes "Mate connector of Part 1 <1>" and copies go **along its Z (the face normal, −Y)**, off the plate | ✗ A9 |
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
| 4.7 | Type into the selected cell | typing starts editing | **nothing happens** (typed text lost) | ✗ A11 |

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
| list | Instance list | pattern rows appear while the dialog is open; header counts top-level rows ("Instances (7)") | ? not checked in ours |
| 4.5 | BOM | Flattened default; Item/Quantity/Part number/Description | ✅ |

## Re-run pass 3 (2026-10-21 14:55–15:00, /tmp/qa5/noshape-1433 = target-shared 14:32; new Assembly 2 + Assembly 1)

| # | Row | Ours | Status |
|---|---|---|---|
| 1.4 | First insert into an empty assembly | **amber + row selected now** (the "Assembly 2" root row is highlighted too); zoom-to-fit still crops the plate (right and bottom off-screen, dialog over it) | ✗ A1 (partly fixed) |
| 2.1 | Revolute 2nd pick | moved part previews **amber** | ✅ A2 fixed |
| 2.2 | New mate row | tooltip "Revolute 1 (Hidden)" + eye-off icon ✅, but the row text is still #333333 (live light grey) | ✗ A3 (partly fixed) |
| 2.5 | Slider flip primary axis | block still inside the plate (only its top face z-fights at the plate top) | ✗ A4 |
| 2.7 | Mate connector hover on the plate top | orange outline + white inference dots on that face ✅; **extra white dots on other parts** (bracket edges, shaft bottom) | ✗ A5 (partly fixed) |
| 3.1 / 3.4 / 3.6 | Replicate / Linear / Circular pattern dialogs | standard feature dialogs with live's fields (dom/ui2-asm-*) ✅; Replicate adds red hints "Select the instances to pattern (seed)" / "Select the geometry to find match in" that live doesn't show | ✅ A6 fixed / ✗ **A14** (hint text) |
| 3.2 | Replicate Part 3 <1> (Cylindrical 1) in Part 1 | one copy in hole 1 → "Replicate 1 ▸ Part 3 <7>" ✅; the new mate "Cylindrical 2" goes to the top-level Mate features (live nests "Mate features (1)" under the Replicate row) | ✅ A7 fixed / ✗ **A12** |
| 3.4 | Linear pattern preview | grey preview copy | ✅ A8 fixed |
| 3.5 | Direction = plate front edge | copy along the edge, 25 mm | ✅ A9 fixed |
| 3.7 | Toolbar button tooltip after click | hides | ✅ A10 fixed |
| 4.7 | Type into a selected BOM cell | editing starts but the **first character is lost** ("PN-002" → "N-002") and is committed on its own: Esc leaves "P" in the cell; Delete doesn't clear a cell | ✗ A11 (changed) |
| – | Relation toolbar button | live: Gear / Rack and pinion / Screw / Linear relation tools (feature dialogs) | **custom "Relation" panel**: tabs Gear / Rack & pinion / Screw / Linear, "Select mate" dropdowns, Lead per radian, blue **OK** | ✗ **A13** |
| – | Tab position of a new Assembly made with + while Part Studio 1 was active | after Part Studio 1 (stays there) | was placed after Part Studio 1, **later shown at the end of the tab bar** (after a BOM edit / undo / tab switches) | ✗ **DOC5** |

## Update from live's answer (behaviours.md "Assembly relation dialogs", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| – | Relation tools | **four toolbar buttons** (Gear / Rack and pinion / Screw / Linear relation); **one feature dialog** (220 px, "<Type> 1" ✓ ✕): unlabelled type select (Gear, Rack and pinion, Screw, Linear), **Mate(s)** query (chips like "⟳ - z - Revolute 1"), one value row (Relation ratio 1 / Distance per revolution 25 mm), ☐ Reverse direction; row "Gear 1" etc. at the end of Mate features | one button opening a custom "Relation" panel with tabs, "Select mate" dropdowns, "Lead per radian 1 mm", blue OK | ✗ **A13 confirmed** (skb) |
