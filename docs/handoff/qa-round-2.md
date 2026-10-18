# QA round 2 handoff

State: the shared tree compiles (`cargo build -p noshape`, target `/root/target-qa2`).
`cargo test --workspace --exclude noshape`: all tests pass (last run after the render, units and model changes below).
`cargo clippy --workspace --all-targets -D warnings`: no lints in files/lines touched by QA2. The remaining
lints belong to other agents' in-progress code: assets.rs ICON_CACHE type, section.rs too many args,
scene_builder.rs vertex-color types, sketch.rs (`match` → `if let` at the dim-input subscription, `.fold`),
sketch_mode.rs collapsible `if`.

## Bugs fixed (symptom → cause → fix → test)

1. **Sketch regions on a part face couldn't be picked** (the whole face highlighted).
   - Cause: in the pick pass, pick-only meshes were drawn in the "transparent" list with no depth writes, so the opaque face drawn afterwards always won.
   - Fix (`crates/render/src/renderer.rs`): new `pick_priority` draw list for pick-only meshes, drawn after the opaque meshes with pipeline `mesh_pick_priority` (depth write on, `PICK_PRIORITY_DEPTH_BIAS` = +4 constant). Opaque faces carry `FACE_DEPTH_BIAS` (see 2), so a region lying exactly on a face wins. Parts in front of a region still win.
   - Test: `crates/render/tests/offscreen.rs::pick_only_region_on_face_wins_the_pick`. It fails without the fix.
   - Verified in the app: a circle region on the top face highlights alone; outside it, the face highlights.
2. **Edges looked dotted** (back edges and silhouettes of faces that rise steeply toward the viewer).
   - Cause: MSAA samples next to the edge fell on the nearer face, which beat the line despite the shader's small pull.
   - Fix: polygon offset on shaded faces, `FACE_DEPTH_BIAS {constant: -16, slope_scale: -2.0}` (reverse-Z, so negative pushes away). Applied to `mesh_opaque` and `mesh_pick`.
   - Test: `offscreen.rs::plate_edges_are_drawn_solid_over_faces` (grazing view, app-like scene with big planes). Without the bias, 32/315 samples fail in runs; with it, only isolated anti-aliased samples miss.
3. **Ctrl+O / "Open…" did nothing without a desktop portal.**
   - Cause: `files::prompt_open` always used rfd. Also, the global action runs while the window is borrowed, so `handle.update` failed.
   - Fix: when `!native_dialogs_available()`, `cx.defer` a `request_open_path` on the active window.
   - Verified in the app: the in-app prompt opens, and opening a path opens the document window.
4. **Zoom to fit included the default planes; the part ended up small and could be hidden by the dialog.**
   - Fix:
     - `Camera::fit_box` (`crates/render/src/camera.rs`): a tight projected-extent fit, with perspective back-off.
     - Viewport fields `fit_bounds` and `fit_inset_left`, plus `fit_camera_to_box` (`crates/app/src/viewport.rs`).
     - Part Studio (`part_studio/behaviors.rs`): `fit_bounds()` uses visible bodies and sketches, falling back to the planes. `fit_inset_left()` is 220 + 24 px while a feature dialog is open. Both are set in `refresh_scene` (`part_studio/mod.rs`).
     - Zoom to selection uses the same fit.
   - Test: `camera.rs::fit_box_frames_the_projected_extent`. Verified in the app.
5. **`#` variable autocomplete.**
   - Logic: `crates/units/src/complete.rs` (`variable_completion`, `apply_completion`, with unit tests).
   - UI: `crates/app/src/var_complete.rs` (an anchored list below the field).
   - Wiring in `feature_dialog.rs`:
     - `completion` field, computed on Change and cleared on Blur;
     - `completion_keys`: capture-phase ↑/↓/Enter/Esc. In GPUI, capture listeners must call `cx.stop_propagation()`, otherwise Enter also accepts the dialog;
     - `accept_completion`.
   - Only variables in scope at the edited feature: `noshape_model::variables::variables_defined_later` (test `variables.rs::scope_tests`), applied in `part_studio/mod.rs::open_dialog`.
   - Verified in the app: `#` lists the variables, Down plus Enter inserts `#width`, and the dialog stays open.
6. **Dimension value box not centered.**
   - Fix (`part_studio/sketch.rs`): the box sits at `top(y - 10)` and the text is centered with `Input::new(st).xsmall().text_center()`. Note that `InputState::set_text_align` is overridden by the element's style on every render.
   - Verified in the app.
7. **Hole picks labeled "Sketch 3 entity".**
   - Fix: `sketch_entity_kinds` in `part_studio/mod.rs` (the labeler); labels now read e.g. "Sketch 1 point", "line", "arc", "circle".
   - Verified: the Hole dialog shows "Sketch 1 point".
8. **Stale frames until the mouse moves** (coordinator report).
   - No reliable repro: undo/redo and trim drag were each checked by screenshot, nudge and compare.
   - Hardening in `viewport.rs`:
     - (a) `request_frame` never calls `notify()` synchronously. GPUI ignores `notify` during a draw (the view is marked dirty but no redraw is scheduled). It always defers, and the deferred callback notifies when off screen.
     - (b) `submit_if_ready` drops in-flight entries older than 3 s and schedules a 500 ms retry when the pipeline is full.
     - (c) The pick buffer is invalidated whenever the submitted camera or size differs. Keyboard zoom (`request_redraw`) used to leave the pick buffer stale.
   - Test: `offscreen.rs::every_scene_change_renders_fresh_content` (color-only and state-only changes each render).
9. **Editing an existing sketch showed it gray until the first edit.**
   - Cause: `apply_display` built the scene before `sync_sketch_editor` set `opts.editing_sketch`.
   - Fix: sync before the build.
   - Verified in the app (demo, Sketch 1).
10. **Tool search (Alt+C) found no sketch tools in a sketch.**
    - Fix: `PaletteItem::SketchTool` (`part_studio/view_tools.rs`), plus `sketch::all_sketch_tools()` and `activate_tool` made `pub(super)` in `part_studio/sketch.rs`.
11. **Configuration panel cells didn't commit on blur.** Renaming an input or configuration and clicking elsewhere was lost; only Enter committed.
    - Cause: GPUI delivers no Focus/Blur events to these inputs (verified with a log: only Change events arrive).
    - Fix (`part_studio/config_panel.rs` and new fields `panel_focused` and `panel_commits` in `part_studio/mod.rs`): every render remembers the focused cell, and `commit_unfocused_cell` commits it once focus has left. Enter updates the stored value so nothing commits twice.
    - Verified in the app: renaming the input and options, then clicking another field or the viewport, commits.
12. **Clicking the viewport left keyboard focus in a right-panel field**, so shortcuts typed into it (F produced "60 mmf").
    - Fix: `on_viewport_event` (`part_studio/mod.rs`) focuses the Part Studio when a panel cell had focus.
    - Verified in the app.

13. **"Configure a parameter" always used the first input.** A checkbox could never drive a parameter: Fillet suppression got keyed to the Size list.
    - Fix (`config_panel.rs`): with more than one input, the menu goes input ▸ feature ▸ parameter, and cards show the driving input ("Fillet 1 › Suppressed (Checkbox 2)").
    - Verified: Checkbox 2 checked suppresses Fillet 1.
14. **Configuration- or expression-suppressed features weren't styled as suppressed** in the feature list.
    - Fix (`part_studio/mod.rs::render_feature_row`): also treat a regenerated status of `Suppressed` as suppressed. The context menu still toggles the manual flag.
    - Verified.

## In progress / not yet tested (Part 2)

- Configurations: DONE (`/tmp/qa2/docs/c1.noshape`).
  - Size list with Small/Large drives the depth.
  - Checkbox 2 drives Fillet 1 suppression.
  - Boolean configured values are typed as text ("true"/"false"), not shown as checkboxes; a gap, not fixed.
- Not started: assembly (insert twice, fastened and revolute mates, drag, BOM, interference), drawing (views, dimensions, PDF), versions (version, branch, edit, merge, compare), STEP/STL export and re-import, robustness (delete a depended-on feature, close/reopen with undo, fast typing, Esc everywhere, tiny/huge window).

## Known open bugs (not fixed; repro)

- **Slot tool does nothing.** In a sketch on a face: Alt+C, "slot", Enter, then click, click, click. No preview and no geometry (the line tool works). Sketch agent's area (`sketch_mode.rs`).
- **Error toast is not cleared** after a later successful accept. Repro: Shift+E with no selection, press Enter (toast "Fix the feature before accepting it…"), select a region, press Enter. The feature is created but the toast stays.
- **Tooltips get stuck**: after clicking the right-panel toggles, their tooltip ("Variable table", "Configurations") stays at the old position after the button moves.
- **Dialog error text overflows** the 220 px dialog (e.g. "Select sketch regions or planar faces to extrude"). Theme/layout.
- **Esc in a sketch**: with no tool active, one Esc leaves (cancels) an existing-sketch edit. Check against Onshape ("Escape exits the command in use").
- Extrude doesn't preselect the just-finished sketch's region (Onshape does, I believe). Unverified.

## How to test

- Build: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-qa2 cargo build -p noshape`.
- Helper scripts in `/tmp/qa2/bin/`:
  - `start.sh <doc>` starts the app on `DISPLAY=:104` with `HOME=/tmp/qa2/home` and `NOSHAPE_NO_NATIVE_DIALOGS=1`;
  - `stop.sh` kills only the qa2 instance;
  - `shot.sh name [crop]` takes a screenshot;
  - `block.sh` sketches and extrudes a block in a fresh studio;
  - `stale.sh` compares screenshots before and after a mouse nudge.
- The window sits at (82,50), 1440x900. The feature list is at x≈177 and the dialog accept button at (519,150).
- Right-side panel toggles at x=1505 (when the panel is closed): y=492 Appearances, 523 Configurations, 555 Variables.

## Gotchas

- `pkill -f` patterns match your own shell if the pattern text appears in the command. Use `stop.sh`, or kill by PID.
- GPUI:
  - capture-phase action listeners don't stop propagation by themselves;
  - `notify()` during a draw is dropped;
  - `InputState` text alignment comes from the `Input` element's style;
  - panel inputs get no blur events.
- Clicking near the origin picks the origin point (8 px pick widening), not the region under it.
