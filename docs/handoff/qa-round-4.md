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

**R4 result (17:00–19:00, shared tree, lead-directed; in v0.4.2 e5e0b4b + a3d3490, the rest uncommitted):**
- `cached_slice.rs` `Slice<V>`; Part Studio slices: toolbar, **underlay** (list toggle, sketch / plane
  labels, graphics dialog) and **overlay** (view cube, strip, panels, menus...), with the **feature dialog
  between them** outside both (typing / blinking in a dialog field leaves them cached; z-order unchanged);
  the features list split into an uncached **head** (`render_feature_list_head`: config box, contexts, filter
  row, "Features (N)") and two slices, **rows** (`render_feature_list`) and **parts** (`render_parts_list`,
  sized `relative(parts_height)` of the panel by the studio, so the separator drag is unchanged). Workspace
  slices: navbar, tab bar, left rail (ui is adapting the tab-bar scroll arrows to it). Underlay, overlay, rows
  and parts observe the viewport and the doc; all slices are invalidated by any `cx.notify()` of the studio.
- The filter's Change handler doesn't notify the studio; it notifies the rows slice only when the set of
  matching features differs from what the rows last rendered (`filter_rows`), so "e" → "ex" → "ext" …
  re-renders just the head.
- Measured (`/root/qa4-scratch/measure2.sh BIN`: perf uprobes on `Window::draw` + UI-thread cpu-clock samples
  inside each call; one app at a time; dev profile = optimized + debuginfo), UI-thread CPU per frame, median:

| | base (dc0da0a) | v5 (+PS toolbar/overlay, Workspace slices) | v6 (+dialog layer, list slices) | v7 (+filter set check) |
|---|---|---|---|---|
| typing, every key changes the filter result (7 / BackSpace ×20) | 15.5 ms | 5.1 | 4.5 | 4.2 (5.0 on the keys that change the rows, 3.1 otherwise) |
| typing "extrude" and deleting it, twice | 15.2 | – | 3.0 (rows re-rendered on 27 of 61 frames) | **3.2** (rows on 7 of 54 frames, 3.25 on the others) |
| caret blink (filter focused, idle) | 16.5–19.8 | 6.0 | 2.75 | **2.9** |

  End to end (XTest key → paint on Xvfb) stays llvmpipe-bound: present (`X11Window::draw`) 90–300 ms wall on
  this loaded machine, only ~3 ms of it UI CPU; meaningless for a Mac. Idle redraws: none after a viewport
  click (focus leaves the field); with a field focused, 2 Hz caret frames now cost ~3 ms instead of ~17.
- Left over above 4 ms: a key that changes which features match rebuilds the rows (render 0.8 ms + their
  layout/paint, ≈ +2 ms; spread over ~10 rows' elements, nothing heavy in particular: filter_features,
  default_name, rollback bar ~0.01 ms each). Going lower needs each feature row as its own cached view (psb's
  list code). The ~3 ms floor is gpui's per-frame work (reusing cached views copies their primitives:
  memmove, BoundsTree inserts, PathVertex clones of the path-drawn labels; taffy for the uncached skeleton).
- Checked on v6/v7: filter (rows, "Features (1 of 7)", "...", ⊗ clears), editing Extrude 1 (dialog over the
  list toggle, rows rolled back, header buttons dimmed, Final un-rolls the rows, Depth 25 regenerates), Esc
  (first Esc reverts the field, second cancels: same on base), parts separator drag, layout identical.
- Build note for the lead: building a private worktree into target-shared corrupts shared artifacts. Cargo
  gives the vendored path crates (gpui-pre, gpui-base, …) the same unit hash in every tree, but rustc's crate
  hash differs per absolute path, so a worktree build overwrites `libgpui_base-*.rlib` / `libgpui-*.rmeta`
  that the shared tree's build then treats as fresh ("crate `gpui_base` required to be available in rlib
  format", "cannot find `ExternalTextureId` in `gpui`"). qa4 hit both; fixed by invalidating one fingerprint
  (`.fingerprint/gpui-base-8030dd…/lib-gpui_base`, original saved in /root/qa4-scratch) and touching
  `vendor/gpui-pre/src/gpui.rs` (mtime only). /root/noshape-qa4 is deleted; other worktrees on target-shared
  (lead, ui, asb, drw2) can cause the same.
- Status 19:10: v7 (`cargo build`) compiled with no warnings in qa4's files. `cargo clippy -p noshape
  --all-targets` is not run yet: the tree currently fails in drawing_palette.rs (another agent's
  in-progress borrow errors). Uncommitted from qa4: part_studio/mod.rs `filter_rows` + `filter_rows_changed`
  set check, behaviors.rs one call, this file. Scripts: /root/qa4-scratch (measure2.sh, ft2.sh, frames.py,
  flat.py).

### R5 (20:00–): filter case under 4 ms, then every tab (lead's task)
Method as R4; per-tab runs: `/root/qa4-scratch/measure3.sh BIN NAME "SETUP" "FIELD_X FIELD_Y"` (opens the tab
with xdotool, types 7 / BackSpace ×20 in the field, then idles 10 s with the caret blinking; UI-thread CPU per
`Window::draw`, median). Setups: Assembly = tab + instance filter; Insert = Insert dialog's search; Variable
Studio = new studio's Name cell; Feature Studio = end of line 1; Drawing = new drawing, one view, a note being
typed; documents window = its search box (`NOARGS=1`).
**Changes:**
- Part Studio rows: `render_feature_list` borrows the model instead of cloning the whole Part Studio (it
  plans the rows first, `ListItem`), and a row's hover text ("Extrude 1 <- Sketch 1", which scans the
  features above) is worked out when its tooltip opens (`ui::with_lazy_side_tooltip`). Per-row cached views
  were tried and removed: gpui reuses a cached view only at the same bounds (rows that move or come back
  after filtering are rebuilt anyway) and never inside a cached view it is re-rendering (see below).
- **Two gpui rules found on the way** (now in `cached_slice.rs`'s doc): (1) re-rendering a cached view sets
  `window.refreshing` for its subtree (vendor/gpui-pre/src/view.rs), so a cached view nested in one that
  re-renders is never reused; text fields must sit outside every cached view, with the heavy parts as
  sibling slices of a non-cached parent. (2) A view whose render read an input's state re-renders on each of
  its caret blinks. A cached view's layout comes from its style alone, so a slice needs a definite size.
- **gpui-pre `LineLayoutCache` keeps shaped lines across frames** (`RecentLines`, two generations of 4096,
  NOSHAPE PATCH qa4, noted in Cargo.toml). Before, a line not drawn in the previous frame was shaped again by
  cosmic-text (~0.3 ms a line): every row shown again after filtering, every changed label.
- **gpui-component `Button` boxes its base control, content style and loading icon** (NOSHAPE PATCH qa4): a
  Button was ~7.3 KB and moved by value at each builder call (`tooltip`, `on_click`, `into_any_element`,
  arena), 3–4 MB of memmove a frame for the drawing toolbar (found with a uprobe on libc memmove + the
  caller's return address).
- Assembly: cached toolbar, instance-list rows (head = config box, filter, "Instances (N)" rendered with the
  view), and the graphics overlay as `under` (list toggle, view cube) / uncached left dialogs
  (`render_left_layer`: Insert, Mate, … with their fields) / `overlay` (docked panes, notices, menus, strip);
  the Insert panel's catalog tree is a cached view inside the left layer; the list filter and the Insert search notify only their rows /
  tree (`filter_rows_changed`, `insert_search_changed`; the tree reads `ui.insert_query`, not the input: a
  view that reads an input entity is re-rendered on every caret blink).
- Drawing: cached toolbar. Documents window: the body (sidebar, list, details) is one cached view; the
  search notifies it. Feature Studio: `outline_of` caches the module outline by the text's hash (it was parsed
  in every render).
**Results (UI-thread CPU per frame, median, typing / caret blink; v17 = all of R5):**

| | base (dc0da0a) | v9 (R4 + Workspace slices) | v17 |
|---|---|---|---|
| Part Studio filter, every key changes the result | 15.5 / 16.5 | 4.75 on those keys | **3.75** on those keys (p90 5.0), 2.75 others / **3.0** |
| Part Studio filter, "extrude" typed | 15.2 | 3.2 | **3.0** (4.25 on the 7 keys that change the rows) |
| Assembly instance filter | 14.2 / 12.2 | 9.8 / 12.2 | **3.2 / 3.2** |
| Assembly Insert search | 17.5 / – | 14.5 / 10.8 | 4.5 / 4.1 |
| Variable Studio Name cell | 5.0 / 5.0 | 3.0 / 3.2 | **2.8 / 3.0** |
| Feature Studio editor | 10.2 / 9.8 | 7.5 / 7.6 | **3.8 / 2.5** |
| Drawing note | 12.8 / 12.0 | 10.2 / 10.2 | **4.0 / 3.0** |
| Documents search | 4.2 / 4.2 | 4.5 / 4.5 | **3.5 / 1.8** |

v17 adds: the editor caret patch (gpui-base `TextLayer` + `BlinkCursor` view, multi-line inputs only),
the Feature Studio toolbar slice, the drawing panel layer (`Slice::follow` on the note's field), the
Insert panel's sized slices (`Slice::sized`) around its search box, the Assembly left-dialog layer, and
the per-row views removed. Checked in the app: the editor caret follows typing and arrows, blinks, hides
on blur; the Insert panel (search, Current / Other documents, catalog, footer) lays out as before; the
Note panel shows over a note typed in place. gpui-base: clippy -D warnings clean.
**Left above 4 ms, and why:**
- Feature Studio: a caret blink repaints the whole code editor (gpui-base `TextElement`: prepaint 1.2 ms
  re-highlights and re-slices the visible text, paint 1.5 ms re-emits ~4000 glyphs), because the blink
  notifies the editor's `InputState`. Needs a gpui-base change (ui's component): the blink notifies a small
  caret entity drawn by its own element, and the text layer becomes a cached view. Proposed to the lead.
- Drawing: render 1.8 ms (13 `sync_*` dialog syncs + ~40 `render_*` dialog calls per render), layout 1.9 ms
  (two taffy passes); the canvas area holds the note editor, so it can't be one cached view without moving
  the editors out (asb's code).
- Part Studio, a key that brings back all rows: their render (~1 ms) + layout/paint; gpui can't reuse a view
  at new bounds.
- gpui's cached-view reuse (~1–1.4 ms a frame, every frame): split on a Part Studio blink with uprobes,
  `Scene::replay` is the largest part (each reused primitive is copied into the new scene and gets its
  draw order again from the `BoundsTree` R-tree, already upstream's optimized version), then the dispatch
  subtree copy (`reuse_subtree`) and `reuse_layouts`. Element-state keys are `Arc` clones (cheap). Paths
  are copied twice per reuse (`replay` clones the primitive, `insert_primitive` clones the path again);
  sharing a path's vertices (`Arc`) would halve that, but it is the renderer's type (perf), and perf's
  wgpu overlays (view cube, plane names) remove most paths. A real cut needs retained draw orders for a
  replayed range whose prefix didn't change: an upstream-sized change to gpui's scene, not done.
- Assembly Insert search: the panel's head (tabs, document row, filters; content-sized, so not a slice)
  re-renders with the view on each blink (~1 ms), and reusing the window's many cached parts costs ~1.4 ms
  (`reuse_prepaint` copies their hitboxes and element-state keys).
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

### R6 (10-21, after v0.4.3 GO): input behaviour, inventory gaps checked in the app (for ui, asb)
Inventory of all ~220 input sites (read from code): /root/qa4-scratch/input-inventory.md. Checked on the
v0.4.3-freeze binary under Xvfb against live's "Input behaviour" capture (behaviours.md "## Input behaviour").
Screenshots /tmp/qa4/s/t1*-t7* (not kept).

| # | Field (app) | App does | Live (behaviours.md) | Owner |
|---|---|---|---|---|
| 1 | Extrude › Depth, 1st click | caret at the click point | **selects all** (0..len); Tab into the field selects all too | ui (shared numeric field) |
| 2 | Extrude › Depth, "30" + **Enter** | commits, keeps the dialog open, shows "30" (selected) | appends the unit on Tab **and Enter** and re-selects: "30 mm" | ui |
| 3 | Extrude › Depth, "10+5" then refocus | refocus shows "10+5" plus an "= 15 mm" hint below | refocus shows **"(10+5) mm"** (no hint) | ui |
| 4 | Extrude › Depth, Esc while typing | reverts to the committed value, field **stays focused** | reverts **and blurs**; the dialog stays open | ui |
| 5 | Feature rename (list › Rename), Esc after typing | nothing; the next click away **commits** the typed name | **reverts** | ui (shared) / psb (rename sites) |
| 6 | Variable Studio value cell, Esc after typing "20" | nothing; clicking away commits "20 mm" | **reverts** | ui / ui2 (Variable Studio) |
| 7 | Variable Studio value cell, 1st click | caret in the text input | focuses the **cell** (no input); typing a key opens the editor with that key (type-to-edit) | ui2 |
| 8 | Drawing › Rename sheet panel, Esc after typing | nothing: the panel stays open with the typed text | Esc reverts / cancels (as every live field) | asb (drawing panels: `DrawingEditor::input`, most panel fields) |
| 9 | Part › Properties… dialog | opens with **no field focused**; typing a part number and pressing **Enter closes the dialog and loses the edit** (reopened: empty) | (Onshape's Properties modal keeps Enter as a field commit; must not drop edits) | ui (gpui_kit Dialog default `on_ok`) / owner of properties_dialog.rs |
| 10 | Feature list filter, Esc | text kept | text kept (✓ same) | — |
| 11 | Variable Studio value cell, bare "15" + Enter | "15 mm" (✓ same) | "15 mm" | — |
| 12 | Extrude › Depth "25"/"2 in"/"10+5" + Tab | "25 mm" / "50.8 mm" / "15 mm" (✓ same) | same | — |
| 13 | Rename / Rename sheet open | all selected (✓ same) | all selected | — |

Root causes (from the inventory): Esc is bound to Cancel only in `Workspace && !Input` and `FeatureDialog` contexts
(actions.rs:155-177) and the gpui-base input's own Escape (state.rs:2214) only clears with `clean_on_escape`; so outside
feature dialogs nothing reverts on Esc unless a site catches it. Numeric commit-with-unit lives in feature_dialog.rs
(Tab/blur) but Enter skips the reformat. Dialogs without `on_ok` close on Enter (gpui-base dialog.rs:437): Properties,
Version properties, Instance properties. Not yet checked in the app (code reading only, same causes): Esc in the other
renames (folder, default geometry, contexts, tabs, assembly rows), analysis/draft/flatten fields (plain-number parsing,
no expressions), Version/Instance properties Enter, focus-on-open of Document details.

### R7 (10-21, new qa4 session): Insert dialog layer, rows, Scene::replay, debug/release timings
State (keep current): **in progress.**
- **Assembly front layer (task 1).** The workspace draws the AssemblyView as a cached view plus, in front of
  it over the list+graphics area (under the toolbar), an uncached `front` Slice (`AssemblyView::front_host` /
  `render_front_host`): the left dialogs (`render_left_layer`: Insert, Mate, … with their fields), then the
  cached `overlay` slice, then the customize-toolbar Tools panel: the same z-order as when they were drawn in
  the view. The front layer has the "Assembly" key context, all the view's actions (`with_actions`, shared
  with the root) and the click-to-focus capture (own `front_focus` counted as inside). The view observes the
  doc. A keystroke / blink in an Insert or Mate field re-renders the Workspace root + the front layer only.
- **gpui-pre NOSHAPE PATCH qa4 (view.rs, window.rs `noshape_frame`):** a cached view re-rendering no longer
  forces the cached views nested in it to re-render (upstream `window.refreshing`; back with
  `NOSHAPE_NESTED_VIEW_REFRESH=1`). Without it, caching the whole AssemblyView would re-render all its slices
  on every instance-filter key. Safety: a cached view reuses its ranges only if it recorded them in the frame
  just drawn (`ViewElementState::frame`); inside a reused parent its indices are stale, so it re-renders.
  This changes rule 2 of cached_slice.rs ("no cached view inside one that re-renders") to: nested is fine,
  but a nested slice is only reused when its parent re-rendered in the previous frame too.
- Measured r7a (/tmp/qa4/noshape-r7a, before the focus fix below), UI-thread CPU per frame, median, typing /
  blink: **Insert search 4.0 / 3.0 ms** (v0.4.3 binary: 5.2 / 4.8). r7a broke the instance filter: the front
  layer's `track_focus` took every click's focus (gpui focuses the topmost tracked handle, then
  prevent_default), so the filter never got focus. Fixed in the tree (not yet rebuilt/measured): no focus
  handle on the front layer; its key context is "Assembly AssemblyFront" and the click-to-focus capture checks
  `window.context_stack()` for "AssemblyFront". **To do when perf resumes:** rebuild, re-measure asm (instance
  filter, must stay ≈3.2 / 3.2) and asmins; functional checks (Insert search filters the tree, Esc closes the
  Insert dialog, Mate dialog fields + Flip/Reorient keys, docked BOM pane over dialogs, context menu over the
  Insert dialog, Tools-panel customize drag, list splitter drag moves the dialogs); pixcmp.sh across tabs.
- **Paused 10-21 ~07:45 by the lead** for the core-workflow QA pass (docs/parity/core-workflow.md). Remaining perf
  items after task 1: Part Studio "all rows come back" frames (idea: rows clone their whole `Feature` incl. a
  sketch's data per render, `render_feature_row_view`; measure first), Scene::replay flag measurement +
  pixcmp.sh (`NOSHAPE_SCENE_REPLAY_ORDERS`, patch already in scene.rs/bounds_tree.rs), debug vs release timings.

### R8 (10-21 07:45–08:40): core-workflow QA pass (lead's priority change)
Checklist + findings: **docs/parity/core-workflow.md** (re-run before every release; 27 findings with owners,
sent to the lead). Scripts: /root/qa4-scratch/cwstart.sh, s.sh. Binaries: /tmp/qa4/noshape-r7a (07:26),
/tmp/qa4/noshape-cw (08:15, includes the Assembly front-layer focus fix: instance filter OK).
Next: perf R7 items (see above) unless the lead says otherwise; re-run the checklist when psb/ui land fixes.

### R9 (10-21 ~08:45–): perf resumed (lead: items 1–4)
1. **Instance filter after the focus fix** (/tmp/qa4/noshape-cw): 3.8 / 3.5 ms typing / blink (v0.4.3 3.5 / 3.5, load
   noise); Insert search 3.2 / 3.0 (v0.4.3 5.2 / 4.8). Both under 4 ms median.
2. **Part Studio rows.** Test doc /tmp/qa4/big/big.noshape (40 sketches + 20 variables, made with
   noshape-cli batch: /tmp/qa4/big/batch.jsonl); `measure4.sh BIN DOC NAME "240 149"` splits frames by
   render_feature_list. Baseline (cw): BackSpace that brings all 64 rows back **≈19–20 ms**, a key leaving 6
   matches ≈7 ms, caret 3.25. Model helpers are negligible (uprobes: filter_features, names, folders,
   dependencies <0.1 ms/frame); the cost is ~0.3 ms per real row (build + taffy + paint, dev profile) and one
   div per placeholder. Change (part_studio/mod.rs `render_feature_list`, behaviors.rs `list_scroll`): the list
   tracks a ScrollHandle; Feature/Folder/"..." rows outside the view (+2 rows) are left out, each run replaced by
   one empty block of its height (all those rows are ROW_H; other items always drawn, heights estimated). A
   wheel scroll notifies the rows slice (gpui div scroll → `cx.notify(current_view)`), which draws the newly
   visible rows before painting. v2 (per-row placeholders, 8 rows spare): all-rows frames ≈12–13 ms; scrolling
   checked (no gaps). v3 (runs merged, 2 rows spare): measuring.
3. **Scene::replay** (`NOSHAPE_SCENE_REPLAY_ORDERS`): `Scene::replay` itself is ~0.24 ms/frame with gpui's own
   replay (uprobe, Part Studio blink), so keeping orders saves ≤0.2 ms; whole frames unchanged within noise
   (blink 3.0–3.2 both, typing 3.5–4.0 both). `NOSHAPE_SCENE_CHECK=1`: 0 primitives out of paint order in 41
   checked frames (PS, Assembly, Insert, FS). pixcmp.sh on v2: **0 differing pixels in all 11 scenarios**
   (ps, ps-edit, ps-menu, ps-tip, ps-sketch, asm, asm-insert, vs, fs, dr, docs). Decision: keep it off (gain too
   small for a scene-order change); the patch stays for a later GPU-side look.
   **v3 had a bug** (each skipped run became a zero-height block: `mem::take` read after it zeroed the count;
   rows after the first run vanished, e.g. below a moved rollback bar). Fixed in v4 (`let run = take(...)`).
   v4 checked against cw: rollback bar ↑↑↑ at the list's end, rolled-back rows greyed below it, wheel scroll both
   ways, filter "3" ("..." runs), Default geometry collapsed: identical. **v4 vs cw back to back** (big doc,
   7/BackSpace ×20): frames that rebuild rows **16.25 → 8.75 ms** median (p90 23.25 → 11.5); caret frames
   3.6 / 3.5. Still above 4 ms in the dev build (app crate at opt-level 1): see item 4 for release.
4. **Debug vs release** (same tree, back to back; /root/qa4-scratch/m-relvsdbg.txt; UI-thread CPU per frame,
   median, typing / blink; binaries in /root/qa4-scratch/bin, symlinked from /tmp/qa4):

| | dev (v4) | release (thin LTO) |
|---|---|---|
| Part Studio filter (demo) | 3.8 / 3.0 | **3.2 / 2.5** |
| 60-feature list: frames that rebuild rows / others | 7.0 / 3.25 | **5.75 / 3.0** |
| Assembly instance filter | 3.8 / 3.5 | **3.0 / 3.0** |
| Assembly Insert search | 3.8 / 3.1 | **3.0 / 2.6** |

   Release is only ~15–20% below dev: dev already builds every dependency at opt-level 3 (only the app crate
   is opt-level 1). Above 4 ms in release: only frames that rebuild a long feature list (5.75 ms, ~14 real rows
   at ~0.2 ms each to build, lay out and paint). **Rule from the lead (10-21): no local release builds of the
   app** (this one held 5.6 GB for 30+ min); release timings come from CI artifacts. perf.yml has no typing
   benchmark and the artifacts are stripped, so CI needs an in-app per-frame UI-CPU log (e.g. a NOSHAPE PATCH
   in gpui-pre `Window::draw` reading CLOCK_THREAD_CPUTIME_ID under NOSHAPE_PERF=1) plus a typing/caret run
   in perf.yml; proposed to the lead. /root/qa4-scratch/keycpu.sh measures stripped binaries (main-thread
   task-clock per key) meanwhile.
   Gotcha: /tmp is tmpfs (RAM): keep test binaries in /root/qa4-scratch/bin, not /tmp.

### R10 (10-21 14:30–14:50): core-workflow re-run on the 14:32 build
Results table at the end of docs/parity/core-workflow.md: fixed F1, F3–F7, F10–F15, F17, F19–F25 (F19, F23
to verify against live); still failing F8, F9 (kern), F26 (kern), F27 (ui2); F18 unchanged (needs a live capture);
new F28 (Show errors defaults off), F29 (shortcuts dead after invalid dim value + Esc Esc), both psb.
Binary: /root/qa4-scratch/bin/noshape-cw2 (app run under `timeout 1800` by cwstart.sh). Perf leftover: release
row-rebuild frames 5.75 ms on a 60-feature list (gpui per-element cost of ~14 rows); options for the lead: reuse
row views at shifted positions (gpui-pre change) or slimmer feature rows (psb/ui row code).

## STATE FOR THE NEXT qa4 (10-21 ~19:10, moving from the Ngyekta to the Motile account)

**Read first:** this file's R7–R10 and this section, docs/parity/core-workflow.md (checklist + all runs), ARCHITECTURE.md
"Performance rules". Scripts: /root/qa4-scratch (cwstart.sh, s.sh, measure3/4/5/6.sh, ft2.sh, frames.py, pixcmp.sh,
rowexp.sh, keycpu.sh, prof2.sh). Test binaries live in /root/qa4-scratch/bin (symlinked from /tmp/qa4, which is RAM).
Big test doc: /tmp/qa4/big/big.noshape (60 features; regenerate with noshape-cli batch /tmp/qa4/big/batch.jsonl if /tmp
was wiped; the CLI is /root/target-engine/debug/noshape-cli). Rules learned today: no local release builds (CI only);
fresh-profile runs with `FRESH=1 cwstart.sh` (deletes the whole HOME; `rm HOME/*` kept .config prefs → false F28);
`pkill -f` only with anchored patterns (`^/tmp/qa4/noshape`), never a pattern from your own command line.

**Uncommitted from qa4 in the tree right now:** docs only (this file, docs/parity/core-workflow.md additions). All code
work (Assembly front layer, gpui-pre nested cached views, feature-list placeholders) is in main at 5d28447. The temporary
row-cost experiment was removed at 19:05 (reference patch /root/qa4-scratch/row-exp2.patch, includes others' edits).

**1. v0.4.5 release gate on 5d28447: FAIL** (sent to the lead 17:20; details in core-workflow.md):
- ✗ F12 new path (psb): Feature pattern of a Hole after a Part pattern → sliver part + facets; editing Extrude 1 then
  silently deletes Part 1. Repro in core-workflow.md.
- ✗ F8 chamfer / F9 shell / F26 STEP "SOLID" products + OCC header (kern).
- F27 footer OK in form; the hash in lead integration binaries is the stale /root/noshape-lead HEAD (told the lead).
- F18 closed (live: also empty without material). F19 differs (live toast spec now in core-workflow.md; psb/ui).
- F28 closed (my stale profile). F29 fixed. Everything else passes.
**Next:** re-run the failed steps (3.5, 3.6, 3.12/4.1, 7.2, F19) when psb/kern report fixes, and the **whole checklist on
every new integration binary** (copy /root/target-lead/debug/noshape after the lead's integrate; check its files equal
the commit with `cd /root/noshape-lead && git diff --stat <commit> -- . ':!.github'`).

**2. Slimmer feature rows (lead's decision; target < 4 ms in release on the 60-feature filter; look must stay
pixel-identical, coordinate with ui (look) and psb (behaviour) via the lead — message_agent only reaches agents you
started).** Findings so far (dev build, frames that rebuild rows, median UI CPU; caret-only frames ≈3.25):
- normal 8.1 ms; without the rollback-drop marker, the tooltip-anchor canvas, the eye, the drag/drop listeners (all
  four) 6.75 ms → those four parts are only ~1.4 ms. Individual parts are within noise (~0.3–0.9 each).
- floor sweep done (19:05, /root/qa4-scratch/m-rowexp2.txt; binary /root/qa4-scratch/bin/noshape-exp built from
  row-exp2.patch): normal ≈7.1, four parts off ≈7.1 (noise), **bare row** (also no icon, name, context menu) **≈4.9**
  vs caret floor 3.25. So even bare rows leave ~1.6 ms dev (~1.3 release) → ≈4.3 ms release with the 3.0 floor:
  **per-row slimming alone cannot reach < 4 ms**; the icon + name (which the look needs) are most of the slimmable
  cost (~2.2 ms dev for ~14 rows). Reported to the lead in the move message.
- Release ≈ 0.82 × dev for these frames (5.75 release vs 7.0 dev on the same tree), floor ≈ 3.0 ms release in the big doc.
  So < 4 ms needs the row delta ≤ ~1 ms release (~1.2 ms dev): slimming the four parts alone can't reach it.
**Next steps:** (a) wait for the lead's decision on the numbers above; options to put to them: fewer real rows (no overscan; overscan is
2 rows now) + the cheap removals below, and/or cutting the per-frame floor (head: filter_features runs ~5×/frame,
render_contexts_section walks every feature's params via referenced_contexts each frame: cache by studio revision).
(b) Cheap, look-identical removals worth doing anyway: drop-marker child only while a drag is active
(`cx.has_active_drag()`; a drag start refreshes the window), and replace the per-row tooltip-anchor canvas with one
`on_children_prepainted` on the list (ui::with_lazy_side_tooltip is only used by feature rows). Verify with screenshots
(rows normal / hover / selected / rolled back / suppressed / error / editing / rollback drag-over line) against the
current build, pixel-identical, and re-measure with measure4.sh. Release numbers must come from CI (perf.yml has no
typing bench yet: proposal to the lead pending: per-frame UI-CPU log in gpui-pre `Window::draw` under NOSHAPE_PERF=1 +
a typing run in perf.yml).

**3. Other open items:** perf leftovers (rows in release 5.75 ms on 60 features); proposal on CI typing bench; R6
input-behaviour audit of ~220 sites (/root/qa4-scratch/input-inventory.md) is still to be redone against live's
"Input behaviour" now that ui landed the shared field behaviour (most F-items above confirm it), one batched list to the
lead.

### R11 (10-21 ~19:30–, Motile account): feature-list row floor (lead's decision)
Lead: do the cheap look-identical removals + cut the per-frame floor; keep the 2-row overscan; no gpui row-view reuse.
Done in the tree (uncommitted, qa4):
- `ListCache` in part_studio/mod.rs (`list_cache: RefCell<ListCache>`): `filtered(cx)` = filter_features once per
  (query, display Weak ptr, document state); `referenced_contexts(cx)` (the walk over every feature's params) once per
  document state. Both dropped on any `DocEvent` (top of the doc subscription). Users: list head, rows, the filter
  box's Change handler (`filter_rows_changed`, `filter_rows` is now `Option<Rc<HashSet>>`), render_contexts_section,
  shown_context_referenced.
- Rollback drop marker built only while `cx.has_active_drag()` (a drag start refreshes the window: every row re-renders).
- No per-row tooltip-anchor canvas: `ui::with_lazy_side_tooltip(row, anchor, text)`; a feature row's anchor asks the
  list's ScrollHandle (`bounds_for_item(child ix) + offset()`, inset by the indent and the 5 px right margin for rows
  in folders) when the tooltip opens. The row gets its `slot` (child index, indent) from render_feature_list.
  (`on_children_prepainted` doesn't work on the list: with `track_scroll` gpui fills the handle's bounds instead.)
- Found: the hollow blue drop circle (live part-studio/30) **never showed** (before too): gpui gives an element with
  only `group_drag_over` styles no hitbox, so the style is never applied. NOSHAPE PATCH qa4 in gpui-pre div.rs
  `should_insert_hitbox` (+ `group_drag_over_styles`).
- Pixel check /root/qa4-scratch/rowcmp.sh (list, tooltip, selected, context menu, rollback drag-over, folder +
  indented-row tooltip, big doc tooltip, scrolled tooltip): before vs after **0 differing pixels in all 8**.
- Release reference from CI: the CI Linux artifact (ci.yml `cargo build --release`, with debug info, so the uprobe
  scripts work on it) of d5745c9, /root/qa4-scratch/ci/d5745c9/noshape (`gh run download 39641518601 -n noshape-Linux-X64`).
  measure4.sh on big.noshape: CI release 5.5–5.75 / 2.75 ms (rows / caret frames) vs dev "before" (19:32 tree) 8.0 / 3.25.
- Scripts: no pkill any more (new rule): qkill.sh REGEX kills by PID (pgrep + kill).
- **Clean A/B (10-22, one binary, temporary NOSHAPE_QA4_OLDROWS toggle, removed from the tree right after the build;
  binary /root/qa4-scratch/bin/noshape-ab, results m-r11ab.txt):** frames that rebuild rows, dev, median UI CPU:
  old 7.25 / 7.0 / 6.5 / 8.25, new 6.75 / 7.0 / 7.75 / 6.5 (≈7.1 → 6.9, inside the noise); caret frames 3.0–3.5 both.
  Call counts: filter_features 76 → 18 per 10 keys, referenced_contexts 0.29 → 0.02 ms/frame. CI ratio (d5745c9 CI release
  vs dev on these frames ≈ 0.70) → release ≈ 5.0 → 4.85 ms: **the < 4 ms target is still not met**; what's left is the
  per-row build/layout/paint of the ~14 visible rows (icon + name), which the lead ruled must stay (no overscan cut, no
  gpui row-view reuse). Drop circle checked in the app: now drawn at the drop line's left end, as live part-studio/30.
- **v0.4.5 gate re-run on 5bd50b2: FAIL (F8, F9 kern)**; F12, F19, F26 pass; new F30–F34 (core-workflow.md, end).

## STATE FOR THE NEXT qa4 (10-22 00:20)
- Code: nothing of qa4 uncommitted (row caches, drop marker, tooltip anchor, gpui group_drag_over hitbox are in 5bd50b2).
  The A/B toggle is gone from the tree. Docs: this file + core-workflow.md (gate re-run 5bd50b2) uncommitted.
- Next: re-run 3.5 / 3.6 when kern reports F8/F9, then the whole checklist on the next integration binary
  (/root/target-lead/debug/noshape; check `cd /root/noshape-lead && git diff --name-only <commit> -- . ':!.github'` is empty).
  Then R6 input audit items 6–8 (VS cell Esc / type-to-edit, drawing Rename sheet Esc) and the rest of the inventory.
- Binaries: /root/qa4-scratch/bin/noshape-5bd50b2 (gate), noshape-ab (row A/B). Scripts: rowcmp.sh (row pixel check),
  count.sh (filter/contexts call counts), blinkprof.sh, qkill.sh (no pkill).

### Rows target CLOSED (lead, 10-22)
Final numbers on the 60-feature list (frames that rebuild rows / caret-only frames, UI-thread CPU median):
dev 6.9 / 3.25 ms; CI release estimate ≈ 4.85 / 2.75 ms (CI release of d5745c9 measured 5.5–5.75 / 2.75 before the row
caches; release ≈ 0.70 × dev on row frames). Lead: 4.85 ms is well inside a 120 Hz frame (8.3 ms) and what remains is the
icon + name the look needs; no gpui row-view reuse, no overscan change. Don't reopen without a new lead decision.

### R12 (10-22 00:30–): input audit re-run on 5bd50b2 (against live "Input behaviour" + "Input behaviour outside feature dialogs")
**Pass now:** all R6 items 1–9 (Extrude depth click/Enter/(10+5) mm/Esc; feature rename Esc; Variable Studio value cell:
1st click focuses the cell, type-to-edit opens with the typed key, Esc reverts, click-away doesn't commit; drawing
Rename sheet Esc cancels; Part Properties opens with Name focused and Enter keeps the dialog and the edit). Esc reverts
in the other renames too: default geometry (Top), element tab, folder. Document details: no field focused on open (no
live capture); Enter saves and closes (no data loss).
**Deviation (one root cause, ~60 sites):** only feature-dialog numbers and the sketch dimension box use the Onshape field
behaviour (`quantity_field.rs` helpers wired by hand in feature_dialog.rs / sketch.rs). Every other Part Studio / assembly
numeric field (live: the same `os-param-number` control everywhere) lacks: "(1+1) deg" expression display, the invalid
state + hover ("Enter a valid expression." / counts "Enter an integer."), ↑/↓ (Shift ×10, Ctrl ×0.1) and focused-wheel
stepping, Enter re-selecting the value, and the blue focused underline (ours stays grey). Checked in the app: Draft
analysis › Minimum draft angle ("1+1" stays, "abc" not invalid, ↑ no step) and Zebra stripes › Stripe count ("1 in" not
invalid, ↑ no step); first click (select all) and Esc (revert + blur) are right on both. Site list (file:line): Part
Studio draft_tool.rs:33, analysis_tools.rs:241/260/286, thickness.rs:317, flatten_tool.rs:69, sketch.rs:1248/1270/1399/
1856/2072/2154, feature_dialog.rs:623 (tolerances), appearance.rs:212, material_panel.rs:149, config_panel.rs:127;
Assembly assembly_reflection.rs:31, assembly_mass.rs:74/480, assembly_view.rs:1606, assembly_panels.rs:279/943/1460/3003,
assembly_mate_dialog.rs:150, assembly_behaviors.rs:2478/2515/3460/3693, assembly_positions.rs:220, assembly_config.rs:238;
prefs graphics_prefs_dialog.rs:174, numbering_prefs.rs:64. Proposed fix (ui): one reusable quantity-input wrapper
(commit_text + step_text + invalid_field + wheel + revert_escape) and switch these sites to it.
**Drawings are different on purpose:** live's drawing fields are Wt widgets (caret at click, Esc does nothing in panels,
spinboxes step ±0.05 with a pale-orange "Must be a number" invalid state, sheet width appends the unit and Esc closes the
dialog). Our drawing fields opt out of select-all/Esc-revert, which matches that family; per-widget check still to do.
**Drawing panel fields checked (10-22, Drawing properties › Annotations, ANSI A inch sheet) vs live's Wt widgets:**
first click puts the caret at the click and Esc does nothing (both as live), but: Notes **Text height** is a plain field
"0.1378 in" (live: a dropdown, default **0.1200**, 4 decimals): "0.15" stays "0.15" (live "0.1500"), "3 mm" stays as typed
(live "3.0000": the unit is ignored), "abc" stays (live silently reverts). **Stack gap size** (live spinbox 0.0500): "0.06"
stays (live "0.0600"), "1 mm" not marked invalid (live pale orange rgba(221,82,40,0.1), hover "Must be a number"), ↑ doesn't
step (live +0.05). Owner: drawing (asb). Sheet properties width not re-checked.

### R13 (10-22 01:00–): `QuantityInput` wrapper + migration (lead: qa4 builds it; ui note in theme-layout.md)
- Wrapper: `quantity_field.rs` `QuantityInput` / `QuantitySpec` / `QuantityEvent::{Changed, Committed(CommitVia), Enter,
  Escape}` (see the theme-layout.md note). Feature dialogs and the sketch dimension box keep their own wiring (not a pure
  refactor: autofill, Create variable popup, per-path baselines).
- Migrated + built: draft_tool.rs (Minimum draft angle), analysis_tools.rs (Zebra count, Reflection rotation, Connection
  G1/G2/G3), thickness.rs (colour-bar bound, click-to-edit: caret on focus, Enter closes, Tab keeps, Esc closes),
  flatten_tool.rs (checker / distortion scale, offset, angle).
- Checked in the app (binary noshape-qty, 03:00) against live "Input behaviour outside feature dialogs": Draft angle all
  8 steps as live (select all + blue underline; 5 → "5 deg" selected; "(1+1) deg", unfocused "2 deg"; abc pink + "Enter a
  valid expression."; ↑ 6 deg, Shift+↑ 16 deg; Esc reverts + blurs). Zebra count: "20" kept selected; "1 in" / "1+1" pink,
  hover "Enter an integer."; ↑ 21. 
- Next (after the gate): Part Studio sketch fields (sketch.rs 1248/1270/1399/1856/2072/2154), feature_dialog.rs:623
  tolerances, appearance / material; then assembly (needs an assembly variable scope passed from AssemblyView for #vars;
  mate limit fields store "No minimum" as text: handle before migrating); prefs. Removed unused builder/setters to keep
  zero warnings; re-add `text()` / `set_text` / `set_units` when a site needs them.

## STATE FOR THE NEXT qa4 (10-22 ~06:50, new workflow)
- **Work only in /root/wt/qa4 (branch agent/qa4)**; /root/noshape is retired (/root/wt/WORKFLOW.md). Commit small, check +
  clippy + tests of touched crates, `git merge main`, `git branch -f ready/qa4 HEAD`; result in /tmp/lead/mq/qa4.status.
  Release gates: test /root/target-lead/debug/noshape (queue builds every merged main there; copy it first).
- **v0.4.5 gate on aa75fdf: PASS** (core-workflow.md) but the lead holds v0.4.5 for qa5's crash: Parts list › Rename
  (gpui "hover style already set", ui fixing). New checklist **section 8** (every feature-list / parts-list context-menu
  item, 73 rows). Dry run on aa75fdf: only 8.6.1 crashes. Runner: /root/qa4-scratch/menurun.sh BIN menus-ps.txt TAG and
  menus-fm.txt (folder / multi-select need setup; 5th field = setup steps; "hX Y" = hover a submenu row). Re-run section 8
  + the whole gate when the fix is merged.
- **QuantityInput migration** (R13): done + committed in the transition snapshot: draft, analysis tools, thickness, flatten.
  In progress (uncommitted in the worktree): assembly mate dialog offsets X/Y/Z + rotation (units + assembly variables
  passed from AssemblyView::open_mate_dialog via `variables::refs_variables`; limit fields still InputState). Next: sketch
  fields, feature-dialog tolerances, the other assembly panels (same variable scope), prefs.

### R14 (10-22 afternoon)
- v0.4.5: PASS on /tmp/lead/noshape-v045 (tagged by the lead). v0.4.6 gate on main 8da7e78: FAIL only on (b) (failed
  Extrude's input region not drawn red); findings: import selection highlight crosses parts (g48-two), Parts list order
  Part 2 above Part 1. Full table in core-workflow.md. Models for (a)/(b): /root/qa4-scratch/mk-models.py CLI DIR (needs a
  noshape-cli from the same main; build `-p noshape-cli` in the worktree, ~35 min).
- QuantityInput: mate dialog offsets X/Y/Z + rotation migrated (units + assembly variables passed from
  AssemblyView::open_mate_dialog; QuantityInput catches the dialog's Cancel so Esc reverts first). Checked in the app vs
  live (Fastened mate › Offset X, all 8 steps). Ready as part of 535b852.
- Builds are slow now (no incremental, unique codegen-units per worktree): check/clippy ≈ 30–38 min, tests ≈ 35 min.
  Use 2 h (7200000 ms) background limits; mark ready only after reading the clippy output (don't chain `&& ready`).
- v0.4.6 candidate (/tmp/lead/noshape-v046, 17:06): FAIL on (b) again; new R1 (two default planes selected → parts
  drawn with black jagged halos / see-through holes); F17 and F21 now fixed; footer still 0.4.4.8da7e78248bc. Table in
  core-workflow.md. Section 8 montage: /tmp/qa4/cm/mont.png.
- QuantityInput: assembly Linear / Circular pattern dialog Distance (Angle) + Instance count, both directions (3404e43),
  checked in the app against behaviours.md:5246 (count: 1+1 shown as typed, '1 in' / 'abc' invalid, ±1, wheel, Esc after
  typing reverts + blurs; distance '(1+1) mm' → '2 mm' unfocused). Open (not part of the batch, pre-existing): Esc on an
  *unedited* focused field there does nothing (the dialog has no "FeatureDialog" key context, so the kit's Escape
  propagates to nothing); with focus outside the fields Esc closes the dialog. Adding the context also binds Tab →
  DialogTab, which the pattern dialog doesn't handle yet.
- QuantityInput batches landed / ready (10-22 night, each checked in the app against behaviours.md:5240–5246): mate
  connector Move X/Y/Z + Rotation angle (hover names from mateConnector.fs: "X translation"…), Explode step Distance /
  Rotation angle (field rebuilt on Translation ↔ Rotation), assembly Zebra Stripe count (1–100, no expressions; Enter no
  longer closes the panel, as live) and Reflection Rotation angle, Animate mate Start / End / Steps (rebuilt on a DOF
  change). Skipped on purpose: the assembly triad value box (on-canvas box like the sketch dimension box; no live data).
- R1 (two default planes selected → black halos / see-through parts) is also on v0.4.5: not a regression.
- Remaining assembly InputStates: names, search, BOM cells (text fields); the Mass properties override field
  (assembly_mass.rs:74) is still to check against live before migrating.
- Migration status (10-23): every os-param-number field outside feature dialogs / on-canvas boxes that live has data
  for is on QuantityInput. Left, each needing a live capture first (request via the lead): (1) assembly Mass properties
  › Override value (assembly_mass.rs:74); (2) Appearance editor R / G / B / A / hex (appearance.rs:214–216: Angular
  quantity field or a colour-picker widget?); (3) Hole › tolerance Symmetrical / Deviation / Limits value fields
  (feature_dialog.rs:629: noshape draws a boxed kit input; dom/dialog-hole-tolerance.txt only has the Default type);
  (4) the assembly triad value box (assembly_panels.rs:1445). Sketch on-canvas boxes (dimension, OTF, offset, pattern
  count, Bezier degree) stay as they are (lead: not a pure refactor). Prefs have no numeric fields.
- Next: sketch fields (sketch.rs open_otf_input / open_count_edit / option_input / offset / bezier / tolerances),
  feature_dialog.rs:623 tolerances, the other assembly panels (same assembly variable scope), prefs.

## QA round 4 (lead 10-24): realistic user sessions on v0.4.7 (tag v0.4.7 = 30979bc1, /root/qa4-scratch/bin/noshape-v047b + noshape-cli-v047b), fresh profile
Live side: REQUESTS.md "qa4 (10-24, QA round 4: five user sessions)" (same recipes, answers in behaviours.md
"qa4 round 4 sN"). Scripts and screenshots: /root/qa4-scratch/r4/.

### Session 1: sheet-metal enclosure (r4/s1.py builds the model by CLI; drawing in the app)
Recipe: Thicken 100×60 on Top (t 1.5, r 1.5, K 0.45) → Flange 30 mm on the long edges + x=100 edge → Hem on the
y=0 flange's top edge → Front-plane circle Ø12 at (50, 1.5) Remove through all both ways → flat → drawing.

| Step | noshape v0.4.7 | Live |
|---|---|---|
| Sheet metal model | OK, 9000 mm³, 6 faces | pending |
| Flange 1 | OK, 20051.651 mm³, 28 faces | pending |
| Hem 1 | OK, 21804.734 mm³, 34 faces | pending |
| Extrude (cut through bends) | OK, 20547.157 mm³, 37 faces (the Ø12 at z 1.5 also cuts a 12 mm strip out of the base and both y bends) | pending |
| Sheet metal table | 4 bends: 3 × 90° R1.5 allowance 3.416 mm, hem 180° R0.01 allowance 2.152; all **Down** (= SM5, routed fs); flat 129.465 × 128.916 mm, area 13744 mm² | pending |
| Drawing › Insert view › Flat patterns filter | "Flat pattern of Part 1" with a flat thumbnail ✅; placed flat view with dash-dot bend lines and notes "DOWN 90.0° R1.5" / "DOWN 180.0° R0.01" | pending |

Differences so far (live numbers pending):
- **S1-a** the flat view in the drawing is turned 90° against the Sheet metal panel's flat (panel: flanges left/right/bottom;
  drawing: rotated). Which way live orients it: pending.
- **S1-b** a bend note sits on top of the cutout's arc ("DOWN 90.0° R1.5" covers the left half-circle, r4/s1-drawing-note-over-cut.png).
  Live's note placement on a cut bend: pending.
- Known, already routed: SM4 (panel design), SM5 (Down vs Up).

### Session 2: 3-part assembly (r4/s2.py builds parts, inserts, mates, exploded view by CLI; limits, drag and drawing in the app)
Recipe: Part 1 plate 100×60×10, Part 2 pin Ø10×40, Part 3 block 20×20×20; insert one each; Fastened 1 (plate to origin),
Revolute 1 (pin bottom centre to the plate top centroid), Slider 1 (block bottom to the plate top); Slider limits 0 / 50 mm;
exploded view (pin +40, block +20 along Z); drawing ANSI_A_MM with Suggested views, BOM table, balloons.
Note: my script mates the pin and the block to the same plate centroid (the request asks for (25,30) for the pin), so they
overlap; that doesn't change what was tested.

| Step | noshape v0.4.7 | Live |
|---|---|---|
| Insert ×3, mates | OK; Parts 2, 3 show the DOF glyph, tooltip "Has degrees of freedom (1)" ✅ (= live); new mates hidden, rows light grey, tooltip "Revolute 1 (Hidden)" ✅ (= live "a new mate is hidden by default") | pending |
| Slider limits | ☑ Limits shows Z "No minimum" / "No maximum" ✅; typed 0 / 50 stay **"0" / "50" (no unit)** → S2-a | pending |
| Drag past the limit | block stops at exactly 50 mm (instance z 60 = plate top 10 + 50), no message | pending |
| Exploded view | Explode 1 with 2 steps, dashed explode line, "Editing Explode 1 [Done]" banner ✅ | pending |
| BOM | Flattened, columns Item / Quantity / Part number / Description (= live's default), rows 1–3 | pending |
| Drawing | Suggested views: top, front, right, iso at 1:2 ✅; BOM table placed; balloons: plate 1, block 3, pin 2 (app click on the pin's silhouette) ✅ | pending |

Differences:
- **S2-a (fixed by qa4)** mate limit fields kept a bare number; Onshape's quantity field commits "50 mm" / "90 deg" like the
  offsets. `assembly_mate_dialog.rs` `commit_limit_text` on Enter / blur ("No minimum" untouched); test
  `limits_commit_with_units_like_onshape`.
- **S2-b (api)** `drawing_get` lists no silhouette edges of curved faces, so a script can't aim a callout at the pin; a
  `drawing_callout` anchor on the pin's centre line attached to the plate (balloon "1"). The app picks the silhouette (balloon
  "2"). → qa5-routing DRW-SIL.

### Session 3: surfaced handle (r4/s3.py, CLI; mass panel in the app)
Recipe: Ø20 circle on Top; Plane 1 offset 80; rectangle 30×16 on it; Loft (surface, from the two sketches); Fill on each
open end; Enclose; Fillet 1 mm on the start boundary.

| Step | noshape v0.4.7 | Live |
|---|---|---|
| Loft (surface) | OK, Surface 1, area 6058.8169 mm², **5 faces** | pending (follow-up request) |
| Fill 1 (Ø20 end) | OK, 314.212 mm² (exact disc 314.159265) | pending |
| Fill 2 (rectangle end) | OK, 480.000 mm² | pending |
| Enclose | OK, Part 1, 31807.0934 mm³, 7 faces | pending |
| Fillet 1 mm (start boundary, tangent chain) | OK, 31805.0156 mm³, 11 faces | pending |

Differences:
- **S3-a** the Mass properties panel (and `query_mass_properties`) show a **Volume 21217.66 mm³ for the open loft surface**;
  an open surface encloses nothing. Fill 2 / Fill 1 report 0. Live's panel for a lone surface: follow-up request; owner
  then psb/kern (model mass properties). r4/s3-surface-mass-volume.png.
- **S3-b** loft face count 5 (the circle's own seam stays as a fifth edge) and the planar fill's area 314.212 vs the exact
  disc: pending live.
- CLI note (not a defect): a surface loft takes `wireProfilesArray` (sketch curves), regions give "Select profiles in order
  from start to finish."; Fill `edges` are objects `{"entities":[…]}`.

### Session 4: configured part S/M/L, Derived (r4/s4.py builds it by CLI; switching and Derived in the app)
Recipe: List input "Size" (S/M/L; the default option renamed S), Variable #w = 40 / 60 / 80 mm, Sketch 1 rectangle with its
bottom dimensioned #w and 30 high, Extrude 1 depth 10 / 15 / 20 mm; Part Studio 2 › Derived of Part Studio 1, then M.

| Step | noshape v0.4.7 | Live |
|---|---|---|
| Configuration panel | Size S / M / L, switching S → L rebuilds the block 80 × 30 × 20 ✅ | pending |
| Variable row under L | **"#w = 40 mm"** (the stored value) → S4-a | pending |
| Derived (app) | Select Part Studio panel: studio row expands with Size ▾ + Generate; M → Generate → block 60 × 30 × 15, part "Part 1" ✅ | pending |

Differences:
- **S4-a (fixed by qa4)** a configured Variable's list row kept its stored value; it now shows the active configuration's
  value (`part_studio/mod.rs` row name from `Configuration::for_editing`; test `a_configured_variable_names_itself_with_the_active_value`).
- **S4-c (fixed by qa4)** a Derived whose Part Studio is missing ("Missing Part Studio") opened the picker drilled into that
  studio, showing "No Part Studios available."; it now opens the whole list (`select_item.rs` `picked_studio_exists`; test
  `a_missing_studio_opens_the_whole_list`).
- **S4-b (api)** `query_mass_properties` returns the default configuration's volume (12000 mm³ for S, M and L) after
  `config_activate`; the app's geometry does change. → qa5-routing CFG-MASS.
- **S4-d (api)** `feature_add importDerived` takes `partStudio: "Part Studio 1"` (a tab name) without complaint and the
  feature fails with "Error regenerating."; it needs the tab id. → qa5-routing CFG-MASS (same entry).

### Session 5: STEP round trip (r4/s5.py, CLI; same file as live: samples/qa4-s5-asm.step)
| Step | noshape v0.4.7 | Live (behaviours "qa4 round 4 s5") |
|---|---|---|
| Import (defaults) | tabs **"qa4-s5-asm"** (Part Studio) + file tab "qa4-s5-asm.step"; parts **Part 1, Part 3, Part 2**, placed where the assembly had them (x 0…110) | **"Assembly 1 (1)"** assembly tab + **"qa4-s5-asm parts"** Part Studio + blob in **CAD Imports** + "BOM : Assembly 1 (1)"; parts **Part 2, Part 3, Part 1**, each at its own product position (x 0…320) |
| Ø5 hole through Part 1 | **59803.650459 mm³** | 59803.65045915 ✅ |
| Rename "Edited" | Edited, Part 3, Part 2 | Part 2, Part 3, Edited |
| STEP export | AP242 Ed. 2 header, same CAx-IF lines and schema ✅; products Part 2, Part 3, Edited, no NEXT_ASSEMBLY_USAGE ✅ (order differs) | Edited, Part 2, Part 3 |
| STL binary mm Fine | header **"noshape-kernel st…"**, 652 triangles, x 0…110 z 0…50 | header **all zeros**, 604 triangles, x 0…320 z 0…40 |
| Parasolid | `APPL=noshape`, `SCH_1200000_12006`, no MC_MODEL/FRU/KEY/FILE/DATE lines (= EXP-HDR, api) | `APPL=Onshape`, `FRU=Parasolid 38.1.231`, `SCH_3801231_38101`, … |
| Default file name | (CLI: the given path) | the Part Studio name "qa4-s5-asm parts" |

### Live comparison of sessions 1–4 (behaviours "qa4 round 4 s1…s4", answered 10-24 ~02:00)
- **S1** live: Flange 1 20295.075 / 34 faces, Hem 22074.861 / 43, cut 20805.412 / 61 vs ours 20051.651 / 28, 21804.734 / 34,
  20547.157 / 37: the corner difference is **SM1** (fs, routed: live's walls run to the corner with rips "Joint B/C Rip").
  Flat **128.9165 × 129.4650** (x × y) vs ours 129.465 × 128.916: **S1-a confirmed**, our flat is turned 90° → FLAT-ROT (fs).
  Bend notes: live **one per bend-line segment** (the slot splits each long bend: 5 × "UP 90.0° R1.5" + 1 hem note), ours one
  per bend (3 + 1) → FLAT-SEG (fs). Live's notes also overlap each other and the slot, so **S1-b is not a difference**
  (withdrawn). Flat view auto scale live **1:3**, ours 1:1 → FLAT-SCALE (asb). The rest as live: "Flat pattern of Part 1"
  only with the Flat patterns filter on; the drawing tab right after Part Studio 1; obround slot outline.
- **S2** live: limit stop at exactly +50 mm, no message ✅; BOM columns ✅; balloon numbers ✅; exploded view ✅; drawing tab
  right after the assembly ✅. **S2-c** live leaders end in an **arrowhead on an edge** and a dot only inside a face; ours put
  a dot on every assembly-part leader → **fixed by qa4** (`scene/src/drawing_tools/pending.rs` `leader_dot`; test
  `callout_leaders_end_in_arrows_on_edges`). Live notes: "REST-made mates show (Hidden) … UI-made mates are normally shown",
  which contradicts the earlier capture ("a new mate is hidden by default", behaviours ~1268) → live asked (REQUESTS.md).
- **S3** live: loft **4 faces / 12 edges / 6060.134 mm²** (ours 5 faces, 6058.817), Fill 1 an exact **PLANE 314.159265**
  (ours 314.212, not planar), Enclose 31743.525 / 6 faces (ours 31807.093 / 7), Fillet 31730.804 / 10 (ours 31805.016 / 11)
  → LOFT-SEAM + FILL-PLANE (kern). **S3-a confirmed**: Onshape can't measure an open surface (Parts to measure stays empty,
  every value blank) → **fixed by qa4** (`regen/measure.rs` `mass_properties` counts solids only; test
  `surfaces_have_no_mass_properties`; also fixes the CLI).
- **S4** live: S 12000 / M 27000 / L 48000 = ours in the app ✅; the picker's configuration dropdown under the studio ✅; the
  configuration not in the feature dialog itself ✅; the derived part named "Part 1" ✅.

### In-app checks of the round-4 fixes (10-24 ~07:30, worktree build)
- **S2-c** ✅ a new Callout on the plate's edge in "Assembly 1 Drawing 1" ends in a filled arrowhead on the edge (the
  v0.4.7-made balloons beside it keep their stored dot). r4/s2c-callout-arrow-on-edge.png.
- **S3-a** values ✅ blank for the lone open loft surface. Follow-up found in the app: the Part tab still took the surface
  pick as a chip "Face of Loft 1"; live's field stays empty → **fixed by qa4** (`app/src/part_studio/menus.rs`
  `mass_parts`: the Part tab measures and lists the solids owning the selection, each once; test
  `mass_parts_tests::surfaces_never_enter_parts_to_measure`). Also from live's part-studio/2382: with nothing measurable
  Onshape keeps the **whole dialog** (Mate connector field, Show calculation variance, Mass/Volume/Surface area/Center of
  mass/inertia rows, all blank); ours collapsed to the one field → **fixed by qa4** (`MassView::blank`; test
  `nothing_to_measure_keeps_every_row_blank`; seen in the app, r4/s3a-mass-blank-dialog.png). Mass's Override is then
  greyed like Center of mass's (live 2382; ours was an enabled box) → fixed (`MassView::mass_override_enabled`, same test).
  Not changed, unverified: after a click on the surface ours keeps the face highlighted orange (it just doesn't enter the
  field); live says the click "adds nothing" without saying whether it highlights. Ours shows the "no material" toast with only a surface present; live's
  REST counts the sheet body in massMissingCount, so it may be right → live asked (REQUESTS.md), not changed.
- S2-a, S4-a, S4-c were seen in the app on the earlier worktree build.
- Incident (qa4, 07:14): `qkill.sh` run without its pattern matched every process and SIGTERMed the whole server (all
  agent sessions, Chrome, builds); reported to the lead. The script now refuses an empty pattern.
