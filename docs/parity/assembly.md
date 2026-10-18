# Assembly tab: Onshape behaviour parity checklist

One line per behaviour. Status: ✅ matches Onshape (verified in the running app and/or a model
test), 🔧 partly there or differs (note says how), ❌ missing, ⏳ implemented, not yet verified.

Sources: `H:<page>` = Onshape help page `docs-raw/md/<page>.md` (`Assembly__` prefix dropped);
`E§n` = `research/sections/E-assembly.md` section; `AS-nn` = `research/sections/04-assemblies.md`;
`B` = `docs-raw/bundle-i18n-strings.txt`; `UI:` = screenshot in `/root/onshape-research/ui-ref/`.
Model tests are in `crates/model/src/assembly/tests.rs` (named in the notes).

Behaviour code: `crates/app/src/assembly_view.rs` (view, events), `assembly_behaviors.rs` (menus,
visibility, rename, filter, folders, subassemblies, display states, animate, shortcut toolbar),
`assembly_panels.rs` (dialogs, triad, snap), `assembly_mate_dialog.rs`; model:
`crates/model/src/assembly/{mod,eval,ops,solver}.rs`.

## 1. Keyboard shortcuts (H:Home__keyboard_shortcuts_and_hotkeys, E§3)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| K1 | `i` opens / closes Insert parts and assemblies | H:shortcuts, E§3 | ✅ | toggles; closing commits a pending placement |
| K2 | `m` opens the Fastened mate dialog | H:shortcuts | ✅ | |
| K3 | `j` shows / hides all mates | H:shortcuts | ✅ | per-mate eye overrides (Mate.hidden) |
| K4 | `k` shows / hides mate connectors | H:shortcuts | ✅ | |
| K5 | `h` toggles Show mates mode | H:shortcuts | ✅ | also toolbar button |
| K6 | `Shift+S` toggles Snap mode | H:shortcuts | ✅ | |
| K7 | `Ctrl+M` opens the Mate connector dialog | H:shortcuts | ✅ | bound in Workspace; Assembly handles MateConnectorTool |
| K8 | `a` flips the primary axis (mate / mate connector dialog, snap drag) | H:shortcuts, E§20 | ✅ | dialog and snap drag |
| K9 | `q` reorients the secondary axis during a snap drag | E§20 | ✅ | snap drag (verified); mate / MC dialog |
| K10 | `Shift` (hold) locks mate inference to the hovered face/edge | H:mates, E§6.3 | ✅ | nearest point of the locked entity; click anywhere picks it |
| K11 | `Ctrl` (tap) cycles the dragged connector in snap mode | E§20 | ⏳ | pre-existing |
| K12 | `Enter` commits the dialog | H:shortcuts | ✅ | mate dialog |
| K13 | `Shift+Enter` commits and starts another of the same command | H:shortcuts | ✅ | mate dialog: commits and reopens the same mate type |
| K14 | `Esc` cancels the command / closes menus, exits isolate | H:shortcuts | ✅ | Insert: Esc closes and keeps placed instances |
| K15 | `Space` clears the selection | H:shortcuts | ✅ | |
| K16 | `Delete` / `Backspace` deletes the selection (instances or mate feature) | H:shortcuts | ✅ | |
| K17 | `Shift+N` renames the selected instance or feature inline | H:shortcuts | ✅ | |
| K18 | `y` hides the hovered or selected instances | H:shortcuts | ✅ | hover in graphics or list |
| K19 | `Shift+Y` shows hidden instances | H:shortcuts | ✅ | |
| K20 | `Shift+I` isolates hovered/selected (repeated presses add) | H:shortcuts, H:View__isolate | ✅ | |
| K21 | `Shift+T` makes hovered/selected transparent (cumulative) | H:shortcuts | ✅ | transparent instances are not pickable |
| K22 | `Ctrl+C` / `Ctrl+V` copy / paste instances (mates and MCs among them come along) | E§24.7, AS-05 | ✅ | model test `copy_paste_duplicates_instances_with_their_mates`; paste follows the cursor, click places |
| K23 | `s` opens the shortcut toolbar at the cursor | E§2, B `OPEN_MINI_TOOLBAR` | ✅ | assembly tool set, kept inside the graphics area |
| K24 | `Shift+/` keyboard shortcut list | H:shortcuts | ✅ | assembly list overlay |
| K25 | `Alt+C` search tools | E§2 | 🔧 | workspace search; assembly commands not listed |
| K26 | `[` Measure, `Shift+X` section view | E§3 | ✅ | Measure: click faces/edges/vertices of any instances (length, area, radius, position, distance ΔXYZ, angle; model test `measure_between_instances`); Section view through the hovered planar face, the selected mate connector or Front, offset and flip |
| K27 | `` ` `` select other | E§3 | ✅ | cycles the instances under the cursor (Shift+` backwards), list at the cursor |
| K28 | `Alt+click` select through transparent | H:shortcuts | 🔧 | transparent instances are never pickable |
| K29 | View keys (`f`, `Shift+1..7`, `z`, arrows) | H:shortcuts | ✅ | shared viewport bindings |
| K30 | Alt-drag ignoring mates | task brief | n/a | not an Onshape behaviour (not documented) |

## 2. Toolbar (E§2.1, UI:toolbars/docs-assembly-toolbar.png)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| T1 | Insert, Replicate, Linear/Circular pattern, Mirror tools | E§2.1 | ✅ | |
| T2 | Mate connector tool | E§2.1 | ✅ | |
| T3 | Mate tools Fastened … Parallel, Tangent, Width | E§2.1 | ✅ | |
| T4 | Group, Relations dropdown | E§2.1 | ✅ | Group is immediate (no dialog), see G1 |
| T5 | Replace instance tool | E§2.1 | ✅ | replaces the selected instance |
| T6 | Display states tool | E§2.1 | ✅ | |
| T7 | Snap mode toggle (highlighted when on) | E§2.1 | ✅ | |
| T8 | Show mates mode toggle | E§2.1 | ✅ | |
| T9 | Create Part Studio in context | E§2.1 | ❌ | in-context modelling not implemented |
| T10 | Update all references | E§2.1 | 🔧 | per-instance "update" badge for other documents |
| T11 | Toolbar order exactly as Onshape | UI:toolbars | 🔧 | owned by the theme agent |
| T12 | Customize toolbar | E§2 | ❌ | |

## 3. Insert parts and assemblies (H:insert_parts_and_assemblies, E§4)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| I1 | Source tabs Current document / Other documents | E§4.1 | ✅ | |
| I2 | Element tabs Part Studios / Assemblies | E§4.1 | ✅ | |
| I3 | Search by partial name | E§4.1 | ✅ | |
| I4 | Insert entire Part Studio as rigid | E§4.1 | ✅ | |
| I5 | Click a part inserts it; click a Part Studio inserts all its parts | E§4.2 | ✅ | |
| I6 | Click an assembly inserts it as a subassembly (cycles prevented) | E§4.2 | ✅ | |
| I7 | Cursor in the dialog: instance at the Part Studio origin; later copies offset | E§4.2 | ✅ | |
| I8 | Cursor in the graphics area: instance follows the cursor; click places | E§4.2 | ✅ | |
| I9 | Esc / close: insertion ends, placed instances stay (Undo removes) | E§4.2, B | ✅ | |
| I10 | Placed instances are free (not fixed, not mated) | E§4.2 | ✅ | |
| I11 | Other documents at a version, newer-version notice | E§4.1 | ✅ | pre-existing |
| I12 | Filters (sketches, surfaces, parts, composite) | E§4.1 | ❌ | |
| I13 | Configuration inputs before inserting | E§4.1 | 🔧 | list and checkbox inputs under the Part Studio, Reset to default; variable inputs show their default; no Generate (part list uses the default configuration) |
| I14 | Snap-insert with explicit MCs (Fastened) | E§4.3 | ⏳ | snap mode on: the inserted instance's Part Studio mate connector lands on the connector under the cursor, Ctrl cycles it, the click adds a Fastened mate |
| I15 | Standard content tab | E§4.7 | ❌ | |
| I16 | Inserted count, thumbnails | E§4.1 | ❌ | |

## 4. Instance list (H:instances_list, E§5, UI:context-menus/assembly__context-*.png)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| L1 | Top-level assembly row; click selects the whole assembly | E§5.1 | ✅ | |
| L2 | Origin row (pickable as a mate target) | E§5.1 | ✅ | |
| L3 | "Instances (n)" header, DOF count | B | ✅ | |
| L4 | Filter box: name substring | E§5.2 | ✅ | |
| L5 | Filter colon commands `:part :assembly :mate :matelimits :connector :errors :suppressed :hidden :shown` (combinable) | E§5.2 | ✅ | model test `list_filter_colon_commands`; dropdown lists them |
| L6 | Filter `:item :folder :warnings` | E§5.2 | 🔧 | parsed; no items / warnings exist |
| L7 | Eye icon on row hover toggles visibility; stays shown while hidden | E§5.1 | ✅ | |
| L8 | Hovering a row highlights the instance in the graphics | E§24.4 | ✅ | |
| L9 | Selecting in the graphics scrolls the list to the row | E§5.1 | ✅ | |
| L10 | Shift/Ctrl click adds to the selection | — | ✅ | |
| L11 | Fixed pin icon ("Part is fixed"), DOF indicator ("Has degrees of freedom") | E§5.3, B | ✅ | |
| L12 | Suppressed rows struck through, hidden rows dimmed | E§5.3 | ✅ | |
| L13 | Error indicator with message tooltip | E§5.3 | ✅ | |
| L14 | Subassemblies expand to their instances | E§5.1 | ✅ | |
| L15 | Inline rename (Shift+N, Rename) | E§5.4 | ✅ | |
| L16 | Folders: Add selection to folder (named inline), count `(n)` | E§5.6 | ✅ | model test `folders_and_display_states` |
| L17 | Folder menu: Rename, Hide/Show, Hide other, Hide all, Isolate, Make transparent, Suppress, Add selection to folder, Expand, Collapse, Delete (with contents), Unpack folder | E§5.6, UI:concepts__featurelist-contextmenu | ✅ | Unpack / Delete model-tested |
| L18 | Folder eye hides / shows all members | E§5.6 | ⏳ | |
| L19 | New folder icon in the header | E§5.6 | ✅ | header folder button (around the selection) |
| L20 | Drag and drop to reorder / into folders / into subassemblies | E§5.5 | ✅ | dropping onto a subassembly row moves the dragged (or selected) instances into it at the same place; model test `drop_instances_into_a_subassembly` |
| L21 | Instance list properties columns | E§5.1 | ❌ | |
| L22 | Items list (non-geometric BOM items) | E§5.1 | ❌ | |
| L23 | Dynamic suppression | E§5.7 | ❌ | |
| L24 | Rigid / lock icons on subassemblies | E§5.3 | 🔧 | lock icon done; no rigid icon |
| L25 | Warnings (more than one part fixed …) | E§5.3, B | ❌ | |

### 4.1 Instance context menu (part / subassembly), Onshape order

| # | Entry | Source | Status | Notes |
|---|---|---|---|---|
| C1 | Exit isolate (top, while isolating) | H:mates | ✅ | |
| C2 | Hide / Show | UI:context-part | ✅ | |
| C3 | Hide other instances | UI:context-part | ✅ | |
| C4 | Hide all instances | UI:context-part | ✅ | |
| C5 | Isolate… | UI:context-part | ✅ | |
| C6 | Make transparent… | UI:context-part | ✅ | |
| C7 | Suppress / Unsuppress | UI:context-part | ✅ | |
| C8 | Fix / Unfix | UI:context-part | ✅ | |
| C9 | Check interference… (subassemblies, multi-selection) | UI:context-subassembly | ✅ | |
| C10 | Add mate connector to instance origin… | UI:context-part | ✅ | single selection |
| C11 | Replace instances… / Replace n instances… | UI:context-part, B | ✅ | |
| C12 | Switch to Part Studio / Switch to Assembly | UI:context-part | ✅ | opens the tab |
| C13 | Change configuration… | UI:context-part | ✅ | |
| C14 | Select instances with same part and same configuration | UI:context-part | ✅ | |
| C15 | Copy <name> / Copy n items | UI:context-part, B | ✅ | |
| C16 | Move to new subassembly | UI:context-part | ✅ | model test `move_to_new_subassembly_and_dissolve_keep_positions` |
| C17 | Dissolve subassembly | UI:context-subassembly | ✅ | tab left empty when unused elsewhere |
| C18 | Zoom to selection | UI:context-part | ✅ | |
| C19 | Add selection to folder… | UI:context-part | ✅ | |
| C20 | Expand ▸ Expand selected / Expand all; Collapse ▸ … | UI:context-part | ⏳ | subassemblies |
| C21 | Create empty subassembly | UI:context-part | ✅ | new "Assembly N" tab + instance |
| C22 | Delete (mates, MCs, relations go; groups keep other members) | UI:context-part, E§1 | ✅ | model test `rename_hide_others_and_unfix_all` |
| C23 | Show mates, Go to item in list (graphics menu) | E§5.4 | ✅ | |
| C24 | Graphics menu on empty space: Show all, Zoom to fit, Isometric, Clear selection | E§5.4 | ✅ | |
| C25 | Properties…, Instance properties…, Show center of mass, Use best available tessellation, Edit in context, Update context, Change to version, Export, Where used, Revision history, Create task, Add comment, Create Drawing, Add to named positions, Lock/follow position to, Select instances with same configuration | UI:context-part | ❌ | not implemented in noshape (in-context, PDM, comments) |

### 4.2 Top-level assembly menu (UI:context-assembly)

| # | Entry | Source | Status | Notes |
|---|---|---|---|---|
| A1 | Rename (the tab) | UI:context-assembly | ✅ | |
| A2 | Check interference… | UI:context-assembly | ✅ | |
| A3 | Add exploded view… | UI:context-assembly | ✅ | |
| A4 | Unfix all (when something is fixed) | E§5.4, AS-06 | ✅ | model test |
| A5 | Expand all / Collapse all | UI:context-assembly | ✅ | |
| A6 | Properties…, Hide instance list properties, Show center of mass, Create Drawing, Export, Where used, Release, Revision history, Create task, Add comment | UI:context-assembly | ❌ | |

## 5. Mate features list (H:mates, E§5.1, E§6.5, UI:assembly__mate-hover.png)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| F1 | Mate connectors, mates, relations, groups listed in creation order | E§5.1 | ✅ | |
| F2 | Hover a mate: its instances highlight, glyph shows | E§5.1 | ✅ | |
| F3 | Hover tooltip: "<name> has error: <message>", type, offset and limit definitions | UI:mate-hover | ✅ | |
| F4 | Mate limit indicator icon ("<name> has limits") | H:mates, B | ✅ | |
| F5 | Eye icon on hover toggles the mate indicator | E§6.4 | ✅ | |
| F6 | Mates expand to the implicit mate connectors they use | E§5.1, UI:mate-hover | ✅ | |
| F7 | Double-click edits | H:mates | ✅ | |
| F8 | Error rows red with indicator | E§6.4 | ✅ | |
| F9 | Suppressed / inactive rows grey, struck through | E§6.4 | ✅ | |
| F10 | Mate folders (Add selection to folder, folder menu) | E§5.6 | ✅ | |
| F11 | Drag to reorder | H:mates | ❌ | |
| F12 | Selecting a glyph in the graphics selects the mate | E§6.4 | ✅ | mate indicators are pickable: click selects the mate, right-click opens its menu; blue OK / red error / grey inactive |

### 5.1 Mate context menu (H:mates "Mate context menu")

| # | Entry | Source | Status | Notes |
|---|---|---|---|---|
| M1 | Rename | H:mates | ✅ | inline |
| M2 | Edit… (works when suppressed) | H:mates | ✅ | |
| M3 | Apply limit position ▸ Min/Max per DOF ("Moved mate to max Z limit") | H:mates, B | ✅ | model test `mate_reset_and_apply_limit_position`; other DOFs stay free |
| M4 | Reset (home position) | H:mates | ✅ | model test |
| M5 | Animate <mate>… | H:mates, B | ✅ | see §11 |
| M6 | Hide / Show | H:mates | ✅ | |
| M7 | Show all mates | H:mates | ✅ | |
| M8 | Isolate…, Make transparent… (instances of the mate) | H:mates | ✅ | |
| M9 | Suppress / Unsuppress | H:mates | ✅ | |
| M10 | Add selection to folder… | E§6.5 | ✅ | |
| M11 | Clear selection | H:mates | ✅ | |
| M12 | Delete | H:mates | ✅ | |
| M13 | Dynamic suppression, Edit named position driver mate, Configure | E§6.5 | ❌ | |
| M14 | Mate connector menu: Rename, Edit, Hide, Hide other mate connectors, Show all mate connectors, Suppress, Delete | E§6.5 | ✅ | |

## 6. Mate dialog (H:mates "Mate dialog", E§6.2)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| D1 | Mate type selector changes the type of the mate in place | E§6.2 | ✅ | default names follow ("Fastened 1" → "Revolute 1") |
| D2 | Two mate connector fields; picks fill the first, then auto-advance to the second | E§6.2 | ✅ | |
| D3 | Clicking a field makes it active; × clears it | E§6.2 | ✅ | |
| D4 | Second pick moves the first connector's instance onto the second (second stays) | E§6.3 | ✅ | |
| D5 | Connectors must be on different instances | E§6.1 | ✅ | |
| D6 | Explicit mate connectors and the Origin pickable (graphics or list) | E§6.1 | ✅ | |
| D7 | Flip primary axis (button, `a`); bottom connector flips when the top one is constrained | H:mates | ✅ | stationary side is the held one |
| D8 | Reorient secondary axis (90° steps) | H:mates | ✅ | |
| D9 | Reorder items link → handles to swap → Done | H:mates | ✅ | |
| D10 | Offset checkbox; offset axes per type (Fastened XYZ, Slider XY, Revolute/Planar/Pin slot Z) | E§6.2, E§7 | ✅ | |
| D11 | Rotation offset with axis selector (Rotate about X / Y / Z) | H:mates | ✅ | model test `mate_rotation_offset_about_any_axis` |
| D12 | Limits checkbox; per-DOF min / max fields (blank = unbounded) | E§6.2 | ✅ | |
| D13 | Ball mate cone angle limit | E§7.8 | ✅ | model test `ball_mate_cone_limit`; dialog shows one Maximum cone angle field |
| D14 | Solve button (solve all mates incl. this one) | H:mates | ✅ | |
| D15 | Heads-up checkmark at the cursor after both picks: "Commit current mate and start a new mate" | H:mates, B | ✅ | |
| D16 | Animate (preview) button in the dialog | H:mates | 🔧 | Animate button previews the first DOF back and forth; other mates stay enforced |
| D17 | Edit implicit / assembly MCs from the dialog (MC icon) | H:mates | 🔧 | opens the Mate connector dialog with the mate dialog kept; an implicit connector becomes an assembly mate connector (Onshape keeps it implicit) |
| D18 | Only the current mate's instances move until Solve | H:mates | 🔧 | the stationary instance is held; other mates are enforced |
| D19 | Tangent mate: entities, Tangent propagation, flip | E§7.10 | ✅ | pre-existing |
| D20 | Width mate: tab and width connector fields | E§7.11 | ✅ | pre-existing |
| D21 | Over-constrained message in the dialog, red mate | E§24.2 | ✅ | |

## 7. Mate connector inference (H:mates, H:assembly_mate_connector, E§6.3, E§8.2)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| N1 | Hovering shows inference dots; the nearest shows a full triad | E§6.3 | ✅ | |
| N2 | Planar face: centroid, every vertex, edge midpoints, arc/circle centers, inner loop (hole, slot) centers; Z normal to the face | E§8.2 | ✅ | model test `inference_points_match_onshape` |
| N3 | Cylindrical face: axis points at both ends and the middle, Z along the axis | E§8.2 | ✅ | model test |
| N4 | Conical face: top, middle, bottom and virtual sharp | E§8.2 | ⏳ | |
| N5 | Spherical face: center | E§8.2 | ⏳ | |
| N6 | Circular edge: center only | H:mates | ✅ | model test |
| N7 | Linear edge: midpoint and both ends, Z along the edge | E§8.2 | ✅ | model test |
| N8 | Arc / curve edge: center, midpoint and ends | E§8.2 | ⏳ | |
| N9 | Vertex: the point | E§8.2 | 🔧 | Z is global Z (Onshape: normal to a face) |
| N10 | No seam vertices of circles offered | — | ✅ | model test |
| N11 | Shift locks the entity; the connector snaps to its nearest point | E§6.3 | ✅ | |
| N12 | Hovered entity highlighted | E§6.3 | ✅ | |
| N13 | Sketch entities / regions as targets | E§8.2 | 🔧 | sketch curves get center/midpoint |

## 8. Mate connector dialog (E§8)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| X1 | Origin entity pick with inference | E§8.1 | ✅ | |
| X2 | Flip primary, reorient secondary | E§8.1 | ✅ | |
| X3 | Move: X/Y/Z translation, rotation axis + angle | E§8.1 | ✅ | |
| X4 | Origin type Between entities, Realign | E§8.1 | ❌ | |
| X5 | Add mate connector to instance origin (prefilled) | E§5.4 | ✅ | |

## 9. Dragging, solving, triad (H:Home__triad_manipulator, E§24)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| R1 | Free drag moves an instance within its remaining DOF; limits stop it | E§24.1 | ✅ | model test `slider_respects_limits` |
| R2 | Unmated, unfixed instances move exactly | H:triad | ✅ | |
| R3 | Fixed instances cannot be dragged (cue, no manipulator) | H:triad | 🔧 | no manipulator; message after the drag instead of a cursor cue |
| R4 | Click an instance shows the triad: axis arrows, plane squares, rotation arcs | H:triad | ✅ | |
| R5 | Drag the triad center to reposition it (snaps to MCs / inference points) | H:triad | ✅ | |
| R6 | Typed value field after a triad drag | H:triad | ✅ | |
| R7 | Right-click center: Move to origin | H:triad | ✅ | |
| R8 | Right-click axis arrow: Align with Z, Anti-align with Z | H:triad | ✅ | |
| R9 | Right-click arc: Rotate 90 degrees, Rotate 180 degrees | H:triad | ✅ | |
| R10 | Box selection selects instances (Shift/Ctrl adds) | — | ✅ | |
| R11 | Live mate values while dragging (and on the selected mate); double-click a value to type one | H:mates | ✅ | labels Rz/X/Y/Z at the mate; typed value moves the parts with the other mates enforced |
| R12 | Mates solved simultaneously; red when unsolvable | E§24.2 | ✅ | |
| R13 | Rigid detection icon (0 DOF + fixed) | E§24.2 | ❌ | |

## 10. Snap mode (H:snap_mode, E§20)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| S1 | Drag an instance: it goes transparent, MCs of others wake near the cursor | E§20 | ✅ | pre-existing |
| S2 | Release opens the mate dialog (Fastened) with both MCs filled | E§20 | ✅ | pre-existing |
| S3 | Ctrl cycles the dragged instance's MCs | E§20 | ⏳ | pre-existing; Ctrl detected through modifier events |
| S4 | `a` / `q` change alignment during the drag | E§20 | ✅ | |
| S5 | Fixed instances cannot be dragged | E§20 | ✅ | |

## 11. Animate (H:mates "Animating movement", E§6.6)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| AN1 | Animate from the mate context menu; all other mates enforced | E§6.6 | ✅ | |
| AN2 | DOF selector for mates with several DOF | E§6.6, B | ✅ | model test `hold_one_degree_of_freedom` |
| AN3 | Start / End default to the limits, else 0 → 360° | E§6.6 | ✅ | |
| AN4 | Steps (more steps = slower, about 60 per second) | E§6.6 | ✅ | |
| AN5 | Playback Single / Reciprocate / Loop | E§6.6 | ✅ | |
| AN6 | Reverse | E§6.6 | ✅ | |
| AN7 | Current value (read-only, live) | E§6.6 | ✅ | |
| AN8 | Play / Stop (stays at the stop point) | E§6.6 | ✅ | closing restores the document positions |

## 12. Other assembly features

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| G1 | Group: dialog with Instances field | E§10 | ✅ | Group dialog: preselection, pick instances in the graphics or list, Edit on the group |
| G2 | Group moves rigidly; deleting a member keeps the group | E§10, E§1 | ✅ | model tests |
| G3 | Relations: Gear, Rack and pinion, Screw, Linear | E§9 | ✅ | mates picked in the list or by their indicators; new relations start empty |
| G4 | Replicate, Linear / Circular pattern, Mirror | E§11–13 | ✅ | pre-existing |
| G5 | Exploded views (steps, rollback, lines) | E§16 | ✅ | pre-existing |
| G6 | Named positions | E§17 | ✅ | pre-existing |
| G7 | Interference detection | E§24 | ✅ | pre-existing |
| G8 | BOM (flattened) | E§23 | 🔧 | no structured BOM, templates or editing |
| G9 | Display states: add, select to apply, update, rename, remove; new items keep defaults | E§15, B | ✅ | model test `folders_and_display_states` |
| G10 | Replace instance (matches geometry) | E§14 | ✅ | pre-existing |
| G11 | Subassemblies: Move to new, Dissolve, Create empty | E§5.5 | ✅ | |
| G12 | Flexible subassemblies (default): their mates solve with the parent, their parts drag on their own; Lock/follow position to ▸ Lock current position / named position / Unlock; lock icon (click unlocks) | E§5.5, E§24.2 | ✅ | model test `flexible_subassembly_moves_in_parent_and_locks`; subassemblies with patterns, relations, tangent mates or nested flexible subassemblies stay rigid |
| G13 | Modeling in context, Create Part Studio in context | E§18 | ❌ | |
| G14 | Standard content | E§4.7 | ❌ | |
| G15 | Show mates mode: hover shows an instance's mates, click pins | E§21 | ✅ | |
| G16 | Isolate / Make transparent panel with Exit isolate and transparency steps | E§24.6 | 🔧 | transparency −/+ steps; no expand by distance / connectivity |
| G17 | Assembly configurations and variable table | E§22 | ❌ | |
