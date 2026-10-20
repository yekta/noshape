# QA round 4 (qa4): "click everything" sweep

Why: the user found dead controls in v0.4.0 on macOS (Variable table rows, input padding, feature-list eyes;
psb and ui own those three). This sweep looks for all the others.

Setup: binary built from the shared tree, copied to /tmp/qa4/noshape, DISPLAY=:110 (1600x1000).

## Round 2 (lead, 09:20): small screens, re-check after integration, assembly/drawing in depth
Displays: :111 = 1440x900, :112 = 1280x800 (MacBook sizes); window resized to the screen with xdotool.
State 11:10: integration still not landed (the lead's run fails on asb's in-progress
`drawing::format::tests::numbers_and_tolerances`), so step 2's re-run was done on the 10:03 / 10:45 builds of the
shared tree; redo the three user reports once it lands. Verifying the input-padding fix proposal on a private
copy (/root/noshape-qa4; delete it after).
### R4 (14:00–): Part Studio typing latency (lead's task)
**For perf (b16dc34d, owns viewport.rs presentation / frame pacing, crates/render, gpu.rs):** qa4 changed
`crates/app/src/viewport.rs` once (in the shared tree, uncommitted, lead-approved): a `scene_bounds:
Option<(Vec3, Vec3)>` field computed in `set_scene`, replacing the 5 `self.scene.bounds()` calls
(`scene_radius` used by `project`, fit, zoom bounds). Reason: `project()` rescanned every vertex per projected
point (the in-plane labels project hundreds of points per frame). No further viewport.rs edits from qa4
without the lead. Tools you may reuse: /tmp/qa4/ft2.sh (perf uprobes on `Window::draw` / `X11Window::draw` +
UI-thread CPU per call), /tmp/qa4/tools/keylat.c (XTest key → XGetImage until the region changes).
Measured on the v0.4.1-era build (/tmp/qa4/noshape-r3), Xvfb :110, filter box, XTest key → XGetImage polling
(/tmp/qa4/tools/keylat.c, /tmp/qa4/lat.sh): **median 358 ms** (15 samples, 31–584).
Profile (`perf record` on the whole process, then the main thread alone, while typing):
- ~95% of CPU is Mesa **llvmpipe** threads (Xvfb software rasterizer drawing the window); the UI thread is
  ~4%. Most of the 358 ms is that rasterization + present, which a Mac GPU does in ~1 ms.
- **UI thread: 15–25 ms per frame** (dev build). Split: gpui element build/prepaint/paint 27%, allocation 21%,
  std/hash 18%, taffy layout 10%, kernel 9%, lyon path tessellation 6% (view cube + in-plane labels), app code
  4%, `Scene::bounds` 3.5%. I.e. the whole window's element tree is rebuilt, laid out and painted per
  keystroke: GPUI marks the notifying view's **ancestors** dirty (window.rs `mark_view_dirty`), and the app
  uses `.cached()` nowhere, so PartStudioView (and Workspace above it) fully re-render.
- **The window also redraws at 2 Hz when idle**: after typing in the feature filter and clicking the 3D view,
  the filter keeps keyboard focus and its caret keeps blinking (each blink = full-window frame). User-visible
  bug too: Shift+E then types "E" into the filter instead of opening Extrude. The Part Studio only takes the
  focus back from right-panel fields (`on_viewport_event`, `panel_focused`); the assembly
  (`focus_from_viewport`) and sketch mode already do it for any field.
- Wasted per-frame work: `Viewport::project` → `scene_radius` → `Scene::bounds()` walks every vertex of every
  mesh, once per projected point; `in_plane_text` (plane names, sketch name) re-runs `layout_text` and
  projects every glyph-outline vertex each frame.
Done: **viewport.rs caches the scene bounds in `set_scene`** (all 5 `scene.bounds()` uses).
14:40 lead: do (1)+(2) (done in the shared tree: `on_viewport_event` focus incl. feature dialog → dialog focus;
`plane_text` cache used by `in_plane_text` and sketch.rs's sketch-name width); hold (3) for ui's reply.
App-side frame metric: perf uprobes on `<gpui::window::Window>::draw` (layout+prepaint+paint, UI thread) and
`X11Window::draw` (present, includes waiting on llvmpipe), script /tmp/qa4/frametime.sh BIN SECONDS
typing|idlefocus (computes file offsets from nm + readelf; probes per binary).
Baseline (0839f5e-era binary): typing 10×(7, BackSpace) → Window::draw **median 22.2 ms**, p90 35.8, max 56.3
(27 draws); present median 342 ms. Idle after a viewport click with the filter still focused: **25 draws in
12 s** (2 Hz caret), Window::draw median 30.0 ms.
The shared tree doesn't compile (assembly_panels.rs, drawing_datum.rs, drawing_more.rs, model drawing/mod.rs,
other agents, since ~13:50), so before/after are built in a private worktree /root/noshape-qa4 at HEAD 53feac6:
`noshape-base` = HEAD, `noshape-v1` = HEAD + /tmp/qa4/mine.patch (only qa4's hunks).
Metric: /tmp/qa4/ft2.sh adds cpu-clock samples on the UI thread inside each probed call (CPU time, independent
of the machine's load, which was 30–55 from other agents' builds).
| (UI-thread CPU, 20 keystrokes) | before | after (1)+(2) |
|---|---|---|
| Window::draw per frame | 17.5 ms (p90 20.0) | 17.8 ms (p90 20.0) |
| └ render_viewport_overlay | 2.0 | 0.8 |
| └ render_toolbar | 2.0 | 2.2 |
| └ render_feature_list | 0.8 | 1.0 |
| X11Window::draw (present; wall ~330–360 ms on llvmpipe) | 3.5 | 3.5 |
| idle 12 s after a viewport click: draws | **24** (2 Hz caret) | **2** (the click) |
End-to-end keystroke (wall, llvmpipe-bound, load-dependent): 304–507 ms before, 452 ms after; meaningless
for a Mac. Behaviour checked on noshape-v1: Shift+E after a viewport click opens Extrude (filter keeps its
text); dialog Depth typed then viewport click commits "30 mm", Esc still cancels; editing Extrude 1 Depth →
viewport click → Enter commits (part regenerates, Undo enabled). `cargo clippy -p noshape --all-targets` on
HEAD + patch: no warnings.
Conclusion: (1)+(2) remove the idle redraw and ~1.2 ms of overlay work, but a keystroke still costs ~18 ms of
UI CPU because gpui re-lays-out and re-paints the whole window (~12 ms of the 18 ms is layout/prepaint/paint,
~5 ms is the Part Studio's element building). Only (3), cached child views, cuts that.
(3) prototype, private worktree only (no shared-tree edits until ui answers): `crates/app/src/cached_slice.rs`
(`Slice<V>`: renders a part of V through a fn pointer; observes V and optional other entities; inserted with
`Entity::cached(style)`), Part Studio slices `toolbar` (w_full × 36 px) and `overlay` (absolute, full size,
also observes the viewport). Step 2 of the prototype: the filter's Change handler drops its explicit
`cx.notify()` (the input's own notify already re-renders the Part Studio, its ancestor, with the new query,
leaving the slices cached).
Prototype results (UI-thread CPU per `Window::draw`, same scripts, back to back):
| | v1 = HEAD+(1)+(2) | v2 = + cached toolbar & overlay | v3 = + filter handler without notify |
|---|---|---|---|
| typing in the filter (20 keys) | 16.8 ms (p90 19.2) | 15.8 ms | **8.2 ms** (p90 10.0) |
| caret blink frame (field focused, idle) | 18.9 ms | 8.0 ms | **8.2 ms** |
| render_toolbar / render_viewport_overlay calls per typing frame | every frame | only on filter changes | **none** |
Functional checks on v3: Undo enables after Suppress and greys out again after Ctrl+Z (Redo enables); Shift+E
opens the Extrude dialog (overlay); arrow-key rotation moves the in-plane labels and the view cube (overlay
observes the viewport); Appearances panel opens and the strip moves; Sketch switches the toolbar to sketch
tools; toolbar hover highlight and the rich tooltip appear; the filter still filters. Esc not closing a sketch
after its plane is picked is the same on HEAD (not a regression). Also added after the measurement: both slices
observe the doc. Full patch (incl. (1)+(2)): /tmp/qa4/proto-slices.patch (HEAD 53feac6).
Next slices once ui agrees: the feature list's non-filter parts / parts list, the right strip and panels, and
the Workspace chrome (navbar, tab bar, left rail), which also re-render on every keystroke.
Planned (part_studio/mod.rs, ui's file, waiting for the lead to forward to ui):
1. `on_viewport_event`: a press in the graphics takes focus from any field (not sketch mode, which does its
   own), so the caret stops and shortcuts work.
2. `in_plane_text`: cache the laid-out polylines per name.
3. The real fix for "a keystroke only redraws what changed": render the big, rarely-changing parts
   (toolbar, view cube, right strip, viewport overlays / labels, Workspace chrome) as **cached child views**
   (`Entity::cached(style)`), each observing the entity it reads (its own notify invalidates it), so a
   keystroke in a field re-renders only the feature list (when the filter changes) or the field itself.
### R3 (13:10–13:45): retest on v0.4.1 / current tree (0839f5e + 3 uncommitted files), :110
Method: click a point, wait, type `7`, wait 1 s, crop (script /tmp/qa4/probe.sh; points on every corner, edge,
padding strip and the text area).
- **Input padding** ✅ everywhere tested: Part Studio filter box 12/12; Part Studio Variable table Name cell
  12/12 (incl. the old dead strip 832–838); Variable Studio Name cell 12/12 (incl. 542–546); documents search
  box 12/12 (I-beam over the padding, ▾ keeps the pointer and opens advanced search); advanced search Name
  field 6/6.
- **Hide/show eyes** ✅ (fast taps): Default geometry Top, Sketch 1, Part 1, assembly instance and mate eyes
  toggle both ways. Solid features (Fillet 1) have no eye, as in Onshape.
- **Variable table** ✅ (Part Studio panel): Name row + Enter adds one variable (`#wq = 0 mm`), Value cell
  click in its padding + `5` Enter commits, Type ▾ (Length/Angle/Number/Any), row menu on every cell (Filter
  feature list, Delete), header ⌄ collapses, ⋯ (Copy table, Collapse all tables). Two parity gaps:
  - ✗ (psb, Variable Studio: ui2) **after Enter in the Name add row the focus doesn't move to the next empty
    Name row** (live behaviours.md psb 10-20 o: "Focus moves to the next empty italic Name add row"): typing
    the next name goes nowhere.
  - ? a bare `5` in a Length value shows `5` / `#wq = 5` (Onshape probably `5 mm`; live check requested).
- **My earlier "intermittent misses inside the text area" were not misses**: in the Part Studio the typed
  character appears 150–630 ms after the key (measured with /tmp/qa4/tools/keylat.c: XTest key + XGetImage
  polling), so a screenshot 0.5 s later sometimes raced it. Same measure in the Variable Studio tab: 40–110 ms.
  With NOSHAPE_PERF=1 no viewport frames are rendered per keystroke (`ps.refresh_scene` ~0.05 ms), so the cost
  is the Part Studio window's own layout/paint. On Xvfb that composite runs on llvmpipe, so the real-hardware
  number is unknown. **For ui/lead (perf):** measure keystroke-to-paint in the Part Studio on a Mac; if it's
  above ~16 ms, profile `PartStudioView::render` (whole feature list / panels rebuilt per keystroke?).
### Notes from the freeze (11:45–13:05)
- frame_hit fix verified on private build /tmp/qa4/noshape-fh (claim every enclosing frame): Part Studio
  filter box padding x 166–174 now takes the click (was dead 166–172). Proposed diff = FrameHitArea::prepaint
  loop over all frames containing bounds.origin, cloning press/release (Claim is Clone).
- Inside the text element itself: intermittent misses, 2/24 on the private build (both at the start of a
  run), 1/12 earlier; not tied to the key typed or to a 1.5 s settle time. Open; to investigate.
- Clippy at freeze (11:4x): clean for qa4's files; warnings in drawing_create.rs (2), drawing_jog.rs,
  drawing_table_tool.rs, part_studio/config_panel.rs (other agents).
- 13:09 GO: v0.4.1 = 0839f5e (includes ui's nested-frame fix from the prototype); private copy and test binaries
  deleted.
### R2 fixed
- (none yet) Window size on small screens is fine: `Bounds::centered` already fits the window into the
  display's visible bounds (macOS: minus menu bar and Dock), so a 1280x800 Mac gets a 1280x≈775 window.
- **Long toolbar dropdowns ran off small screens** (1280x800: the collapsed Plane group's 26 tools; the menu
  slid up over the navbar and Query variable, Composite part and Tag were still below the window, no scroll).
  Fix: `ui::fit_tool_menu` (gpui-kit PopupMenu `scrollable` + `max_h` = window height − 84 px) on the Part
  Studio tool groups and both sketch dropdown kinds (`part_studio/mod.rs`, `part_studio/sketch.rs`).
- **Esc didn't close the assembly Animate mate dialog** (see 3a). Fix: one branch in the Esc chain. Verified on
  the 10:45 build ✅.
### R2 checklist (1280x800 on :112; 1440x900 = round 1's window size)
- Window size: the document window fits the screen (`Bounds::centered` clamps to the visible bounds) ✅
- Part Studio: toolbar (collapsed groups) ✅, feature list, viewport tools, right strip, 712 px panels ✅
- Assembly toolbar fits at 1280 ✅
- Hole dialog, Graphics preferences (621 px) ✅ fit; Create Drawing modal ✅; sketch Line ▾ (22 tools) just fits
- **Tab bar overflow** ✗ (ui): at 1280 the tabs after "Custom features" (Flange, Drawing 1 …) are off the
  window and there are no tab scroll buttons; Onshape has `document-tabs-scroll-button` left/right (live
  dom/cursors-controls.json `tab_scroll_left`; details requested in REQUESTS.md). Only the tab manager and
  Ctrl+Space reach them now.
- **Feature Studio toolbar clipped** ✗ (fs/ui): at 1280 "→ ▾" (Forward reference) and "Line N, Column N" are off
  the window; at 1440 "Line 1 , Column" is already cut.
- Drawing toolbar at 1280 (10:45 build): collapses into its 9 groups and fits ✅ (round 1's clipping fixed);
  the sheet, sheet tabs, right strip, title block all visible ✅
- Test-env note: on the new Xvfb :112 every cursor reads as the X11 root "X" (app cursors don't load there);
  cursor checks stay on :110, where they're normal.
### R2 step 2: the user's three reports + qa4's fixes (10:03 build of the shared tree; integration not landed
yet, the lead's run is failing `sheetmetal_frames::convert_box_with_bends_and_joints`; redo after it lands)
- **Hide/show eyes** ✅: Default geometry Top, feature row (Sketch 1), Part 1, assembly instance, mate: each
  toggles on a fast tap and on a slow click.
- **Variable table (Part Studio panel)**: rows render; Enter in the italic Name row adds one variable ✅; row
  menu on cells ✅ (psb uses `os_context_menu_over_inputs`). ✗ **Clicks on the empty Name cell still fail on an
  8 px strip at its start** (x = cell left + 4 … + 10, the empty input's own text element; 4/4 misses at each of
  832–838 px vs 4/4 hits at 830 and ≥842 on :110). The frame leaves presses inside the text element to it,
  and it doesn't focus. That's exactly where a user clicks on the "Name" placeholder. (ui + psb)
  Same class elsewhere: the **Variable Studio add row** (`variable_studio.rs`, `input_frame` + kit `Input`) is
  dead on its left 5 px (x 542–546, cell left = 541), live from 548. Pattern: inside an app `input_frame`, a
  few px at the left of the kit `Input` (its own padding / the empty text element) take no press. Suspect
  `frame_hit.rs`: the outer frame skips presses inside `claim.inner`, but the kit Input's padding there isn't
  handled by the text element either (the kit frame's probes don't claim because the outer frame already
  has). Repro script idea: click a point, type `1` (no shortcut), crop, compare.
  **Root cause (qa4, for ui; not edited because frame_hit.rs changed at 10:43):** with an app `input_frame`
  around a kit `Input`, only the outermost (app) frame gets the claim (`FrameHitArea::prepaint` →
  `frames.first_mut()`). A press in the **kit frame's padding** (BaseInput `input_px`) bubbles first to the
  kit frame's own `.track_focus(&frame_focus_handle)` (gpui-component input.rs:744): gpui focuses that
  *frame* handle and calls `prevent_default()`. The app frame's handler (registered in `InputFrame::paint`
  before its child, so it runs later in bubble) then returns on `window.default_prevented()`. Result: the frame
  handle has focus, the text input doesn't, keystrokes go nowhere (or to shortcuts).
  Suggested fix: in `FrameHitArea::prepaint` give the claim to **every** frame on the stack that contains the
  text element, not just the first. The kit frame then inserts its hitbox and registers its press handler at
  `probe_start` paint, after BaseInput's own listeners, so in bubble it runs before `track_focus`: it focuses
  the input and prevents default; the outer app frame then skips (no double press); presses in the app box
  outside the kit frame still go to the app frame. Affects every wrapped site (Part Studio filter box,
  Variable table cells, Variable Studio cells, …).
- **Input padding** ✗ (ui): Part Studio "Filter by name or type" box: its left padding (x 166–172 on :110)
  is dead, and one click on the text area (x 184) missed; text from x≈174 works. Not every input has the
  frame fix yet.
- qa4's 11 fixes: all still in the tree (code checked); tooltip placement and Measure-without-selection
  re-checked in the app ✅.
### R2 step 3a: assembly in depth (10:03 build, --demo-drawing-full document)
- Instance menu, all 23 items ✅: Hide / Hide other / Hide all, Show center of mass, Isolate… (panel),
  Make transparent… (panel), Suppress, Fix, Use best tessellation, Show mates (mode + banner), Edit in context
  (switches to the Part Studio, "Assembly 1 displayed" bar, Go to assembly ✅), Add mate connector…, Replace
  instances…, Switch to Part Studio 1, Copy (toast), Move to new subassembly, Change to version… (Reference
  manager), Export… (modal), Zoom to selection, Create Drawing of Part 1… (template modal), Add selection to
  folder… (Folder name popup), Create empty subassembly, Delete; Ctrl+Z restores each.
- Other menus ✅: mate row (Rename … Expand ▸ Collapse ▸ Delete), assembly top row, Origin, empty viewport;
  mate Edit… (mate dialog), Edit named position driver mate… (Driving mate selector), Animate… (Animate mate);
  the empty list area opens no menu (to check live).
- **Esc didn't close the Animate mate dialog** ✗ → fixed (qa4, R2 fix 2): `assembly_view.rs` Esc chain gains
  `anim` (close_animation), after the mate dialog.
- Mate row Reset: no visible effect on this demo (mate at its default); not judged.
- Named positions: row menu (Apply / Edit / Duplicate / Update / Delete) ✅, Add named position (row + editing
  banner, Done) ✅
- BOM ✅: BOM type ▾ (Flattened / Structured), Add column ▾ (17 properties), ⋯ (Copy table, Show
  excluded/suppressed, Show top level assembly row, Export to CSV), header right-click (Move left/right, Remove
  column), row right-click (Switch to Part Studio), cell edit (Enter → next row). After Esc closes a menu the
  last clicked cell keeps its blue outline (looks like editing; minor, skb).
- Docked panes now move the strip to the pane's left edge (skb changed it since round 1) ✅
### R2 step 3b: drawing in depth (10:03 build)
- Drawing toolbar now collapses into groups (asb/ui2 fixed round 1's clipping) ✅
- View menu, 5 submenus ✅ (Show/hide 9 items, Tangent edges Hidden/Solid/Phantom, Order, Align views,
  Select for replication View/Region…); Show hidden lines ✅, Align views › Vertically (prompt) ✅
- View menu items ✅: Adjust linestyle… (dialog), Insert custom table… (dialog), View properties… (dialog),
  Switch to Part Studio 1, Move to sheet… (dialog), Copy (toast "Copied 1 view"), Clear selection, Zoom to fit,
  Delete (Ctrl+Z restores); Paste as table and Suppress alignment with parent: no visible change (to check
  with suitable content)
- **Sheet tabs drawn over the Adjust linestyle dialog's title** ✗ on the 10:03 build ("Sheet1 inAestylembly…"),
  even though `render_sheet_tabs` hides the tabs while `linestyle` is set; asb rewrote the sheet bar /
  canvas row at 10:34 (`drawing_view.rs`, `drawing_extra.rs`), re-check on the next build.
- Sheet tab right-click ✅ (Rename…, Properties…, Duplicate, Add sheet after, Move left / right, Delete sheet);
  Rename sheet popup ✅, Sheet properties dialog ✅, Add sheet after ✅, Move left ✅, Ctrl+Z restores ✅
- Dimension right-click ✅ (Edit…, Paste as table, Unbreak dimension, Clear selection, Zoom to fit, Delete);
  double-click opens "Dimension properties" ✅ (checkbox ✅, Esc closes ✅). It's a small form (Prefix/suffix,
  Precision, Units, Tolerance, Override text); Onshape's double-click editor is the 405 px one with text
  above/below, precision ▾, tolerance ▾ and the symbol row (behaviours.md "Dimension (d)") ✗ (asb)
- Note: right-click ✅ (Edit note…, Add leader, Explode note, Copy, Move to ▸, Paste as table, Clear selection,
  Zoom to fit, Delete), double-click → Edit note ✅
- GD&T frame: right-click ✅ (Add leader, Remove leaders, Edit…, Copy, Move to ▸, …), double-click →
  Geometric tolerance dialog ✅
- Hole table: right-click ✅ (Format cell…, Merge…, Size ▸, Resize…, Precision ▸, Flip X/Y, Fixed corner ▸,
  Text height ▸, Fit to content, Delete table); double-click only selects it (live check requested)
- Title block: double-click → "Title block" fields dialog ✅; right-click gives the sheet menu (live check
  requested)
- Toolbar: all 9 group ▾ open with every tool (views 18, dimensions 14, annotations 4, notes/tables 8,
  centerlines 6, sketch 11, constraints 6, insert 3, inspection 2) ✅; formerly unreachable tools now work:
  Trim and Sketch point (prompts), Hatch region (dialog), Horizontal (prompt), Insert image (modal), Format
  painter (dialog + prompt), Inspection item (dialog + prompt) ✅
- View tools: Section view dialog ✅ (Vertical / Horizontal / Angular tabs + Cut angle, jog and flip toggles,
  Show cut geometry only, Section depth → Blind / Up to entity + Depth), Auxiliary view (dialog: Normal / In
  line / Axis), Break view (dialog: Horizontal / Vertical, Break line, Gap), Detail view (dialog), Broken-out,
  Jogged, Crop, Datum target (pick modes with prompts) ✅. Detail view's "View scale" shows empty (Onshape
  shows the scale, e.g. 2:1) ✗ minor (asb).
- Linestyle dialog no longer overlapped by the sheet tabs on the 10:45 build ✅ (asb's 10:34 change)
- Empty paper right-click ✅ (Paste as table, Sheet properties…, Drawing properties…, Zoom to fit / window /
  sheet)

## State
- **Done (09:10).** The sweep ran on the 06:35 binary (the shared tree didn't compile from 06:55 to ~08:30:
  asb's drawing `cuts`, then ui's `input_frame` → `InputFrame` change). qa4's fixes were verified on a private
  copy (/root/noshape-qa4, rsync of the shared tree, built in target-shared; delete it when done: `rm -rf
  /root/noshape-qa4`). At 09:00 a fresh rsync of the shared tree passes `cargo clippy -p noshape --all-targets`
  with no warning from qa4's files (remaining: assembly_panels.rs, drawing_jog.rs, part_studio/config_panel.rs,
  model custom/host/hole.rs ×2, all other agents' in-progress files).
- 08:40: **all qa4 fixes verified in the app** on the private build (/tmp/qa4/noshape-p2): Appearances row
  click + editor beside the panel, Measure with no selection (no crash), bottom-right tooltips above their
  buttons (Left/Right tooltips unchanged), Versions Compare with only Start, Open menu no longer under the
  row tooltip, BOM pane fits, drawing View properties beside the Sheets panel, Variable Studio single add and
  row menu on cells, view cube cursor. Compiles with no warning from qa4's files (the build's 2 warnings: the
  private instrumentation, since removed, and `drawing_more.rs::open_broken_out_panel` unused, asb's).

- Files qa4 changed (all small hunks; nothing committed): crates/model/src/regen/measure.rs (guard + test, in
  psb's rewritten file), crates/app/src/ui.rs (PlacedTooltip, MenuAwareTip), os_menu.rs
  (`os_context_menu_over_inputs`), variable_studio.rs (add-row clear, row menu), history_panel.rs (Compare),
  part_studio/appearance.rs, part_studio/view_cube.rs (cursor + now-unused FluentBuilder import removed),
  assembly_bom.rs, drawing_props.rs (one line, in asb's file), drawing_keys.rs (one line).
  `cargo test -p noshape-model --lib empty_selection_measures_nothing` passes.

## Checklist (✅ = responds and matches Onshape; otherwise the bug)

### Part Studio: feature list
- Filter box: click on the text ✅; click in its padding ✗ (known, ui)
- Default geometry eye (hover → click): selected the row on 06:35 (the user's bug); ✅ on the current tree,
  also for fast taps (psb's fix)
- **Features header Add folder** ✗ (psb): live (behaviours.md "Features header New folder button",
  part-studio/585–590): always enabled, tooltip "New folder", and a "Folder name" ✓✕ popup opens first with
  "New folder" selected; ✓ with nothing selected makes an empty "New folder (0)" at the rollback bar, with
  Extrude 1 selected it wraps it into "New folder (1)" (collapsed, selected). Ours: no popup, "Folder 1", and
  nothing at all with an empty selection (`feature_ops.rs::add_to_new_folder` returns early).
- Pause regeneration ✅ (bar: Regenerate features and exit ✅); Regeneration times ✅ (popup × ✅)
### Part Studio: toolbar
- All 9 split carets open their menus ✅; menu item → dialog ✅ (Revolve)
### Feature dialog (Revolve)
- Solid/Surface/Thin tabs ✅, New/Add/Remove/Intersect ✅, checkbox by its label ✅, tolerance toggle ✅,
  tolerance chevron (inert until a tolerance is set, like Onshape) ✅, tolerance-type dropdown ✅, × ✅
- Tooltip "Tolerance type" stays drawn over the open dropdown list ✗ (ui)
### Right panel strip
- Appearance, Configurations, Custom tables, Inspection, Variable table buttons ✅ (712 px panel, strip moves) ✅
- Appearances: Parts / Sketches tabs ✅; **single click on a part row did nothing** ✗ → fixed (qa4);
  double click on the swatch ✅ opens the editor, but **the editor sat under the panel** (its ✓ ✕ hidden, no
  way to close it) ✗ → fixed (qa4); Esc doesn't close the editor ✗ (to check live)
- Configurations: Configure Part Studio ▾ (List/Checkbox/Configuration variable) ✅, rows switch ✅,
  input ⋯ ✅, panel ⋯ ✅, section chevron ✅, Delete ✅
- Custom tables: Add custom tables popup ✅, tabs ✅, Custom table samples › back ✅, × ✅
- Inspection ("Model definitions") ✗ (psb): ⋯ and the ① are plain icons with no handler
  (`config_panel.rs::render_inspection`). Live (behaviours.md "Inspection table panel", part-studio/591–599):
  ⋯ opens an empty dropdown; the tool row is Datum (dialog, label A), Dimension ▾ [Dimension, Fillet,
  Minimum, Maximum dimension] (pick mode), Geometric tolerance (dialog), Weld symbol (dialog) | Set
  coincident, Set parallel, Set text alignment (pick modes).
### Viewport
- View cube: faces ✅, arrows ✅, rotate arcs ✅; camera menu ✅ (Isometric ✅, Shaded with edges ▸ ✅,
  Graphics preferences ✅: tabs, colour swatch popover, ×)
- Bottom-right tools: **Measure with nothing selected crashed the app** ✗ → fixed (qa4, see Fixed 3);
  **their tooltips showed ~600 px to the left of the buttons** ✗ → fixed (qa4, see Fixed 4)
### Left rail
- Versions and history, Create version, Document notes, Performance, Release drafts ✅
- Versions panel: Create release candidate ✅ (dialog Close ✅), Create version… ✅ (description, checkbox label,
  Cancel ✅); **Compare with the workspace did nothing** when the document has no versions but Start ✗ → fixed
  (qa4, Fixed 5); click on Start opens it ✅ but with the old chrome ✗ (psb: "History" + yellow "VIEWING
  HISTORY" badge and the full toolbar; live navigation/300–302: grey "Start" label, white "Versions are view
  only. Viewing Start." bar with Return to Main, tools hidden); click on Main returns ✅
- Versions panel: right-click a version row ✅ (Open, Branch…, Compare, Restore to Main…), **but its row
  tooltip "Open version Start" was drawn over the menu's first item** ✗ → fixed (qa4, Fixed 6);
  right-click on the Main row "opened no menu" while Start was viewed: the menu (one item, "Open") was hidden
  under the row's tooltip → fixed by Fixed 6 (verified: the Open menu shows)
### Tab manager (bottom-left), tabs, + menu
- Tabs panel: search toggle ✅, Sort tabs ▾ ✅, Filter toggle ✅, List / Detail view ✅, row click selects +
  preview ✅, Toggle preview ✅, ×  ✅; Enter opens ✅
- + menu ✅ (Create Material Library … Import…); element tab right-click ✅ (Delete, Rename, Properties…,
  Duplicate, Copy to clipboard, Move to document…)
### Feature Studio
- New feature ✅ (inserts the template at the caret) but **Ctrl+Z doesn't undo it** ✗ (fs: typed text undoes;
  Ace keeps snippet/template insertions in its undo stack; the inserted placeholders aren't selected either)
- Snippet dropdowns (Feature parameters, Query, Evaluate, Math, Sketch, Operation) ✅; Import ✅; Format ✅;
  Commit ▾ (Commit / Commit all) ✅; Monitor Part Studio ▾ ✅; Module outline ▾ (to check)
- **Import popover stays open** on Esc and clicks elsewhere (only its × closes it) ✗ (fs/ui2; live check
  needed)
### Assembly
- Instance eye (hover → click) ✅ hides/shows; mate row chevron ✅, mate eye ✅
- Toolbar: all 25 buttons respond ✅ (Mate connector, 10 mates, Group, Snap mode, Show mates, Replicate,
  Replace, patterns, mirror, relations, Display states, Create Part Studio in context)
- Instance right-click menu ✅ (Properties…, Instance properties… dialog: typing enables Save ✅, Close ✅)
- **"Select an instance to replace." banner stays until its ×** (Replace instance with nothing selected; Esc and
  other tools don't clear it) ✗ (skb: Onshape's notices go away when another tool starts / Esc)
- Right strip: BOM, Configurations, Exploded views, Named positions, Variable table ✅;
  **BOM pane ran off the window** (fixed 712 px inside a dock capped at half the graphics area: the strip sat on
  top of it and "Add columns" was cut) ✗ → fixed (qa4, Fixed 7). For ui2/skb: the Part Studio docks 712 px and
  moves the strip to the panel's left edge (live part-studio/77); the assembly docks at most 50% of the graphics
  area with the strip at the right, which differs from Onshape at 1440 px.
### Drawing (--demo-drawing-full)
- Sheets panel toggle ✅, sheet rows switch sheets ✅, chevrons ✅, Sort by sheet / reference ✅
- Toolbar carets: section views ▾ ✅, Dimension ▾ ✅, Circle ▾ ✅, Rectangle ▾ ✅, Spline ▾ ✅
- **Toolbar clipped at 1440 px** ✗ (asb/ui2/ui, high: 1440 is a common MacBook width): everything after
  Spline ▾ (Sketch point, Hatch region, Trim, Horizontal ▾, Insert DXF/DWG, Insert image, Format painter,
  Inspection item, Hide inspection items) and "Search tools…" are off the window and unreachable. The Part
  Studio and sketch toolbars collapse into category groups (live part-studio/220–221); the drawing doesn't.
- All 17 tools left of Note: Insert view ("Select a part or assembly") ✅, Projected/Auxiliary/Section/Aligned/
  Broken-out/Detail/Break/Crop views ✅ (dialog or pick mode, beside the Sheets panel), Dimension ▾ tools ✅,
  Datum, Geometric tolerance, Surface finish, Weld symbol ✅
- Hovering the icon half of a split tool highlights only the icon; Onshape highlights both halves `#d7d7d7`
  (ui2)
- Right strip: Inspection table (View ▾, ⋯ Export to CSV ✅), Styles, Drawing properties, MBD status ✅
- View: click selects ✅, right-click menu ✅; **View properties… opened under the Sheets panel** (only its ✓ ✕
  showed) ✗ → fixed (qa4, Fixed 8). Also the drawing Measure readout box (bottom-left) → same fix.
  With the dialog beside the Sheets panel, the panel's list-toggle icon (bottom-left of the panel edge) is
  drawn over the dialog's "View label" row ✗ (asb/ui2; Onshape: dialogs above everything).
  For asb/ui2: in Onshape the Sheets panel pushes the canvas (live drawing/30: the sheet moves right and the
  panel toggle sits at the panel's edge); ours overlays the left of the drawing.
### Variable Studio
- Name add-row: click on the text ✅, in the cell's padding ✗ (known, ui/psb)
- **One Enter in the add row created the variable twice** ("cd", "cd", the second red "defined more than
  once") ✗ → fixed (qa4, Fixed 9). **psb: the Part Studio Variable table's `cell_input` has the same shape**
  (commit on PressEnter and again on the Blur that follows); check its add row.
- Variable type ▾ ✅, title ⋯ (Copy table, Rename, Collapse all tables) ✅, row right-click on the type cell
  (Delete, Move up/down) ✅; **on the Name/Value/Description cells the input's Cut/Copy/Paste menu showed
  instead** ✗ → fixed (qa4, Fixed 11). Live (behaviours.md "Variable Studio cell right-click"): always the row
  menu; the Description cell adds Copy and Convert to expression (ui2: those two items are still missing).
  psb: the Part Studio Variable table can use the same `os_context_menu_over_inputs`.
- Insert into all Part Studios and Assemblies ✅, ?▲ popover ✅, Insert Variable Studio dialog ✅ (Close ✅)
- New tabs are appended at the end; Onshape inserts after the active tab (qa3, still open)
### Documents page (start window)
- Create ▾ (Document…, Folder…, Import files…, Label…) ✅, Owned by me / Recently opened / Created by me / Trash
  ✅, Create label ✅, search ▾ advanced search ✅, + Add column chooser ✅, Name / Modified sort ✅, row click →
  Details ✅, row right-click menu ✅ (the right-clicked row isn't selected; Onshape selects it), Details × ✅,
  user menu (Preferences…) ✅
### Part Studio rows (right-click)
- Feature row menu ✅ (Dynamic suppression ▸ Add expression…, Show dependencies… panel ✅); the Dependencies panel
  stays after Esc / clicks elsewhere (only × closes it; to check live)
- Part row menu ✅; Properties… dialog ✅: input padding click focuses the frame but typing goes nowhere (known,
  ui); **Category is a static grey box** ✗ (psb, `properties_dialog.rs:391`): Onshape's is a picker with an
  input and a ▾ caret (live dom/dialog-assembly-part-properties.txt `osx-category-picker`)
### Document navbar
- ☰ menu ✅ (Rename document … Close document), ? menu (Keyboard shortcuts, About) ✅, {·} FeatureScript notices
  panel ✅ (× closes it; Esc doesn't)
- **With the notices panel open the Part Studio feature list shows no features** ✗ (ui): the Parts section
  keeps its height and the feature tree gets 0 px (only "Features (7)" + "Parts (1)" remain). Onshape's
  panel is also about half the window (live document/75) but the list keeps both sections.
### Cursors (XFixes cursor dump on :110, /tmp/qa4/tools/curdump.c; live dom/cursors-controls.json)
- ✅: right-strip and left-rail icons, inactive tab, Search tools…, Features header icons, list toggle (pointer);
  feature rows (default); dialog ✓ ✕ (default), option tabs, checkbox labels, dropdowns, (?) (pointer), number
  field (I-beam), dialog header (grab)
- ✗ (ui) **arrow instead of pointer** on every gpui-kit `Button`: toolbar tools (Sketch, Extrude…), the tool ▾
  carets, the + and tab-manager buttons, navbar ☰ and ?, the camera ▾ button. Cause: gpui-component's `Button`
  render calls `cursor_default()` unless the variant is link/text (`gpui-component-0.7.3/src/button/button.rs`
  ~655–663), so a `.cursor_pointer()` on it or on a parent can't win. Fix needs a pointer-capable button
  (vendor the button or use a div button in `ui::icon_button` / `text_tool_button` / `tool_caret`).
- ✗ (psb/ui) Default geometry / Parts expander chevrons: arrow (Onshape pointer)
- ✗ document name in the navbar: arrow (Onshape text; click behaviour requested live)
- ✗ view cube faces / arrows: pointer (Onshape default) → fixed (qa4, Fixed 10)
### macOS code paths (read, not run)
- Shortcuts: `secondary-` (⌘ on Mac) for Undo/Redo/Copy/Paste/New/Open/Save copy/Close/Quit and the Feature
  Studio's Commit (⌘S), Commit all (⇧⌘S), Search (⇧⌘F) ✅; Ctrl-only ones (Ctrl+M mate connector, Ctrl+Space,
  Ctrl+Q drawing update, Ctrl+arrows) match Onshape's list ✅; Ctrl+Y and ⇧⌘Z redo ✅.
  **Missing Feature Studio shortcuts** ✗ (fs): ⌘/Ctrl+⇧O (Module outline), ⌘/Ctrl+[ and ⌘/Ctrl+] (back /
  forward reference), from Help "Keyboard shortcuts".
- Ctrl+click = right click: converted in `vendor/gpui-pre-macos/src/window.rs::handle_view_event` (all
  `os_context_menu`s get it) ✅; two-finger click arrives as a native right press ✅
- Retina: viewport picking scales by `window.scale_factor()` (viewport.rs pick radius, readback coords), drawing
  hit-tests too (drawing_view.rs) ✅
### Code scan: controls drawn without a handler
- Part Studio Inspection panel ⋯ and tool icon (above, psb)
- **Assembly Configurations panel (configured)**: "Filter inputs" is a bordered div, not an input, and its
  table / checkbox / variable / ⋯ icons have no handlers (`assembly_config.rs:759–763`) ✗ (skb; the Part
  Studio panel's equivalents work, `config_panel.rs:677–690`)
- **Part Studio Variable table header** ⌄ and ⋯ are static glyphs (`config_panel.rs:318–327`) ✗ (psb, who owns
  the Variable table now)

## Fixed by qa4
1. **Appearances panel: a click on a part row did nothing** (Onshape selects the part: row with a 2 px
   `#1651b0` outline and `#def1fb` cells, part highlighted, live part-studio/173). Cause: the row's `on_click`
   only handled double clicks. Fix: `part_studio/appearance.rs` single click → `pick_body`, selected look.
2. **Appearance editor hidden under the right panel** (double-click a swatch: the editor opened at x=340 of the
   viewport, so the 712 px panel covered its right half with ✓ ✕; it couldn't be closed). Fix: dock it at the
   viewport's right edge, beside the panel, when a right panel is open (live 173).
3. **Crash: the app panicked** (`measure.rs:277 index out of bounds: the len is 0`) right after clicking a
   bottom-right viewport tool with nothing selected. Cause: psb's in-progress `measure_shapes` rewrite indexes
   `shapes[0]` / `shapes[1]` unguarded. Fix: `crates/model/src/regen/measure.rs` uses `first()` / `get(1)`;
   test `regen::measure::tests::empty_selection_measures_nothing`. **psb: keep this if you rewrite the file.**
4. **Tooltips placed beside the wrong spot near the window edge** (bottom-right Measure / Analysis / Mass
   tools: "Show analysis tools" drawn ~600 px left of its button). Cause: `ui.rs::PlacedTooltip` centred the box
   in a fixed 600 px strip; near the right edge gpui's `anchored()` (SwitchAnchor) flipped the whole strip to
   the other side. Fix: the box is measured on its first (hidden) frame, then placed centred on / beside its
   element and clamped into the window (Above, Below, Left; Right already snapped). ui: FYI, tooltip code.
5. **Versions panel "Compare with the workspace" did nothing** with no versions (only Start). Cause: it compared
   `versions.last()` only, and ignored the viewed row. Fix (`history_panel.rs`): compare the viewed row
   (version, Start or history point), else the latest version, else Start.
6. **Row tooltips drawn over right-click menus** (Versions panel: "Open version Start" covered "Open"; same for
   every `ui::tooltip`, 191 call sites). Cause: gpui shows the hovered row's tooltip after its delay even though
   a menu opened on top. Fix: `ui::tooltip` wraps the tooltip in `MenuAwareTip`, which draws nothing while an
   os_menu is open (PlacedTooltip already did this).
7. **Assembly BOM pane cut off** (see Assembly above). Fix: `assembly_bom.rs` `bom-pane` gets `max_w_full()`.
8. **Drawing View properties dialog under the Sheets panel.** Cause: it used `left_2()`, unlike the other 25
   drawing dialogs that use `dialog_left()`. Fix: `drawing_props.rs` (one line) and the Measure box in
   `drawing_keys.rs::render_measure`.
9. **Variable Studio: Enter added the new variable twice.** Cause: the add row commits on `PressEnter` and on the
   `Blur` that follows, and the old input still held the name. Fix: `variable_studio.rs::cell` empties the add
   row's input when it commits.
10. **View cube showed the pointer cursor**; Onshape keeps the default (the cube is part of the canvas). Fix:
    `part_studio/view_cube.rs` (also used by the assembly) drops `cursor_pointer`.
11. **Variable Studio: right-click on a cell opened the text input's menu** instead of the row menu (Delete,
    Move up/down). Cause: the input handles the right press and stops it before the row's bubble-phase
    `os_context_menu`. Fix: new `OsContextMenuExt::os_context_menu_over_inputs` (os_menu.rs, capture phase)
    used by the Variable Studio rows; reusable for every table whose cells are inputs.


## For other agents (the lead forwards; repro on the current tree unless noted)

### ui (chrome, inputs, cursors)
- **Cursor: arrow instead of pointer on every gpui-kit `Button`** (toolbar tools and ▾ carets, + and
  tab-manager buttons, navbar ☰ / ?, camera ▾). Cause and fix: see Cursors above (gpui-component `Button`
  forces `cursor_default()`).
- **Feature list collapses to nothing with the FeatureScript notices panel open** (navbar {·}): the Parts
  section keeps its height. Onshape keeps both sections.
- Tooltip "Tolerance type" drawn over the open tolerance-type dropdown (Revolve › Thin › Thickness tolerance).
- Expander chevrons (Default geometry, Parts) and the navbar document name: cursor (pointer / text in Onshape).
- FYI: `ui.rs` changed by qa4 (`PlacedTooltip` placement, `MenuAwareTip` in `ui::tooltip`), `os_menu.rs`
  (`os_context_menu_over_inputs`).
### psb (Part Studio behaviour)
- **Features header Add folder**: no "Folder name" popup, wrong name, dead with nothing selected (details in
  the checklist; live part-studio/585–590).
- **Inspection panel**: ⋯ and tools are static icons (live part-studio/591–599 lists the 7 tools).
- **Part Studio Variable table**: header ⌄ / ⋯ static; check the add row for the double add (Fixed 9) and use
  `os_context_menu_over_inputs` for the row menu on cells (Fixed 11).
- **Part properties › Category** is a static box (Onshape: picker).
- **Viewing a version (Start)** uses the old chrome ("History" + yellow badge, full toolbar); Onshape:
  grey version label, "Versions are view only. Viewing Start." bar with Return to Main, tools hidden
  (live navigation/300–302).
- `measure.rs`: keep qa4's guard and test (Fixed 3) in your rewrite.
- Appearance editor: Esc doesn't close it (live check requested).
### skb (assembly)
- **Assembly Configurations panel (configured)**: fake "Filter inputs" box and handler-less icons
  (`assembly_config.rs:759–763`).
- "Select an instance to replace." notice stays until × (Replace instance with nothing selected).
- Docked panes (BOM, Configurations, Named positions, Exploded views) are capped at 50% of the graphics area
  with the strip on the right; the Part Studio (and Onshape) dock 712 px and move the strip to the pane's edge.
### asb / ui2 (drawing)
- **Drawing toolbar clipped at 1440 px** (high): the tools after Spline ▾ and "Search tools…" are unreachable;
  needs the category collapse the Part Studio has.
- Sheets panel overlays the canvas (Onshape pushes it); its list-toggle icon draws over dialogs beside it.
- Split tool hover highlights only one half (Onshape both, `#d7d7d7`).
- `drawing_props.rs` currently calls the private `drawing_more.rs::open_broken_out_panel` (E0624 at 08:55; your
  in-progress edit).
### ui2 (Variable Studio, studios' visuals)
- Variable Studio Description cell menu lacks Copy and Convert to expression (live "Variable Studio cell
  right-click").
- Documents page: right-click doesn't select the row.
### fs (Feature Studio)
- **New feature isn't undoable** (Ctrl+Z after New feature leaves the template); placeholders not selected.
- Missing shortcuts ⌘/Ctrl+⇧O, ⌘/Ctrl+[ and ⌘/Ctrl+].
- Import popover doesn't close on Esc / outside click (live check requested).

### hover-revealed controls and fast clicks (checked, OK on the current tree)
On the 06:35 binary (before `ui::hover_reveal`), a click that lands before the app has handled the pointer's
move onto the row (`xdotool mousemove X Y click 1`, i.e. a trackpad tap or quick flick) missed the hover-only
control: the assembly instance eye selected the row instead, the Preferences shortcut pencil did nothing.
On the current tree the same fast tap works on the Part Studio plane eye, the feature-row eye and the assembly
instance eye: the opacity-0 + group_hover approach keeps their hitbox and listeners, so `hover_reveal` does fix
this class. Use it for every hover-only control (no `invisible()`).

### Open live requests (REQUESTS.md, qa4): Esc / outside-click behaviour of Insert dialog, Appearance editor,
Dependencies panel, notices panel, FS Import popover, Replace notice; navbar document name click.
