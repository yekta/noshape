# QA round 3 handoff (qa3, after the Onshape restyle; main at e4b90e2)

State: building in `/root/target-qa3`; sessions not started yet.

## Sessions
1. Bracket from scratch — DONE (/tmp/qa3/home/Documents/noshape/Bracket.noshape): sketch, dims, extrude, cut, fillet, chamfer, hole, feature pattern, feature mirror, edit Sketch 1 (80→100, all downstream regenerated), rollback drag, undo/redo x3, close/reopen. No crash, no regen error.
2. Variables and configurations — DONE: Variable #t (rollback bar moved up to insert it), Extrude 1 depth `#t` via the # autofill, List configuration Default 12 mm / Configuration 2 20 mm, switching regenerates correctly.
3. Assembly — DONE: inserted Part 1 twice, Fix on <1>, Revolute between the Ø10 holes, drag rotates (Rz readout), Animate plays. No crash.
4. Drawing — DONE: + → Create Drawing (ANSI_A_MM), Insert view Front, projected Top + Right, Section A-A, linear 95.46 and Ø8 (Shift+D), PDF export checked (/tmp/qa3/out/drawing1.pdf: all views, hatch, dims, title block).
5. Import/export — DONE via ☰ → Export active Part Studio (STEP/STL/OBJ) and ☰ → Import into Part Studio (new Part Studio 2): all three come back. Part right-click → Export… was broken (fixed, see 2).

Rule (user, via lead): any difference from Onshape is a bug (except cloud-only and simulation features). Never write one up as acceptable.

## Bugs fixed (symptom → cause → fix → test)

1. **Chamfer/fillet larger than the corner fillets it runs over gave a broken part** (5 mm chamfer of a top edge
   on a plate with 3 mm corner fillets, tangent propagation on: the preview showed the hole walls through
   missing faces; the feature was "OK").
   - Cause: `BRepFilletAPI_MakeChamfer/MakeFillet` report `IsDone` but return a solid that fails BRepCheck.
   - Fix: `crates/model/src/regen/features/detail.rs::require_valid_blend` after the Fillet and Chamfer
     features' kernel call (BRepCheck on the result; fails only if the input was valid). Messages are Onshape's
     "Failed to fillet selections." / "Failed to chamfer selections." (kernel reason at debug level).
     Not in the kernel op itself: sheet metal bend fillets return BRepCheck-invalid solids that it relies on
     (`features_c::sheet_metal_loft_hopper_flattens` fails if the check is in `op_fillet`).
   - Test: `crates/model/tests/qa_regressions.rs::blend_over_smaller_fillets_is_valid_or_fails`.
   - Made in /root/noshape-qa3 (worktree of e4b90e2), `cargo test -p noshape-model` green, clippy clean;
     copied to the shared tree (both files were unchanged there).

2. **Part right-click → Export… did nothing** (Export button clicked, no prompt, dialog stays) whenever the in-app
   path prompt is used (Linux without an XDG portal, `NOSHAPE_NO_NATIVE_DIALOGS`).
   - Cause: the dialog's `on_ok` opens the prompt with `window.open_dialog` and returns `true`; closing "the top
     dialog" then closes the new prompt instead of the Export dialog (assembly export already worked around it
     with `window.defer`).
   - Fix: `crates/app/src/files.rs::request_save_path` / `request_open_path` open the in-app prompt with
     `window.defer`, so every caller (Part Studio export, flat pattern export, …) is safe. Applied to the shared
     tree (those two functions were untouched there; ui is restyling `in_app_save_prompt` below them).
   - Test: app-level (no unit test harness for dialogs); verified in the app — see "Verification".

## For other agents (not fixed by qa3; the lead forwards)

Found on main e4b90e2 (private worktree /root/noshape-qa3, because the shared tree didn't compile at the time).
"Shared tree: unchanged" means I grepped the working-tree diff and the code is still the same.

### psb (Part Studio behaviour)
- **Hole: clicking a sketch in the feature list doesn't add its points** while "Sketch points to place holes" is
  active (the row just gets selected). Onshape uses all the sketch's points (like Extrude takes all regions).
- **Rolled-back features aren't italic gray while editing an earlier sketch** (double-click Sketch 1 with 9
  features after it: all rows stay normal). Onshape greys them (rollback-bar drag does grey them correctly).
- **Selection stays after editing**: after Sketch 1 edit is accepted, or after clicking Sketch 3 in the list
  for a Hole, the row stays highlighted (selected) indefinitely.
- **Dimension value box shows "80" without unit** (Onshape: "80 mm", value selected) when re-editing; the new
  dimension box shows "77.82060542" (unit-less, clipped). (skb owns the box.)
- **Fillet/chamfer: clicking a short edge often adds nothing and shows "Select faces or edges."** The edge is
  highlighted on hover, but the click resolves to the edge's midpoint (within 8 px, `behaviors.rs::midpoint_pick`)
  or to an end vertex, and `pick()` rejects it because the field takes only edges/faces. With a dialog open the
  midpoint marker isn't even drawn (`midpoint_overlay` checks `edit.is_none()`), so the user can't see why.
  Onshape's pre-selection follows the active field's filter: only acceptable entities highlight and get picked.
  Fix: when a dialog field is active, skip `midpoint_pick` unless the filter accepts points, and resolve a
  vertex/midpoint click to the hovered edge when the filter takes edges but not points. Repro: 80x40x10 plate,
  Shift+F, click a vertical corner edge (35 px long on screen) at its middle. Shared tree: unchanged.
- **Extrude: Enter (apply depth) then Enter again doesn't commit** (Onshape: the second Enter commits).
  `feature_dialog.rs`: `enter_applied` is set by `PressEnter` and only cleared by the following `Accept` action,
  which the Input's own Enter binding never lets through, so the flag swallows the next Enter. The shared tree
  rewrote this (`enter_applied` removed): re-check there.

### ui (Part Studio chrome, dialogs, viewport)
- **Feature-list toggle button (≣ at x≈343, y≈523) is drawn on top of feature dialogs**, clipping fields
  (Extrude "Merge scope" chip read "t 1"; Hole/Linear pattern dialogs too). Onshape: dialog above everything.
- **Viewport sketch-name label shows through the dialog title** ("…h 3" over "Sketch 3"): too strong for a 95%
  white dialog; labels are probably drawn above the dialog.
- **Dialog dropdowns (End type etc.)**: no search input, no keyboard (typing "thr" + Enter does nothing), current
  item not highlighted, list ~112 px wide. Onshape: 194 px list, search input, ↑/↓, typing filters, Enter picks,
  current item `#1651b0`/white (live/dialogs/extrude-13..16).
- **Hole dialog**: a lone "Standard" label on an empty row; "Hole type"/"Drill size" use bordered ▼ buttons
  unlike the other underline dropdowns; title doesn't show the spec ("Ø 5 mm ↧ 12 mm" in Onshape).
- **Fillet dialog is the old layout**: "Edges or faces to fillet", "Cross section", Radius|Width tabs,
  "Partial / variable fillet". Live Onshape: "Entities to fillet", Measurement ▾, Control ▾, Radius ±,
  Asymmetric, › Partial fillet, › Variable fillet, › Allow edge overflow, › Smooth fillet corners.
- **"regenerating…" text in the Features header overlaps the header icons.**
- **Tool search (Alt+C)** is a plain centred list; Onshape: 600 px popover under the toolbar's right end with a
  description pane, "Close search [escape]" trigger text and the "Highlight tool in toolbar" footer.
- **Region hover** fills the region light tan; Onshape only draws the region boundary light orange.
- **Extrude preview** looks like an opaque part; Onshape: translucent dark slate + amber outline of the region.
- **Right-panel strip** has 3 buttons; Onshape has 5 (Appearance, Configuration, Custom tables, Inspection,
  Variable table).
- **View animations are slow**: N (view normal) takes >1.2 s here (Onshape ≈ 0.5 s); measured on llvmpipe.

### asb (assembly / drawing)
- **Mate HUD ✓ commits and immediately opens "Revolute 2"** (accept-and-repeat). Onshape's ✓ closes the mate
  dialog; only Shift+Enter repeats. Please verify live and fix.
- **Insert dialog**: Part Studio and part rows show generic icons, not 48 px thumbnails; "Inserted: 0" while the
  first instance is following the cursor; a blue toast "Click to place the inserted instance (Esc keeps it where
  it is)" that Onshape doesn't show.
- **Triad** is coloured rings/arrows; Onshape's drag triad is gray-outlined (blue on hover).
- **Instance context menu** is 342 px wide with 28 px rows (Onshape: 196 px, 22 px rows, 12 px text) and lacks
  Properties…, Instance properties…, Show center of mass, Dynamic suppression ▸.
- **Stale tooltip over a context menu**: right-click the mate row → the row tooltip "Revolute mate / Offset: off"
  stays drawn on top of the menu (covers Rename/Edit…).
- **Drag readout "Rz -12.232°" stays in the viewport** after the drag ends (still there after Animate…).
- **Animate dialog**: no "Mate DOF to animate" field, Steps 60 (Onshape 300), Single/Reciprocate/Loop toggles
  (Onshape: Playback type ▾ Reciprocate + ↻), title "Animate Revolute 1" (Onshape "Animate mate").
- **Drawing: Create Drawing from + doesn't open Insert view**; Onshape opens the Insert view panel with the view
  following the cursor. Template table: no default row highlighted.
- **Drawing views stay orange after Esc** (QA2 reported it too).
- **Drawing Section view**: no dialog (Onshape: Vertical | Horizontal | Angular tabs, label "A", jog/flip, Show
  cut geometry only, Section depth); clicks just outside the view are silently ignored.
- **Drawing Dimension (d) doesn't take a circle** ("Select a point or a straight edge"); I believe Onshape's
  Dimension tool is smart (circle → diameter). Shift+D works. Please verify live.
- **Drawing export**: toolbar ⤓ ▾ PDF/SVG/DXF straight to a path. Onshape: tab right-click → Export… → "Export
  Drawing" modal (File name, Format ▾ PDF/DWG/DXF/DWT/SVG/PNG/JPEG/TIFF…, Version, Text, Color, Options).

### skb (sketch)
- **No live values while drawing**: corner rectangle shows no width/height, circle no Ø (Onshape shows them in
  `#449ccd` next to the preview).
- **Point tool makes filled dots**; Onshape drops hollow points.
- **Vertical dimension text sits on its dimension line** ("40" overlaps the line); horizontal ones break the
  line around the text.
- **Just-applied dimension has no tan background** (Onshape highlights it).

### ui (more, sessions 2–5)
- **Modals are see-through**: the 3D model shows through "Create Drawing" and title-block lines through "Select
  a part or assembly". Onshape modals are opaque white over a dim backdrop.
- **Element tab right-click** has only Duplicate, Delete. Onshape: Delete, Rename, Properties…, Duplicate, Copy
  to clipboard, Select as document thumbnail, Move to document…, Export….
- **+ menu**: "Part Studio, Assembly, Create Drawing, Variable Studio, Feature Studio", ~120 px wide, covers the
  tab bar. Onshape: Create Part Studio, Create Assembly, Create Variable Studio, Create Drawing…, Create folder,
  Import… (+ Create Feature Studio), 253 px wide, opens upward.
- **☰ document menu** differs (Documents, New, Open, Open recent, Save a copy as, Export/Import, Units, Close
  window). Onshape: Rename document, Move to…, Document details…, Workspace units…, Workspace properties…,
  Close document (local equivalents of New/Open can stay in the OS menu).
- **Part export dialog**: format chips; Onshape: File name + Format ▾ + format options (STL units/resolution…).
- **Keyboard focus is lost after an in-app prompt closes** (and/or after creating a tab from +): F/Shift+7 do
  nothing until the viewport is clicked. Onshape shortcuts work right after a dialog closes.
- **Toolbar tooltips don't appear when moving directly from one button to the next** (only after entering the
  toolbar from outside). Onshape shows each after ~0.5 s.
- **Configurations panel** is a form, not Onshape's 712 px table (configurations as rows, configured parameters
  as columns, "Configure Part Studio ▾"). After Enter in a configured value the cell keeps keyboard focus, so
  shortcuts type into it ("20 mm!f").
- **Variable feature**: title "Variable 1" (Onshape "#? = 0 in", then "#t = 12 mm", also the list name); no
  Assigned | Measured | From import tabs; default value 25 mm (Onshape 0); Name isn't focused on open.

### ui2 (start window)
- Create ▾ menu: "Document…, Folder…, Open file…". Onshape: Document…, Folder…, Import files…, Import from ▸,
  Label….
- Document list columns: Name | Modified | Location | Size. Onshape: Name | Modified | Modified by | Owned by.

## How to test
- Build: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-qa3 nice cargo build -p noshape`.
- Helpers in `/tmp/qa3/bin/`: `start.sh [args]` (DISPLAY=:106, HOME=/tmp/qa3/home, no native dialogs,
  NOSHAPE_PERF=1, log in /tmp/qa3/app.log), `stop.sh`, `shot.sh name [crop]`, `x.sh` (xdotool on :106).
