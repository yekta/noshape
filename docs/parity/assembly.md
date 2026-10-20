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
| K11 | `Ctrl` (tap) cycles the dragged connector in snap mode | E§20 | ✅ | Ctrl tap cycles the dragged connector only when the instance has more than the picked one (its explicit connectors); live 570 (a block with no explicit connectors): Ctrl taps don't change the face-centre connector, and ours doesn't either. No message while snap-dragging (live 569) |
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
| K25 | `Alt+C` search tools | E§2 | ✅ | Alt+C / "Search tools…" in the assembly toolbar: the tool search popover with the assembly tools, live ranking (starts, contains, letters in order; shorter names first; max 10), live descriptions, Highlight tool in toolbar (`assembly_search.rs`, unit test) |
| K26 | `[` Measure, `Shift+X` section view | E§3 | ✅ | Measure: click faces/edges/vertices of any instances (length, area, radius, position, distance ΔXYZ, angle; model test `measure_between_instances`); Section view through the hovered planar face, the selected mate connector or Front, offset and flip |
| K27 | `` ` `` select other | E§3 | ✅ | cycles the instances under the cursor (Shift+` backwards), list at the cursor |
| K28 | `Alt+click` select through transparent | H:shortcuts | ✅ | Alt held (or Isolate › Select transparent geometry): transparent / isolated-away instances become pickable (`AsmSceneOptions.pick_dimmed`); the isolate enter / exit / expand and pickability toggles now redraw the pick buffer (rebuild_scene(true)); GUI k28e (checkbox path; Alt held via xdotool is unreliable) |
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
| T9 | Create Part Studio in context | E§2.1 | ✅ | toolbar button and Origin row › Create Part Studio in context… ("Origin of new Part Studio"), new Part Studio opened in context with a persisted Assembly context snapshot (see G13) GUI p1–p3 |
| T10 | Update all references | E§2.1 | ✅ | toolbar "Update all references to latest versions" is enabled (blue) only while another document referenced here has a newer version (live 607, 621), else disabled; rows: ⋮ "Versioned part from current document" / "…from other document", blue badge "Versioned part from other document with newer version" (live 617, 620); the non-Onshape "update" pill and "Update to Vx" menu item are gone. Fixture: test `t10_fixture_reports_the_newer_version_of_a_referenced_document` (`NOSHAPE_T10_DIR`); GUI :103 t1–t9: enabled button, blue badge, Reference manager row with thumbnail "T10 source / V1 ⇨ V2", Update all → "Versioned part from other document" icon + tooltip, "Newer versions available for 0 documents", button faded to 0.4 when disabled (live 607 vs 621) |
| T11 | Toolbar order exactly as Onshape | UI:toolbars | 🔧 | owned by the theme agent |
| T12 | Customize toolbar | E§2 | ✅ | toolbar right-click: Customize toolbar… / Hide (Show) "Search tools"; customize mode: #f5f5f5 tool sets with ••• sleds (drag to move a set), drag a tool onto the 640 px Tools panel (red dashed drop zone) to remove it and back to re-add, Show items already in toolbar (3 columns, present tools greyed), Save / Reset to default (disabled until changed) / Cancel; saved in assembly_toolbar.json (`assembly_toolbar.rs`). GUI :103 h1–h5 vs live 340–346 |

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
| I12 | Filters (sketches, surfaces, parts, composite) | E§4.1 | ✅ | Parts / Composite parts (off) / Sketches (off) / Surfaces filter toggles (live 320); composite parts first under their Part Studio with the composite icon, one instance row with the composite icon (live 326); sketches inserted as Sketch 1 <1> with the sketch icon. Note: the Part Studio model always consumes a composite's members (Onshape keeps them unless Closed): Part Studio area GUI :103 i1–i7: three toggles (Parts on, Composite parts off, Sketches off) + rigid at the right as live 320; sketches listed under the parts with thumbnails; a composite hidden until its toggle is on, then inserted as one "Composite part 1 <1>" row (composite icon, DOF glyph, no chevron). |
| I13 | Configuration inputs before inserting | E§4.1 | ⏳ | live 592–595: a configured row gets the 57x30 split button "Set to last viewed configuration" (configuration icon) with ▾ Set to last viewed / Reset to default ("Could not find last viewed configuration." toast when the Part Studio was never shown in a configuration); the inputs ("Size [Default ▾]", checkboxes) show when the row is expanded, above the parts; no Generate (live: none) |
| I14 | Snap-insert with explicit MCs (Fastened) | E§4.3 | ✅ | snap mode on: the inserted instance's Part Studio mate connector lands on the connector under the cursor, Ctrl cycles it, the click adds a Fastened mate. GUI :103 k6–k9: a Mate connector on the Pin's bottom circle, snap mode on, Insert › Pin › Part 1: the instance's connector lands on the hole's centre connector under the cursor; the click adds "Fastened 2" |
| I15 | Standard content tab: Standard › Category › Class › Component, Size (+ auto-size), Length, preview, Insert | E§4.7, H:standard_content | ✅ | `model/assembly/standard.rs` (ANSI/DIN/ISO bolts, screws, nuts, washers, pins; parts generated by a built-in Feature Studio script with explicit mate connectors); `app/assembly_standard.rs`; model tests `standard_content_*` |
| I16 | Inserted count, thumbnails | E§4.1 | ✅ | "Inserted: N" counts the instance following the cursor, as live 122 ("Inserted: 1" while the sketch follows the cursor); part / Part Studio / sketch thumbnails rendered offscreen (assembly_thumbs.rs). GUI :103 k5, i2. Visual for ui2: live has a `?` at the right of the "Undo to remove instances" footer |

## 4. Instance list (H:instances_list, E§5, UI:context-menus/assembly__context-*.png)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| L1 | Top-level assembly row; click selects the whole assembly | E§5.1 | ✅ | |
| L2 | Origin row (pickable as a mate target) | E§5.1 | ✅ | |
| L3 | "Instances (n)" header, DOF count | B | ✅ | |
| L4 | Filter box: name substring | E§5.2 | ✅ | |
| L5 | Filter colon commands `:part :assembly :mate :matelimits :connector :errors :suppressed :hidden :shown` (combinable) | E§5.2 | ✅ | model test `list_filter_colon_commands`; dropdown lists them |
| L6 | Filter `:item :folder :warnings` | E§5.2 | ✅ | `:warnings` matches instances with an instance warning (More than one part is fixed, …); `:folder` lists the folders (flat, like every filtered row; a folder also matches by name); `:item` lists non-geometric items, which this account cannot create (live 105: no "Add items" flow), so it is always empty. GUI :103 w1 (two fixed parts → both listed with ⚠), w4 (Folder 1) |
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
| L17 | Folder menu: Rename, Hide/Show, Hide other, Hide all, Isolate, Make transparent, Suppress, Add selection to folder, Expand, Collapse, Delete (with contents), Unpack folder | E§5.6, UI:concepts__featurelist-contextmenu | ⏳ | live 537: Rename | Hide, Hide other instances, Hide all instances, Isolate…, Make transparent…, Suppress, Dynamic suppression ▸ (folder expression: `ListFolder.unsuppressed_if`, model test `dynamic_suppression_of_a_folder_suppresses_its_instances`), Unfix all (when it holds fixed instances) | Add selection to folder…, Expand ▸, Collapse ▸ | Delete, Unpack folder |
| L18 | Folder eye hides / shows all members | E§5.6 | ✅ | GUI f3–f4 |
| L19 | New folder icon in the header | E§5.6 | ✅ | header folder button (around the selection) |
| L20 | Drag and drop to reorder / into folders / into subassemblies | E§5.5 | ✅ | dropping onto a subassembly row moves the dragged (or selected) instances into it at the same place; model test `drop_instances_into_a_subassembly` |
| L21 | Instance list properties columns | E§5.1 | ✅ | Top row › Show / Hide instance list properties: gray release-state pill ("In progress") after each instance name (live 103: no columns) (skb) GUI l21c |
| L22 | Items list (non-geometric BOM items) | E§5.1 | n/a | not on the captured plan: live 105 / 678 found no non-geometric item flow (no "Add items" control in the BOM header, ⋯ or rows); a subscription-gated feature (TEAM-RULES exclusion) |
| L23 | Dynamic suppression | E§5.7 | ✅ | Instance row › Dynamic suppression ▸ Add expression…: 220 px "Dynamic suppression" dialog, "Unsuppressed if" expression; evaluated with the assembly variables, false → suppressed (`eval::dynamically_suppressed`, model test); menu with an expression set: capture requested |
| L24 | Rigid / lock icons on subassemblies | E§5.3 | ✅ | lock icon; rigid icon when the subassembly has 0 DOF and exactly one fixed instance (`InstanceNode.rigid`); GUI g1 (Move to new subassembly of a fixed + fastened pair) |
| L25 | Warnings (more than one part fixed …) | E§5.3, B | ✅ | warning icon + "<name> has warning: …": More than one part is fixed / More than one part inside is fixed / Fastened to origin even though Assembly has fixed part(s) |

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
| C20 | Expand ▸ Expand selected / Expand all; Collapse ▸ … | UI:context-part | ✅ | subassemblies; GUI g2–g4 |
| C21 | Create empty subassembly | UI:context-part | ✅ | new "Assembly N" tab + instance |
| C22 | Delete (mates, MCs, relations go; groups keep other members) | UI:context-part, E§1 | ✅ | model test `rename_hide_others_and_unfix_all` |
| C23 | Show mates, Go to item in list (graphics menu) | E§5.4 | ✅ | |
| C24 | Graphics menu on empty space: Show all, Zoom to fit, Isometric, Clear selection | E§5.4 | ✅ | |
| C25 | Properties…, Instance properties…, Show center of mass, Use best available tessellation, Edit in context, Update context, Change to version, Export, Where used, Revision history, Create task, Add comment, Create Drawing, Add to named positions, Lock/follow position to, Select instances with same configuration | UI:context-part | 🔧 | all of the live list implemented (see LA13 and G13: Edit in context ▸, Update context ▸, Set as primary instance ▸); Where used / Revision history / Create task / Add comment are cloud-only or not in the live menu |

### 4.2 Top-level assembly menu (UI:context-assembly)

| # | Entry | Source | Status | Notes |
|---|---|---|---|---|
| A1 | Rename (the tab) | UI:context-assembly | ✅ | |
| A2 | Check interference… | UI:context-assembly | ✅ | |
| A3 | Add exploded view… | UI:context-assembly | ✅ | |
| A4 | Unfix all (when something is fixed) | E§5.4, AS-06 | ✅ | top row: Unfix all right after Show center of mass once an instance at this level is fixed (not for fixed parts inside a subassembly; live 530–538); folder rows too (L17); multi-select of fixed instances: Unfix GUI :103 a1 = live 532 |
| A5 | Expand all / Collapse all | UI:context-assembly | ✅ | |
| A6 | Properties…, Hide instance list properties, Show center of mass, Create Drawing, Export, Where used, Release, Revision history, Create task, Add comment | UI:context-assembly | 🔧 | Properties…, Show / Hide instance list properties, Show center of mass, Create Drawing…, Export…, Check interference…, Add exploded view… implemented; Release… is a paid-plan upsell (excluded), Add comment cloud-only |

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
| F11 | Drag to reorder | H:mates | ✅ | drop a mate (connector, relation, group) row on another of the same kind; model test `reorder_mate_features` |
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
| M13 | Dynamic suppression, Edit named position driver mate, Configure | E§6.5 | ✅ | Dynamic suppression ▸ (mates and instances: "Unsuppressed if"; once set: Unsuppress + Edit dynamic suppression), Edit named position driver mate… (Driving mate selector, editing banner, value columns); Configure: mate parameters are configured from the Configuration panel's mode (G17) |
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
| D16 | Animate (preview) button in the dialog | H:mates | ✅ | ▶ (`play-dialog`, "Animate mate degree of freedom", live 575): the mate's first DOF runs once from its current value through 2 full turns (a span for sliders) with the value arc and whole-degree label ("208°"), then back to the start pose (~12 s: 720 steps at 60/s; live ≈45 s in a slow browser, "probably a fixed frame count"); ▶ never becomes a stop button. GUI :103 d4–d7, v6–v7 |
| D17 | Edit implicit / assembly MCs from the dialog (MC icon) | H:mates | ✅ | the chip's `mate-connector-qlv-item` icon opens the Mate connector dialog docked right of the mate dialog (live 578, dom/assembly-mate-chip-connector-dialog.txt): close button only for an implicit connector, Origin type ▾ On entity, active "Origin entity" field naming the entity ("Face of Part 1 <1>"), ☐ Realign, ☐ Move (X/Y/Z translation, Rotate about X/Y/Z ▾, Rotation angle), flip / reorient icons, `?`; edits go into the mate's own reference, which stays implicit (`MateConnectorRef.moved`, `ConnectorMove.off`; model test `implicit_connector_move_offsets_the_mate`). GUI :103 m4–m5; standalone dialog: Onshape's preview slider (112 px, rests at 0.7, fades the connector glyph to the left) with `?` on its row |
| D18 | Only the current mate's instances move until Solve | H:mates | ✅ | help (Mates): "Onshape does not continually calculate all mates and their positions until the Mate feature is accepted. Use Solve". `SolveOptions::only_mate`: before Solve only the mate being defined is solved (the second connector's instance held, live 33–35: the first-picked part moves); other mates and relations are not enforced and report ok; Solve / ✓ solve everything. T:`a_mate_being_defined_is_solved_alone_until_solve`; GUI :103 d1 (Part 1 <2> mated onto <1>: the revolute-mated Pin stays put), d2 (Solve: the Pin follows, and Fastened 1 conflicts as expected) |
| D19 | Tangent mate: entities, Tangent propagation, flip | E§7.10 | ✅ | pre-existing |
| D20 | Width mate: tab and width connector fields | E§7.11 | ✅ | pre-existing |
| D21 | Over-constrained message in the dialog, red mate | E§24.2 | ✅ | |

## 7. Mate connector inference (H:mates, H:assembly_mate_connector, E§6.3, E§8.2)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| N1 | Hovering shows inference dots; the nearest shows a full triad | E§6.3 | ✅ | |
| N2 | Planar face: centroid, every vertex, edge midpoints, arc/circle centers, inner loop (hole, slot) centers; Z normal to the face | E§8.2 | ✅ | model test `inference_points_match_onshape` |
| N3 | Cylindrical face: axis points at both ends and the middle, Z along the axis | E§8.2 | ✅ | model test |
| N4 | Conical face: top, middle, bottom and virtual sharp | E§8.2 | ✅ | T:`inference_points_on_cones_spheres_arcs_and_vertices` |
| N5 | Spherical face: center | E§8.2 | ✅ | T:`inference_points_on_cones_spheres_arcs_and_vertices` |
| N6 | Circular edge: center only | H:mates | ✅ | model test |
| N7 | Linear edge: midpoint and both ends, Z along the edge | E§8.2 | ✅ | model test |
| N8 | Arc / curve edge: center, midpoint and ends | E§8.2 | ✅ | T:`inference_points_on_cones_spheres_arcs_and_vertices` (fixed: an arc edge's Midpoint resolved to the center, regen/implicit.rs) |
| N9 | Vertex: the point, Z normal to a face | E§8.2 | ✅ | hovering a part vertex: Z normal to the first planar face around it T:`inference_points_on_cones_spheres_arcs_and_vertices` |
| N10 | No seam vertices of circles offered | — | ✅ | model test |
| N11 | Shift locks the entity; the connector snaps to its nearest point | E§6.3 | ✅ | |
| N12 | Hovered entity highlighted | E§6.3 | ✅ | |
| N13 | Sketch entities / regions as targets | E§8.2 | ⏳ | sketch instance curves: a line offers its midpoint (Z along the line) and its ends, a circle its center (GUI n2–n3). Regions are pickable faces (`face_refs` = SketchRegion); hovering inside a loop offers the region centroid plus the loop's corners and edge midpoints, Z along the sketch normal (live 601; `regen::implicit` SketchRegion branch, `inference_candidates`); model test `sketch_instances_are_wire_occurrences_with_edges` |

## 8. Mate connector dialog (E§8)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| X1 | Origin entity pick with inference | E§8.1 | ✅ | |
| X2 | Flip primary, reorient secondary | E§8.1 | ✅ | |
| X3 | Move: X/Y/Z translation, rotation axis + angle | E§8.1 | ✅ | |
| X4 | Origin type Between entities, Realign (primary / secondary axis entity) | E§8.1, H:assembly_mate_connector | ✅ | Between verified in the app, Realign model-tested; entities of the same instance; model test `mate_connector_between_entities_and_realign` |
| X5 | Add mate connector to instance origin (prefilled) | E§5.4 | ✅ | |

## 9. Dragging, solving, triad (H:Home__triad_manipulator, E§24)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| R1 | Free drag moves an instance within its remaining DOF; limits stop it | E§24.1 | ✅ | model test `slider_respects_limits` |
| R2 | Unmated, unfixed instances move exactly | H:triad | ✅ | |
| R3 | Fixed instances cannot be dragged (cue, no manipulator) | H:triad | ✅ | no manipulator for a selected fixed instance (live 565); press-dragging it: `no-drop` cursor + the fix glyph at the cursor, nothing moves, no message (live 562–563; `AsmUi.fixed_drag`); no pre-highlight while dragging (live 562: hover cleared when the fixed drag starts). GUI :103 r5 (glyph, nothing moves), r7 (selected, no triad) |
| R4 | Click an instance shows the triad: axis arrows, plane squares, rotation arcs | H:triad | ✅ | |
| R5 | Drag the triad center to reposition it (snaps to MCs / inference points) | H:triad | ✅ | |
| R6 | Typed value field after a triad drag | H:triad | ✅ | |
| R7 | Right-click center: Move to origin | H:triad | ✅ | |
| R8 | Right-click axis arrow: Align with Z, Anti-align with Z | H:triad | ✅ | |
| R9 | Right-click arc: Rotate 90 degrees, Rotate 180 degrees | H:triad | ✅ | |
| R10 | Box selection selects instances (Shift/Ctrl adds) | — | ✅ | |
| R11 | Mate values on the selected mate; double-click a value to type one | H:mates | ⏳ | live 300–318, 305z; bundle `calculateMateValues` / `calculateDimensionDisplayData`: the first connector measured in the second's frame (`onshape_mate_values`; typed values converted back to the solver's convention by `solver_values_for`, unit test `typed_mate_values_round_trip_through_the_solver_convention`). Rotation: angular dimension around the moving connector, radius 80 px fixed when first shown (`PREVIEW_ANGULAR_DIMENSION_RADIUS_PIXEL_SIZE`), arc from +X through the value, arrowheads out, text at half the sweep in the plane along X, arc broken around it. Translations: in-plane linear dimensions X: origin → x, Y: (x,0) → (x,y), Z: (x,y,0) → (x,y,z), arrowheads out, text centred, line broken (`value_dims`, `ValueLine`). Double-click under Xvfb: `xdotool click --repeat 2 --delay 80` |
| R12 | Mates solved simultaneously; red when unsolvable | E§24.2 | ✅ | |
| R13 | Rigid detection icon (0 DOF + fixed) | E§24.2 | ✅ | see L24 |

## 10. Snap mode (H:snap_mode, E§20)

| # | Behaviour | Source | Status | Notes |
|---|---|---|---|---|
| S1 | Drag an instance: it goes transparent, MCs of others wake near the cursor | E§20 | ✅ | pre-existing |
| S2 | Release opens the mate dialog (Fastened) with both MCs filled | E§20 | ✅ | pre-existing |
| S3 | Ctrl cycles the dragged instance's MCs | E§20 | ✅ | see K11 |
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
| G8 | BOM (flattened) | E§23 | ✅ | BOM Table pane (live 181–187): BOM type Flattened / Structured, Item, Quantity, Part number, Description + Add column list (16 properties), ⋯ Copy table / Show excluded/suppressed / Show top level assembly row / Export to CSV, header Move left / right / Remove column, row Switch to Part Studio, editable cells write Properties; Subassembly BOM behavior and Exclude from all BOMs honoured (`assembly/bom.rs`, `assembly_bom.rs`, T:`bom_flattened_structured_exclusion_and_cell_edits`); structured / excluded row looks: capture requested GUI :103 a2–a6, g2 (live details 350–378) |
| G9 | Display states: add, select to apply, update, rename, remove; new items keep defaults | E§15, B | ✅ | model test `folders_and_display_states` |
| G10 | Replace instance (matches geometry) | E§14 | ✅ | pre-existing |
| G11 | Subassemblies: Move to new, Dissolve, Create empty | E§5.5 | ✅ | |
| G12 | Flexible subassemblies (default): their mates solve with the parent, their parts drag on their own; Lock/follow position to ▸ Lock current position / named position / Unlock; lock icon (click unlocks) | E§5.5, E§24.2 | ✅ | model test `flexible_subassembly_moves_in_parent_and_locks`; subassemblies with patterns, relations, tangent mates or nested flexible subassemblies stay rigid |
| G13 | Modeling in context, Create Part Studio in context | E§18 | ✅ | Assembly contexts (`crates/model/src/in_context.rs`): Edit in context / Create Part Studio in context store a snapshot of the other parts (BREP, placed relative to the primary instance) in the Part Studio; features may reference its faces / edges / vertices ("Face of Context 1"; regeneration adds the snapshot bodies only to features that reference them); the context persists only once referenced; Part Studio "Assembly contexts" section (None / Context N dropdown, ⋯ Rename…, Update context, Hide/show context instances…, Restore context view state, Follow position ▸, Delete), amber "Context 1 of <assembly>" banner [Hide/show instances ▾][Done], yellow / outline context arrows on features; Assembly: solid / dashed context arrows on instance rows, blue update dot, Edit in context ▸ [Edit Context 1, Create new context], Update context ▸, Set as primary instance ▸ (live 500–525). Model test `edit_in_context_snapshot_is_referenced_by_part_studio_features`. A context face picked in a feature dialog is filled magenta with the amber outline, and the section shows greyed without ⋯ while the dialog is open (live 504). Hovering a feature's context arrow outlines its context references in magenta (506). Out-of-date contexts get a blue dot after the "Assembly contexts" header ("Update available for a context.") and "• Context 1" in the dropdown (520–521); Update context clears it in both tabs. The stale state is published by the assembly after each evaluation (`StaleContexts` global). GUI :103 e2–e16, c11–c31 (live 502–521 side by side) |
| G14 | Standard content (verified in the app): insert follows the cursor and snaps to a hole's mate connector (no snap mode needed), `a` flips, click adds a Fastened mate; auto-size pick fastens into the picked hole(s) (planar face: all its holes); Edit standard content instance (Size/Length, Update); standard content icon in the list | E§4.7 | ✅ | no stacking (Insert closest / furthest), no thread type, no part number |
| G15 | Show mates mode: hover shows an instance's mates, click pins | E§21 | ✅ | |
| G16 | Isolate / Make transparent panel: transparency, Expand by distance / connectivity | E§24.6, H:View__isolate | ✅ | Isolate / Make transparent dialog as live 61 (220 px, Transparency controls: ☐ Select transparent geometry + transparency slider; Component controls: Expand: connectivity / distance select + expand slider, ?); model test `isolate_expands_by_distance_and_connectivity` GUI t2 vs live 61 |
| G17 | Assembly configurations and variable table | E§22 | ✅ | Configuration panel (live 190–200): Configure Assembly ▾ (List / Checkbox / Configuration variable), sections with + Configure assembly features, two-level header, Name add row, row click = current configuration, input ⋯ (Copy table, Copy input, Rename, Delete), Configurations box above the instance list; yellow Select instances or mates / Select parameters modes, mate dialog parameter picking (Unsuppressed, Offset, offsets), instance menu Configure suppression, toasts; subassembly instances in a configuration (`assembly/config.rs`, `assembly_config.rs`, T:`assembly_configurations_drive_suppression_and_mate_offsets`) GUI :103 c2–c13 (live 191–199 side by side) |

## 13. Live Onshape comparison (2026-10-18 capture: `/root/onshape-research/live/assembly/`, `live/dom/*assembly*`, behaviours.md "Assembly")

Where the live app differs from the help pages, the live behaviour wins (items above were adjusted).

| # | Behaviour (live) | Source | Status | Notes |
|---|---|---|---|---|
| LA1 | Insert dialog 300 x 587: header "Insert parts and assemblies" + green ✓ (dim until something is inserted) + red ✕; tabs Current document / Other documents / Standard content (100 px, two-line labels, blue 2 px underline) | live/assembly/01, dom/dialog-assembly-insert.txt | ✅ | `render_insert_panel`; verified in the app |
| LA2 | Document row: name (14.67 px), "Main" with branch icon, Create version… and Version graph buttons (30 px outlined) | same | ✅ | buttons dispatch CreateVersion / ToggleVersionGraph |
| LA3 | Part Studios / Assemblies sub-tabs (150 px), "Search Part Studios" / "Search Assemblies" box | same | ✅ | placeholder follows the tab |
| LA4 | Filter row: Parts toggle (on by default), Sketches toggle, "Insert entire Part Studio as rigid" toggle at the right | live/assembly/01–02 | ✅ | Parts (default on), Sketches, Surfaces (when the document has surfaces), rigid toggle; skb |
| LA5 | Tree: chevron, 70 x 40 thumbnail, name (13.33 px light); Part Studio rows expand / collapse | same | ✅ | chevron toggles; rendered 70x40 thumbnails of every Part Studio and part (offscreen, document-thumbnail look, cached per studio hash; `assembly_thumbs.rs`); assemblies keep the icon |
| LA6 | Footer: "Inserted: N" (bold, right) and the gray "↶ Undo to remove instances" row | same, live/assembly/07 | ✅ | count = placed instances inserted since the dialog opened that still exist; the ↶ undoes the last insert (insert + place steps) |
| LA7 | Clicking a part row inserts it at once; it follows the cursor until the click; the dialog stays open; ✓ closes | live/assembly/04–08 | ✅ | pre-existing |
| LA8 | Instance list: "Filter by name" box; the state glyph follows the name: fixed (bar over ground hatch), free (3-arrow tripod, instance has DOF), top "Assembly 1" row: arrow onto ground once anything is fixed; no DOF counts | live/assembly/80 | ✅ | icons `list-fixed`, `list-free`, `list-grounded`; verified |
| LA9 | Clicking a part shows the triad at the picked point | live/assembly/12 | ✅ | `triad_at_click` (axes along the world axes); triad look (light gray outline, open arrowheads) is a visual for ui2 |
| LA10 | Right-click on the triad without selecting a handle gives the normal menu; click a handle first, then right-click it for its menu | live dom rmb-asm-triad-center, H:Home__triad_manipulator | ✅ | `AsmUi.manip_selected` |
| LA11 | Right-click an unselected instance in the graphics: general menu (Show all, Create Drawing of <assembly>…, Zoom to fit, Isometric); a selected one: the instance menu | dom/context-menus-assembly.txt | ✅ | |
| LA12 | Graphics instance menu order: Hide, Hide other instances, Hide all instances, Show center of mass, Isolate…, Make transparent…, Section view…, Suppress <name>, Fix, Use best available tessellation · Copy <name> · Create Drawing of <part>…, Create Drawing of <assembly>…, Export… · Edit in context, Find in instance list, Add mate connector to instance origin…, Replace <name>…, Switch to <studio> · Select other… · (Add comment) · Zoom to fit, Zoom to selection, View normal to · Delete <name> | dom/context-menus-assembly.txt (rmb-asm-two-instances-selected) | ✅ | all of the live list but Add comment (cloud-only), GUI :103 g3–g5 (Edit in context before Find in instance list, as in the live dump; it opens the Part Studio in context); Show center of mass (quartered-circle marker, density-weighted, follows the solve); View normal to only over a planar face (g4). Open: whether "Check interference…" belongs in this menu with 2 instances selected (REQUESTS line 200) |
| LA13 | List instance menu order: Properties…, Instance properties… | Hide, Hide other, Hide all, Show center of mass, Isolate…, Make transparent…, Suppress, Dynamic suppression ▸, Fix, Use best available tessellation | Edit in context, Add mate connector to instance origin…, Replace instances…, Switch to … | Copy <name> | Move to new subassembly, Change to version…, Export…, Release… | Add comment | Zoom to selection | Create Drawing of … | Add selection to folder…, Create empty subassembly | Delete | same | ✅ | live order incl. Properties…, Instance properties…, Dynamic suppression ▸, Use best available tessellation, Edit in context, Change to version…; Release… is a paid-plan upsell (excluded), Add comment cloud-only; GUI e1 |
| LA14 | Top-level row menu: Properties…, Show instance list properties | Rename, Show center of mass, Check interference… | Create Drawing…, Add exploded view… | Export…, Release… | Add comment | Expand all, Collapse all | same | 🔧 | Properties… (assembly metadata modal with Subassembly BOM behavior), Show / Hide instance list properties, Rename, Show center of mass, Check interference…, Create Drawing…, Add exploded view…, Export…, Expand / Collapse all; Release… excluded (upsell) |
| LA15 | Mate row menu: Rename, Edit… | Reset | Hide, Hide other mates, Hide all mates (hidden mate: Show, Show all mates), Isolate…, Make transparent…, Edit named position driver mate…, Suppress, Dynamic suppression ▸ | Animate… | Add comment | Add selection to folder…, Expand ▸, Collapse ▸ | Delete | same | ✅ | live order and icons; Edit named position driver mate…, Dynamic suppression ▸; reduced menu while editing a named position; Apply limit position ▸ kept (help); GUI v40 |
| LA16 | Origin row menu: Create Part Studio in context… | Hide | ✅ | Create Part Studio in context… ("Origin of new Part Studio" dialog, Origin chip or an assembly mate connector row), Hide / Show, Create empty subassembly; GUI p1 |
| LA17 | Mate dialog: connector chips "Mate connector of <instance>" with the edit-connector icon and × on every row; ⇅ in the box header swaps the two connectors | live/assembly/33–35 | ✅ | replaces "Reorder items" |
| LA18 | Mate type select order: Slider, Cylindrical, Revolute, Pin slot, Planar, Ball, Fastened, Parallel | live/assembly/36 | ✅ | Tangent / Width stay separate tools |
| LA19 | Bottom row: flip, reorient, ▶ animate, Solve, ?; "Simulation connection" checkbox | dom/dialog-mate-*.txt | ✅ | ▶ dimmed for 0-DOF mates; Simulation connection n/a |
| LA20 | After the 2nd pick a floating toolbar (310 px, ◻◻◻ drag handle): Mate type ▾, flip, reorient, Solve, green ✓ (commit and start a new mate), ✕ | live/assembly/35, dom/mate-hud.txt | ✅ | `render_mate_hud`; draggable by the handle |
| LA21 | A new mate is hidden by default (row light gray, tooltip "Fastened 1 (Hidden)"); j shows mates | live/assembly/38–41 | ✅ | mates start hidden (show_mates off) |
| LA22 | Animate dialog "Animate mate": Mate DOF to animate box with the chip "⟳ - z - Revolute 1" ×, Start value 0 deg, End value 360 deg, Steps 300 (right-aligned fields), Playback type ▾ (default Reciprocate) + ↻ reverse, Current value, ▶ ■, ? | live/assembly/53, dom/dialog-animate.txt | ✅ | `render_anim_panel` rebuilt to the live layout; DOF buttons under the box for several-DOF mates |
| LA23 | Mate limits: per limit an axis letter + min / max glyph and a right-aligned field, empty = "No minimum" / "No maximum" | live/assembly/51 | ✅ | per-limit rows with icons `limit-*.svg` and "No minimum" / "No maximum" placeholders (`assembly_mate_dialog.rs`) |
| LA24 | Instance and top-row "Export…" | dom/context-menus-assembly.txt | ✅ | STEP / IGES / STL / OBJ / BREP at the solved positions; verified (STEP with three named parts) |
| LA25 | Instance menus: "Use best available tessellation" toggles the finest tessellation for the instance's parts; when on the same slot reads "Use automatic tessellation setting" | dom/context-menus-assembly.txt, assembly/110 | ✅ | skb: `Instance.best_tessellation`, model test |
| LA26 | Insert footer "Inserted: N" counts the instance still following the cursor; no hint toast while placing; the following instance is selected (amber) and deselected once placed | live/assembly/04–06 | ✅ | skb QA3 fixes; GUI :103 /tmp/skb/asm/b1.png |
| LA27 | Mate HUD ✓ commits the mate and starts a new one ("Commit current mate and start a new mate") | dom/mate-hud.txt, H:mates | ✅ | QA3 question answered by the live tooltip |
| LA28 | No list-row tooltip over an open context menu | QA3 | ✅ | `os_menu::is_open` |
| LA29 | Mate row menu Expand ▸ (Expand selected / all), Collapse ▸ | dom/context-menus-assembly-revolute.txt | ✅ | GUI b3-c.png |
| LA30 | Instance row › Properties…: 800 px Properties modal (Name*, Description, Category, Part number, Revision, State*, Vendor, Project, Product line, Material, Title 1–3, Not revision managed, Exclude from all BOMs, Unit of measure, Mass ☐ Override + Calculate value); form undo / redo; Apply, Save (40 % until edited), Close | assembly/100, dom/dialog-assembly-part-properties.txt | ✅ | `properties_dialog.rs` (shared; Part Studio's Properties… can use it), `PartProperties.meta`; Unit of measure list + default: capture requested GUI v30 |
| LA31 | Instance row › Instance properties…: 600 px modal, instance name, "Reference designator" (focused), Save / Close | assembly/101 | ✅ | `Instance.reference_designator` |
| LA32 | Edit in context: the instance's Part Studio opens with the rest of the assembly as translucent context graphics (≈20 % opacity), the part selected, banner "Assembly 1 displayed [Go to assembly] [Hide/show instances] ✕" + Show internal geometry, Select transparent geometry, Context transparency slider; transient | assembly/115–117, dom/edit-in-context-banner.txt | ✅ | see G13 (persisted Assembly contexts, references, Update context) |
| LA33 | Create Part Studio in context: new Part Studio (origin at the chosen connector) opened in context; Go to assembly ▾ › Insert and go to assembly inserts its parts at that origin | assembly/114, H:Assembly__create_part_studio_in_context | ✅ | banner of a Part Studio created in context (live 551): "<asm> displayed" · [Insert and go to assembly][▾] (Insert and go to assembly / Go to assembly / Hide/show instances) · ✕; the command opens the 220 px dialog "Select items to insert" with the empty field "Select parts/surfaces/sketches" (faces pick their part, curves their sketch; parts list rows too); ✓ with 0 items closes and stays, else inserts them ("Part 1 <3>", sketches as sketch instances) at the Part Studio origin and opens the assembly (live 545–550); `part_studio/in_context.rs` (`render_insert_go`) GUI :103 g1–g10 (2026-10-19): Origin menu, dialog, banner, empty "Select items to insert", Part 1 picked, ✓ → "Part 1 <3>" at its Part Studio position with the floating icon only |
| LA34 | Change to version… / toolbar Update all references: "Reference manager" (330x433): Update to latest (Update all) / Selective update (checkboxes, Version graph ▾ per document, Update selected); this document's workspace references become version references | assembly/111–112, dom/dialog-reference-manager.txt | ✅ | `assembly_props.rs`. Change to version… is scoped to the instances it was opened from (live 604–607: Update all changes only that instance); Update all / Update selected show no message, the dialog stays open and recounts ("Newer versions available for 0 documents") GUI :103 t4–t9 (T10 fixture): both tabs, 66x40 thumbnail, ⓘ icon line kept at 0 documents, white tab row, 11 px ✕ |
| LA35 | Search tools (Alt+C) in the assembly: the Part Studio's popover with the assembly tools | menus/tool-search-*.png | ✅ | `assembly_search.rs`; descriptions: capture requested |
| LA36 | Named positions pane (712 px, docked): Name column + per driving-DOF value columns under the mate name ("Revolute 1" / "⟳ - z", "0 deg"), italic "Name" add row, info box while nothing drives, footer "Add named position" (adds a row and enters editing mode); click / double-click only edit the cell; row menu Apply / Edit / Duplicate / Update / Move down / Move up / Delete <name>; applied row blue with a 2 px bar + green toast "<name> is applied." | assembly/150–161, dom/panel-named-positions-*.txt | ✅ | `assembly_positions.rs`, `Assembly.position_drivers` |
| LA37 | Editing a named position: yellow `#fab601` "Editing <name> [Done]" banner at the top centre, toolbar reduced to Undo / Redo, typed values drive the mates | assembly/105–107, 155 | ✅ | `render_np_banner` |
| LA38 | Dynamically suppressed instance: gray italic struck-through name, ƒx glyph instead of the DOF glyph, tooltip "<name> (Suppressed with expression)"; menu Unsuppress + Edit dynamic suppression | assembly/140–142 | ✅ | skb GUI v38c–d |
| LA39 | Insert › Other documents (live 610–621): search row (funnel, "Search or paste URL" / 'Search in "<category>"', 🔍); categories Owned by me, Recently opened, Created by me, Shared with me ("No documents found"), Public; category view home · ‹ · name, 51 px rows (70x40 thumbnail, bold name, grey latest version), folders first; search results with the match in blue; a document opens with home · ‹ · name / ⑂ version, Create version…, Version graph (○ Workspace ◆ Version, Main not selectable, versions incl. Start), Part Studios | Assemblies, "N changes since the last version. Create a version in <doc>.", "No Part Studios available."; remembers the last document | assembly/610–621, dom/assembly-insert-other-*.txt | ⏳ | `assembly_panels.rs` (`OtherDoc`, `render_insert_other`). Inserting other documents' assemblies is not supported by the model yet (rows listed only); Public lists nothing locally |
| LA40 | In-context ghosts (Edit in context, no dialog): a hovered context face is outlined `#ffc685` (live 503), a selected one gets the amber outline; context faces win the pick over the translucent default planes (psb's pick-order fix, `draw_lists` lit-after-unlit transparents) | assembly/503 | ✅ | `context_ref_overlay` (part_studio/in_context.rs); GUI :103 p5–p9; live look without a dialog requested (REQUESTS skb 10-19 in-context look) |
| LA41 | A context exists for the user only once a feature references it: no context arrow / Edit in context ▸ submenu / stale dot for an unreferenced context; Assembly contexts section and the amber banner only once the open feature picks context geometry | assembly/502, 548, behaviours "Create Part Studio in context" | ✅ | `instance_contexts`, `contexts_in`, `publish_stale_contexts` filter `referenced_contexts`; `add_assembly_context` drops unreferenced contexts before creating one (so it's "Context 1" again); `render_contexts_section` `pending` needs `params_use_context`; GUI :103 y1–y2 (Edit in context, back to the assembly: no arrow, plain Edit in context) |
| LA42 | Bottom-right Show analysis tools: the tool list of an assembly (help 'Available in' + Part Studio order): Curve/surface analysis…, Interference detection…, Zebra stripes…, Reflection analysis… | H:View__analysis_tools, live analysis/03 | ⏳ | `assembly_mass.rs` `open_analysis_menu`. Interference detection (existing panel); Zebra stripes (220 px dialog, Stripe count 35 / Flip stripes / Show edges, ✓ keeps them, ✕/Esc off; `AsmSceneOptions.zebra`; GUI z1); Reflection analysis (`assembly_reflection.rs`: the Part Studio dialog + 300 px Select image panel, library maps / document images, `AsmSceneOptions.reflection`); Curve/surface analysis (`assembly_curvature.rs`: picks of instance edges/faces evaluated in their Part Studio with `analysis::edge_curvature` / `face_curvature`, combs moved by the occurrence transform). Live capture of the assembly menu requested |
| LA43 | Bottom-right Display mass and section properties: the 440 px Mass and section properties dialog for the selected instances' parts (all when none), where they sit | H:View__mass_properties_tool, live dom/dialog-mass-properties.txt | 🔧 | model `analysis::assembly_mass_properties` (occurrence transforms, parallel-axis combination; test `assembly_mass_properties_combine_parts_where_they_sit`); dialog `render_mass_dialog` (Part | Face, Parts to measure, Mate connector for reference frame, Show calculation variance, Mass/Override, Volume, Surface area, Center of mass, inertia grid; warning bubble 'One or more parts do not have a material defined.'). Not yet: Face tab values, reference connector, Override, calculation variance |
