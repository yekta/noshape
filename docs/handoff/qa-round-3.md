# QA round 3 handoff (qa3, after the Onshape restyle; main at e4b90e2)

State: all 5 sessions done on main e4b90e2 (worktree /root/noshape-qa3, target /root/target-qa3).
Fixes 1 and 2 are on main (18cadb8). Fixes 3 (shell) and 4 (split-piece references) are in the shared tree,
uncommitted: `crates/model/src/regen/features/detail.rs`, `crates/model/src/regen/context.rs` (only the
`find_faces/find_edges/find_vertices` bodies + new `edges_related`; the other 37 changed lines there are another
agent's), `crates/model/tests/qa_regressions.rs` (also removes 3 `clone_on_copy` lints). On 18cadb8 + these:
`cargo test -p noshape-model` all green, `cargo clippy -p noshape-model --all-targets` clean, app verified.
The "For other agents" items were found on e4b90e2; re-check them against 18cadb8 (restyle round 2) before
forwarding.

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
   - Test: app-level (no unit test harness for dialogs). Verified in the app (worktree build): part right-click
     → Export… → Export shows the path prompt, writes the STEP, toast "Exported …"; Ctrl+O prompt still works.
     Clippy: no lints in files.rs.

3. **Shell silently gave bad or no geometry** on the bracket (Shell, top face removed, 2.5 mm): the app showed
   "Shell failed: operation failed: shell failed (thickness too large?)".
   - Cause (explored in `qa_regressions`): OCCT `BRepOffsetAPI_MakeThickSolid` depends on the join type.
     Block + 3 mm corner fillets: Arc join returns the right geometry but a BRepCheck-invalid solid (the app
     showed it as OK); Intersection fails. Block + fillets + 1 mm chamfer round the top: depending on thickness
     it throws, returns garbage (957 mm³), or returns the **input unchanged** with the top face still there.
   - Fix (`regen/features/detail.rs::valid_shell`): try Arc then Intersection; reject results that still
     contain a face to remove (`still_has_faces`); keep the first valid one, else a ShapeFix-repaired one whose
     faces are unchanged (`repaired_same_faces`, so names stay right); else fail with Onshape's "Could not shell
     part with selections.". Empty field messages: "Select faces to remove." / "Select parts to hollow.".
   - **Resolved (kern, 10-23):** the kernel's own shell (`shell_solid`: regions / offset / slabs) builds these, checked
     against Onshape's volumes (kernel_robustness S1, S2 and `shell_bracket_matches_onshape`). Onshape itself fails the
     real bracket at exactly 2.5 mm (the hole's offset wall touches the front wall's); we fail it the same way.
     Original note, for history: OCCT's offset fails outright when the
     shelled walls would merge or overlap. Repro on the real bracket (features copied with Ctrl+C, then
     `{"noshape_features": …}` into a test): 100x40x12 plate, 3 corner fillets (Width 5 mm), Ø10 hole about
     10 mm from the front edge, Shell top face 2.5 mm → both joins fail, and so does the closed Hollow offset
     (`BOPAlgo_AlertNullInputShapes`). Also a 1 mm chamfer round the removed face. Onshape (Parasolid) shells both.
     Needs a more robust shell (e.g. per-face offset + boolean trimming of overlapping walls); the feature now
     fails honestly instead of returning a wrong/invalid part.
   - Test: `qa_regressions::shell_is_valid_and_real_or_fails`.

4. **A mate broke when an unrelated upstream feature was suppressed** ("Revolute 1 has error: Mate connector
   could not be resolved (missing geometry)" after suppressing Linear pattern 1 in the Part Studio).
   - Cause: the pattern's (and the mirror's) holes cut the Ø10 hole's wall in two, so the rim edge the mate
     sits on was named after a split piece `[cap, Split{parent: wall, by, ordinal}]`. With the pattern
     suppressed the wall is whole (`[cap, wall]`); `Body::find_edges/find_faces/find_vertices` only accepted
     candidates that *descend* from the referenced names, never their ancestors.
   - Fix (`crates/model/src/regen/context.rs`): after the exact and descendant passes, fall back to faces that
     are *ancestors* of the referenced split piece (`edges_related` helper; same for faces and vertices). Onshape
     keeps such references.
   - Test: `qa_regressions::reference_to_split_pieces_survives_when_the_split_goes_away` (fails without the fix).
   - Verified in the app (18cadb8 + fix): Linear pattern 1, Mirror 1, Chamfer 1 suppressed → Revolute 1 resolves.
   - Found by reproducing the real document: features copied with Ctrl+C from the app (clipboard JSON) and
     regenerated in a test.

Fix 1 verified in the app too: Chamfer 1 edited to 5 mm turns red, model falls back to the valid part, title
tooltip "Chamfer 1 did not regenerate properly: Failed to chamfer selections." (Onshape's exact text).

## For other agents (not fixed by qa3; the lead forwards)

Found on main e4b90e2 (private worktree /root/noshape-qa3, because the shared tree didn't compile at the time).
**Re-checked on main 18cadb8** (worktree moved there): FIXED there — Enter/Enter commits (extrude/chamfer/fillet),
Variable list name "#t = 12 mm", tool search (now Onshape's popover with description + toolbar highlight),
feature-list toggle no longer clips the Extrude "Merge scope" chip. STILL PRESENT — fillet midpoint pick
(`midpoint_pick` unchanged), Fillet dialog old layout,
dropdowns without search/keyboard/highlight, Hole dialog (lone "Standard", bordered dropdowns, no spec title,
sketch-in-list doesn't add points), + menu, see-through Create Drawing modal, no Insert view after Create
Drawing. NEW: a new element tab is appended at the end; Onshape inserts it right after the active tab.
Assembly items not re-checked.
"Shared tree: unchanged" means I grepped the working-tree diff and the code is still the same.

### psb (Part Studio behaviour)
- ~~Feature-row clicks are additive, so Delete removes all selected~~: **matches Onshape** (live check,
  live/part-studio/110-114: plain click is additive, Delete removes all, one Ctrl+Z restores). Not a bug.
- **Hole: clicking a sketch in the feature list doesn't add its points** while "Sketch points to place holes" is
  active (the row just gets selected). Onshape uses all the sketch's points (like Extrude takes all regions).
- **Rolled-back features aren't italic gray while editing an earlier sketch** (double-click Sketch 1 with 9
  features after it: all rows stay normal). Onshape greys them (rollback-bar drag does grey them correctly).
- **Selection stays after editing**: after Sketch 1 edit is accepted, or after clicking Sketch 3 in the list
  for a Hole, the row stays highlighted (selected) indefinitely.
- ✅ (skb: sketch 14.47, plain numbers reopen as "80 mm"; the new-dimension box text carries the unit but may be clipped in the narrow box, to check in the GUI) **Dimension value box shows "80" without unit** (Onshape: "80 mm", value selected) when re-editing; the new
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
- ✅ n/a (live 91–95: Onshape's ✓ does commit and open the next mate) **Mate HUD ✓ commits and immediately opens "Revolute 2"** (accept-and-repeat). Onshape's ✓ closes the mate
  dialog; only Shift+Enter repeats. Please verify live and fix.
- **Insert dialog**: Part Studio and part rows show generic icons, not 48 px thumbnails; "Inserted: 0" while the
  first instance is following the cursor; a blue toast "Click to place the inserted instance (Esc keeps it where
  it is)" that Onshape doesn't show.
- **Triad** is coloured rings/arrows; Onshape's drag triad is gray-outlined (blue on hover).
- **Instance context menu** is 342 px wide with 28 px rows (Onshape: 196 px, 22 px rows, 12 px text) and lacks
  Properties…, Instance properties…, Show center of mass, Dynamic suppression ▸.
- ✅ (skb: row tooltips gated on `os_menu::is_open`, toolbar rich tooltips hide while a menu is open) **Stale tooltip over a context menu**: right-click the mate row → the row tooltip "Revolute mate / Offset: off"
  stays drawn on top of the menu (covers Rename/Edit…).
- ✅ (skb: live 300–318 show mate values only on the selected mate, never during/after a drag) **Drag readout "Rz -12.232°" stays in the viewport** after the drag ends (still there after Animate…).
- ✅ (skb: live layout, verified GUI v41) **Animate dialog**: no "Mate DOF to animate" field, Steps 60 (Onshape 300), Single/Reciprocate/Loop toggles
  (Onshape: Playback type ▾ Reciprocate + ↻), title "Animate Revolute 1" (Onshape "Animate mate").
- ✅ (asb 10-23, app check: + › Create Drawing… OK with No views opens Insert view with the part picker; ANSI_A_INCH.dwt row highlighted) **Drawing: Create Drawing from + doesn't open Insert view**; Onshape opens the Insert view panel with the view
  following the cursor. Template table: no default row highlighted.
- ✅ (asb 10-23, app check: Esc clears the view selection) **Drawing views stay orange after Esc** (QA2 reported it too).
- ✅ (asb 10-23: Section view dialog Vertical / Horizontal / Angular, label, Show cut geometry only, Cut angle, Section depth, Exclude from cut; `drawing_section.rs`) **Drawing Section view**: no dialog (Onshape: Vertical | Horizontal | Angular tabs, label "A", jog/flip, Show
  cut geometry only, Section depth); clicks just outside the view are silently ignored.
- ✅ (asb 10-23, app check: d + a click on a circle away from its snap points gives Ø, as live's tool help "Select … arcs and circles") **Drawing Dimension (d) doesn't take a circle** ("Select a point or a straight edge"); I believe Onshape's
  Dimension tool is smart (circle → diameter). Shift+D works. Please verify live.
- ✅ (asb 10-22: tab right-click › Export… opens the Export Drawing modal with live's formats and options) **Drawing export**: toolbar ⤓ ▾ PDF/SVG/DXF straight to a path. Onshape: tab right-click → Export… → "Export
  Drawing" modal (File name, Format ▾ PDF/DWG/DXF/DWT/SVG/PNG/JPEG/TIFF…, Version, Text, Color, Options).

### skb (sketch)
- ✅ (skb: sketch 14.39) **No live values while drawing**: corner rectangle shows no width/height, circle no Ø (Onshape shows them in
  `#449ccd` next to the preview).
- ✅ (skb: sketch 14.40) **Point tool makes filled dots**; Onshape drops hollow points.
- ✅ (skb: sketch 14.41) **Vertical dimension text sits on its dimension line** ("40" overlaps the line); horizontal ones break the
  line around the text.
- ✅ (skb: sketch 14.42, it stays selected) **Just-applied dimension has no tan background** (Onshape highlights it).

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
- **Keyboard focus is lost after a feature-row context menu** (18cadb8): right-click Chamfer 1 → Suppress, then
  Alt+C "shell" Enter did nothing useful (keys went elsewhere: a Measure panel opened); works after clicking
  the viewport.
- **Keyboard focus is lost after an in-app prompt closes** (and/or after creating a tab from +): F/Shift+7 do
  nothing until the viewport is clicked. Onshape shortcuts work right after a dialog closes.
- **Toolbar tooltips don't appear when moving directly from one button to the next** (only after entering the
  toolbar from outside). Onshape shows each after ~0.5 s.
- **Configurations panel** is a form, not Onshape's 712 px table (configurations as rows, configured parameters
  as columns, "Configure Part Studio ▾"). After Enter in a configured value the cell keeps keyboard focus, so
  shortcuts type into it ("20 mm!f").
- **Variable feature**: title "Variable 1" (Onshape "#? = 0 in", then "#t = 12 mm", also the list name); no
  Assigned | Measured | From import tabs; default value 25 mm (Onshape 0); Name isn't focused on open.

- **Toolbar has no overflow**: at 900x600 the Part Studio toolbar is just clipped after the Boolean tools.
  Onshape collapses the extra tools into an overflow menu.

### ui2 (start window)
- Create ▾ menu: "Document…, Folder…, Open file…". Onshape: Document…, Folder…, Import files…, Import from ▸,
  Label….
- Document list columns: Name | Modified | Location | Size. Onshape: Name | Modified | Modified by | Owned by.
- ui2 (18 Oct, 21:50): both fixed — Create ▾ = Document…, Folder… | Import files…, Label… ("Import from ▸" lists
  cloud drives: left out as cloud-only); columns Name | Modified | Modified by | Owned by ("me" for you).

## Performance (llvmpipe on Xvfb, so GPU-bound numbers are pessimistic)
- Regeneration is fast: full bracket (15 features) refine 26–64 ms after an edit; first evaluation of Mirror 1
  193 ms, Hole 1 40 ms, Linear pattern 1 64 ms; config switch ~400 ms preview + 64 ms refine.
- Frame latency dominates: `v.submit_to_present` mean 220–350 ms, `w.gpu_wait` ~110 ms, `v.frame_to_present`
  ~200 ms (UI thread late to take the frame; GPUI itself renders on llvmpipe). Mate animation runs ~3 steps/s
  here because each step waits for a presented frame. Re-measure on a real GPU before optimising.
- No stale-until-mouse-move frames seen in these sessions (view animations, undo/redo, config switch, drags).

## How to test
- Build: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-qa3 nice cargo build -p noshape`.
- Disk: after each build `find /root/target-qa3/debug/deps -maxdepth 1 -type f -executable ! -name '*.so' -mmin +60 -delete`
  (never delete the proc-macro `.so` files; if they're gone, remove their `.fingerprint/<name>-<hash>` dirs so cargo
  rebuilds them);
  keep debug/incremental under 2 GB (build with CARGO_INCREMENTAL=0).
- Worktree of main: `/root/noshape-qa3` (detached at e4b90e2 + my two fixes); build there with
  `CARGO_INCREMENTAL=0` to save disk; the app binary is copied to `/tmp/qa3/noshape-main`.
- Helpers in `/tmp/qa3/bin/`: `start.sh [args]` (DISPLAY=:106, HOME=/tmp/qa3/home, no native dialogs,
  NOSHAPE_PERF=1, appends to /tmp/qa3/app.log, `NOSHAPE_BIN` picks the binary), `stop.sh` (kills by PID: a
  `pkill -f` on the binary path also matches your own shell), `shot.sh name [crop]`, `x.sh` (xdotool on :106).
