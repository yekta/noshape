# Handoff: Part Studio behaviour parity with Onshape

Item-by-item status lives in `docs/parity/part-studio.md` (current as of
this handoff: 207 items, 67 ✅, 116 🔧, 17 ❌, 7 ⏳). This note is for whoever
continues.

## State right now

- The shared tree **compiles** (`cargo build -p noshape`, target
  `/root/target-psb`). No half-finished edits. Newest done items: tab
  manager and Ctrl+Space (`workspace.rs`), pause regeneration and times
  panel, parts list sections/separator/collapse/properties, A flips the
  mate connector axis.
- The last change was 5.23 in `feature_dialog.rs`: an unfocused numeric field
  shows its value converted to document units. Clicking it focuses the input.
  It built but hasn't been tried in the app yet.
- Built but not yet tried in the app either: the selection badge, missing
  references, pick reordering, Convert to expression, dynamic suppression,
  Copy/Paste sketch, sketch DXF export, multi-plane section, Select ▸ and
  Create selection.
- Tried in the app this round: curvature and draft maps, the Appearance
  editor on a part, and the part context menu.
- Not committed (the lead integrates).

## Where things live

| Area | Files |
|---|---|
| Pure logic + tests | `crates/model/src/interaction.rs` (dependencies/reorder, list click, feature filter, wheel nudge, evaluated hint, ray hits, dynamic-suppression test), `crates/model/src/select.rs` (Select ▸ criteria, kernel topology) |
| Model data | `partstudio.rs`: `Feature.hidden`, `Feature.suppression_expr`, `PartStudio.hidden_defaults`, `PartStudio.appearances` (`FaceAppearance`/`AppearanceTarget`), `PartProperties.{revision, mass_override, tessellation}`, `TessQuality`, `NamedView.section` |
| Regen | `regen/mod.rs`: dynamic suppression and checkbox expressions (`eval_bool`, `resolve_bool_exprs`); `regen/display.rs`: hidden sketches/planes/origin/connectors, per-part tessellation quality (finer meshes cached under a separate key), `SketchDisplay.color` |
| Render (surgical) | `renderer.rs`: `MeshVertex.color` (RGBA8, location 3), `upload_mesh_colored`, extra clip planes in `Globals` (`clip2`, `clip3`, `flags.x` = plane count); `scene.rs`: `Scene.clip_extra`; `shaders.wgsl`: `clipped()` = clipped by all planes, mesh vertex color for per-primitive instances |
| Scene builder | `scene_builder.rs`: `SceneOptions.section: Vec<(n, d)>`, `vertex_colors`, `hidden_edges_visible`; `DisplayMode::Translucent`; colored-mesh and hidden-edge caches in `GpuCache` |
| App | `part_studio/behaviors.rs` (`UiState`, picking, hide/isolate, select other, S toolbar, camera history, feature-list ops, auto Add/Remove, sketch copy/paste/export, dynamic suppression editor, Select ▸ and Create selection, selection badge), `view_cube.rs`, `section.rs`, `appearance.rs`, `analysis.rs`, `menus.rs` (graphics-area context menu), `mod.rs` (feature/parts list, actions), `view_tools.rs` (palette, shortcut sheet, named views), `config_panel.rs` (Appearance panel variant, variable hover), `feature_dialog.rs` (field behaviours), `actions.rs` (bindings), `viewport.rs` (zoom window, double middle-click, BoxSelect modifiers, `pointer_inside()`) |

## Remaining items (priority order) with notes

1. **9.11 tangent edges.** Hidden edges visible and Translucent are done
   (camera menu). Tangent-edge visible/phantom/removed needs per-edge
   smoothness: use `noshape_model::select` style edge/face normal tests per
   body edge, then draw those edges dashed or skip them in the scene builder.
2. **5.18 dropdown keys.** Tab to an enum dropdown, then Enter/Space should
   open it and arrows should move. Check whether gpui-kit already does this;
   if not, handle `on_key_down` on the dropdown wrapper in
   `feature_dialog.rs`.
(6.25, 6.26, 7.6, 7.7, 1.25 and 1.27 are done.)

7. **11.5 mass properties.** Add a Face tab (area/perimeter of selected
   faces; kernel `area()`, edge lengths) and Override mass (use
   `PartProperties.mass_override` in the mass rows in `menus.rs`). Reference
   mate connector: transform the inertia by parallel axis.
8. **6.38 nested folders.** Add `Folder.parent: Option<u32>` (serde
   default). `new_folder` inside a folder sets the parent. Render with depth
   indentation; a collapsed ancestor hides its descendants.
9. **10.6 / 2.23 transparent geometry.** Add a "Select transparent geometry"
   checkbox to the isolate panel (`render_isolate_panel`). When it's off, or
   when Alt is held on click, re-pick via
   `interaction::entities_along_ray` skipping the transparent bodies (see
   `select_other` for the ray setup). For Expand by distance, sort bodies by
   bounds distance and make the first N opaque.
10. **11.3 measure graphics.** Draw a dotted line between the min-distance
    points as an extra `Instance` in `refresh_scene` (see
    `manipulator_overlay`). Add a type dropdown that filters
    `measure_cache` rows.
11. **3.10 midpoints.** While hovering an edge, draw a point at the polyline
    midpoint (overlay points). A click within about 8 px of its projection
    picks `Reference::Implicit { on: edge, inference: Midpoint }`. Check that
    `measure`/plane features resolve `Implicit`.
12. **9.15 rotation inertia and pivot.**
    - Inertia: in `viewport.rs`, keep the last orbit delta. On right-button
      up with recent motion, keep orbiting with ×0.77 per frame via
      `request_animation_frame`.
    - Pivot: add `pub orbit_pivot: Option<DVec3>` set by the Part Studio on
      right-down (ray hit) and use `camera.orbit_around`.
    - Keep these viewport edits surgical; QA works there.
13. **5.25 preview opacity slider.** Regenerate `upto = index` (cheap, it's
    cached) into `preview_before`. In `display_for_scene`, merge the
    "before" bodies (alpha 1−s) with the "after" bodies (alpha s). Put the
    slider in the dialog footer and emit a new `DialogEvent`.
14. **5.27 movable dialog and help.**
    - Moving: `on_drag` on the dialog title, plus an `on_drag_move` listener
      on the Part Studio root that updates `ui.dialog_pos`. Record the
      overlay container origin with a canvas, like `cube_origin`.
    - Help: the "?" can show `FeatureType.description`.
15. **5.29 tolerances.** For `Hints::CAN_BE_TOLERANT` numeric fields, add a
    ± button and store `"<id>.tolerance"` as `ParamValue::Text` JSON (type,
    upper, lower, precision). Show an inline row with upper/lower inputs.
16. **5.32 more manipulators.** Use the extrude/revolve pattern in
    `manipulators.rs` for fillet radius, chamfer distance, shell thickness
    and plane offset.
17. **6.24 remainder.** Show dimensions needs sketch dims outside edit mode,
    and Create drawing of sketch needs drawing-model support.
18. **10.8 remainder.** Section drag manipulator (reuse the
    `manipulators.rs` arrow), and caps (stencil pass in the renderer; large
    change).
19. **Toolbar customization.**
    - 1.21 S toolbar contents and 4.7 toolbar customization: persist JSON in
      the config dir.
    - Search tools needs a "Removed from toolbar" section.
    - `TOOLBAR` is a const in `mod.rs`. Make it data and keep the group
      faces in `ui.toolbar_faces`.
20. **Misc shortcuts.** 1.28: Ctrl+U shows an About/log dialog;
    Ctrl+Shift+F is Feature Studio search.

## How to test

- Build: `cd /root/noshape && OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-psb cargo build -p noshape`.
- Tests:
  - `cargo test -p noshape-model --lib interaction select`;
  - app: `cargo test -p noshape -- view_cube behaviors view_tools appearance analysis`;
  - full: `cargo test --workspace --exclude noshape --no-fail-fast`.
  - The render `offscreen::plate_edges_are_drawn_solid_over_faces` failure
    belongs to QA round 2.
- Clippy: `cargo clippy -p noshape --all-targets` and `-p noshape-model`.
  Filter the output to your own files; other agents' warnings may be present.
- UI on Xvfb `:102` (window at x=82, y=50, viewport about x 335–1520,
  y 125–918):
  - `/tmp/psb/start.sh` kills our instance, deletes `/tmp/psb/x.noshape*`
    (stale sqlite WAL files cause "disk I/O error"), starts
    `NOSHAPE_NO_NATIVE_DIALOGS=1 DISPLAY=:102 timeout 200 …/noshape --demo /tmp/psb/x.noshape`
    and waits 14 s.
  - `/tmp/psb/x.sh <name> [xdotool args]` runs xdotool, waits `$WAIT` s
    (default 1.5), and screenshots to `/tmp/psb/<name>.png`. Crop with
    `convert`.
  - The app exits after 200 s, so restart before each test session.
  - Kill only `pkill -f "[t]arget-psb/debug/noshape"`.
  - Use `timeout N tail -f /dev/null` to wait; foreground `sleep` is
    blocked.

## Gotchas

- **Concurrent edits.** Other agents (theme/layout, sketch, QA) edit the
  same files, including my `behaviors.rs`, `view_cube.rs` and the feature
  list. Always re-read before editing, and use exact-match replacements that
  assert. Builds often fail for a few minutes on someone else's half-done
  file; retry.
- **`.context_menu(...)` must be last** in a builder chain (it returns
  `ContextMenu<E>`).
- **Overlays need `.occlude()`.** Otherwise clicks fall through to the
  viewport and select geometry.
- **Re-rendering on mouse move re-renders the whole tree.** A child entity
  doesn't help, because GPUI marks the ancestors dirty too. Throttle instead
  (see the badge, ~30 Hz).
- **No new `#[allow]`.** Use type aliases (`ViewAction`, `VertexColors`)
  for complex types.
- **Mesh cache keys.** The face mesh is per body and each face owns its
  vertices, so per-face vertex colors work. A colored mesh is cached by
  (mesh ptr, colors ptr), so keep the colors `Arc` stable
  (`ui.vcolor_cache`).
- **Space in dialogs.** `on_clear_selection` propagates while a dialog is
  open, so Space can toggle checkboxes.
- **Enter is bound in the Part Studio context.** Accept propagates in
  sketches, which keeps the sketch agent's behaviour.
- **Onshape vs coordinator.** Onshape's Shift+D is dihedral analysis; per
  the coordinator it is draft analysis here.
