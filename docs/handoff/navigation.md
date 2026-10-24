# Handoff: view navigation (agent "nav", interaction engineer)

Task: view navigation exactly like Onshape on every input device and
platform (mouse, trackpad incl. pinch, keyboard, view cube, touchscreen,
Preferences › Mouse controls) in Part Studio, Assembly and Drawing.
Checklist: `docs/parity/navigation.md`.

## nav2 (2026-10-24): exact input numbers without hardware

Worktree /root/wt/nav2 (branch agent/nav2, codegen-units 237), scratch
/root/nav2-scratch (Chromium sources in chromium/, test fixture fix/).

* **Chrome mapping**: Onshape reads `wheelDelta` only; how each OS's input
  becomes it is in docs/parity/navigation.md "Chrome's input mapping"
  (Chromium source, quoted file by file). Consequences fixed here: pinch =
  one wheel notch per update (not the finger ratio), Windows Shift+wheel
  zooms, macOS notched wheels use the raw notch (no acceleration), Wayland
  finger scroll 12 per axis unit (was 8), Windows precision touchpad 1 per
  whole pixel (was 1.2), X11 has no touchpad pinch.
* **gpui patch**: `ScrollWheelEvent::wheel_ticks` (Chromium's wheel_ticks,
  before any Shift swap) filled by every platform layer (macOS CGEvent
  fields, Windows WHEEL_DELTA / DirectManipulation whole pixels, X11
  valuator increments, Wayland value120 / axis); macOS magnify Cancelled =
  Ended. `navigation::wheel_delta` / `wheel_zoom_amount` / `pinch_zoom_amount`.
  Cross-checks: `cargo +stable check --target aarch64-apple-darwin -p
  gpui-pre-macos` and `--target x86_64-pc-windows-msvc -p gpui-pre-windows`
  with CARGO_TARGET_DIR=/root/target-shared/cross (macOS: touch the
  placeholder `…/gpui-pre-apple-*/out/stitched_shaders.metal`; Windows:
  PATH=/root/nav2-scratch/fakebin for a stub `llvm-rc`). The pinned 1.98.1
  toolchain has no cross std; stable (also 1.98.1) does.
* **Replay harness** `.github/nav/` + `.github/workflows/nav.yml`
  (workflow_dispatch): `NOSHAPE_NAV_TRACE=<file>` makes the viewport write
  every input and camera as JSON lines (`nav_trace.rs`). Linux: uinput
  mouse/keyboard/touchpad/touchscreen (`uinput.py`, no deps) under a headless
  sway (`WLR_BACKENDS=headless,libinput LIBSEAT_BACKEND=noop`, root) →
  `replay.py` → `check.py` recomputes Chrome+Onshape from WAYLAND_DEBUG and
  checks each camera change. macOS: `mac-replay.swift` (CGEvents). Windows:
  `win-replay.ps1` (SendInput, InjectTouchInput; touchpads can't be injected).
  Local run 10-24 (Xwayland installed with apt): Wayland 468/468 and X11
  240/240 checks pass. Run it as root:
  `.github/nav/linux-replay.sh <bin> .github/nav/block.noshape <out> wayland|x11`.
* **Live request** REQUESTS.md "nav2 (P1)": DOM WheelEvent wheelDelta sweep,
  CDP synthesizePinchGesture (real touchpad-pinch pipeline), drags with the
  camera read from the view controller, CDP touch gestures.

## Where the truth comes from

Onshape's own client code is in `/root/onshape-research/bundle-raw/`:
`webgl.*.js` has the view controller (search `zoomOnMouseWheelScroll`,
`zoomOnCanvasPoint`, `pinchZoom`, `onDrag(e,t,i,n,s,r)`,
`updateManipulationMode`, `getClosestMode`, `performStopRotationAnimation`,
`rotateInDirection`, `animateRotation`, `animatePan`, `animateZoom`,
`canZoom`, `dollyAlong`); `woolsthorpe.*.js` has the constants
(`ROTATE_ANGLE:15`, `ROTATION_INERTIA:.77`, `DEFAULT_PAN_AMOUNT_PX:150`,
`DEFAULT_ZOOM_FACTOR:1.75`, …) and the Mouse controls page
(`mouseActionSettingsMap`, `setDefaultMouseScrollDirection`). Prefer it to
guessing; quote the function name in comments.

Key facts derived from it:
* Every wheel / trackpad scroll zooms (never pans) about the cursor:
  amount i = 0.5 per notch (×3 on Windows), view scale 2^(−i/6).
  Horizontal-dominant scrolls are ignored (Shift+wheel included).
* Browser `wheelDelta` per platform is emulated in `navigation::wheel_delta`
  (macOS 120/line, 3/px; Windows 120/notch from gpui lines ÷ system wheel
  lines, 1.2/px; Linux 40/gpui line, 120/45 per Wayland px).
* Touchpad pinch: one wheel notch per pinch update (Chrome sends ±120
  Ctrl+wheel events); touch-screen pinch: the finger ratio (`pinchZoom`).
* Right-drag rotate is free (camera axes), 3 turns per viewport diagonal;
  Alt = constrained (levels the view, then about world Z).
* Bindings: flags MMB=1 RMB=2 SHIFT=4 CTRL=8 ALT=16; left button has no
  flag (never navigates); unbound modifiers are ignored (Shift+right
  rotates); "closest mode" fallback; mode re-resolved on every drag event.
* Keys: steps animate linearly 250 ms and run concurrently (holding a key
  adds them up); pan/zoom keys are ignored while the camera animates.

## Code

* `crates/app/src/navigation.rs` — all mapping/math + 20 unit tests
  (`cargo test -p noshape navigation`). Mouse controls settings
  (`MouseControls`, saved to `<config>/mouse-controls.json`, cached).
* `crates/app/src/viewport.rs` — input only (ui owns the visuals):
  `Drag::Nav` + `nav_mode`, `on_scroll`, `on_pinch`, `on_mouse_up_out`,
  inertia and keyboard steps in `render`, public `rotate_step`, `pan_step`,
  `zoom_step`, `animate_to_with`. View animations now ease with
  smootherstep (`Camera::interpolate` + `navigation::ease`).
* `crates/render/src/camera.rs` — added `Camera::interpolate` (lerp without
  easing; `lerp` calls it).
* Part Studio (`part_studio/mod.rs` `rotate_by`, ZoomIn/Out handlers;
  `behaviors.rs` `pan_by`; `view_cube.rs` `roll_view`) and Assembly
  (`assembly_view.rs`: cube hooks + new arrow/Z/pan handlers) delegate to the
  viewport API.
* Drawings (`drawing_tools.rs` `on_scroll`, `on_pinch`, drag start;
  `drawing_view.rs` `Drag::Zoom`, `.on_pinch`) use the 2D bindings.
* `crates/app/src/mouse_prefs.rs` — the Mouse controls section, first in
  the Preferences page (`preferences_window.rs`: field `mouse`; the
  Keyboard shortcuts scroll target index became 3).

## State (2026-10-19)

Done (code + 18 passing unit tests in navigation.rs): checklist sections
1–6. Verified in the app on Xvfb :109 (screenshots in /tmp/nav/shots):
* Front + Left arrow brings the Right face in, Up brings the bottom
  (= live navigation/75–76); right-drag left = same direction; Alt+drag
  stays level.
* 3 wheel notches at an off-centre point zoom ×1.19 about it; 3 back out
  give a pixel-identical image.
* Holding Left 1 s turns ~165° (X autorepeat 660 ms / 25 Hz).
* Z = ×1.75 out, Ctrl+Shift+Left = 150 px left; Assembly arrows/Shift+Z work.
* Preferences › Mouse controls renders like live 61. (ui, 19 Oct) os_menu
  now works in every window (`os_menu::open_window` wraps each window's
  content and draws that window's open menu), and the "View settings" select
  uses it: `OsMenu::native_select(170)` (browser `<select>` list: 27 px rows,
  `#1e63d0` highlight, 1 px `#767676`), opened with `open_anchored` 1 px under
  the field, the current preset `checked`; `MousePrefsView` keeps
  `open: Option<Subscription>` (DismissEvent) and `closed_at` so a click on
  the field closes the list. The old hand-made list is gone. Verified on X11
  (pick, toggle close; matches live 62).

Live answers (behaviours.md "View navigation…") were applied: Shift+wheel
zooms on Linux; drawing Z/Shift+Z ×2 about the centre, Ctrl+Shift+arrows
10 % of the canvas width, double middle = fit, arrows nothing.
Open: the live CDP held-arrow test gave 105°/1.2 s while the client code
adds every repeat; re-measure with real X autorepeat requested.

## Round 2 (2026-10-19, lead decisions)

* **Rotate centre by visible depth** (`getRotateCenterBasedOnWindowDepths`):
  `PickRect::depth_samples(n)` makes the render worker draw a tiny pick pass
  (viewport shrunk to ~n px, Onshape's `DepthSamplingPass`) into its own
  Depth32Float + R32Uint textures and read them back whole (depth textures
  cannot be partially copied); `PickBuffer::samples` + `view_proj` come
  back. The pick pipelines now use `PICK_DEPTH_FORMAT` (Depth32Float, no
  stencil). The viewport asks at a navigation press when nothing is under
  the cursor (`request_depth_pivot`) and swaps the centre in when it lands
  (`pivot_awaits_depth`). `navigation::depth_average_pivot` averages; GPU
  test `visible_depth_samples_land_on_the_geometry` (render/tests/offscreen.rs).
  The sample query renders two passes (`DrawMode::depth_samples`, pipeline
  `mesh_sample`: true depth, no pick bias; faces and points, no lines, as
  Onshape's pass): without plane depth (`PickBuffer::samples`) and with the
  datum planes writing depth (`samples_with_planes`, Onshape's
  `includePlanes` pass). The viewport uses the second only when the first
  has under 4 hits (only planes visible). GPU test
  `plane_depth_counts_only_in_the_with_planes_samples`.
* **macOS Ctrl+click** (vendor/gpui-pre-macos): `last_right_press_was_ctrl_click()`
  marks the platform's Ctrl+left→right conversion; the viewport keeps the
  click (context menu) and makes the drag do nothing.
* **Fit**: `Camera::fit_box` pads 1.05 (`CAMERA_ZOOM_TO_FIT_PADDING`);
  Part Studio `zoom_fit_bounds()` adds the visible planes (Onshape's model
  node bounds); standard views and view cube clicks fit at the new
  orientation (`Viewport::animate_to_rotation_fit`). Check: Onshape's
  trimetric fit shows the three default planes over 0.80 of the graphics
  height (live navigation/70), which is exactly cube-box corners ×1.05.
* **Touchscreen** (vendored gpui):
  - `vendor/gpui-pre` (core): `impl MouseEvent for TouchEvent`; in
    `Window::dispatch_touch_event` raw touches go to mouse listeners first,
    a `prevent_default` on Started claims the touch from the portable
    recognizer; `Window::topmost_hitbox_at`.
  - `vendor/gpui-pre-windows`: `WM_POINTER*` for PT_TOUCH (and PT_PEN unless
    `set_pen_as_mouse`) → `PlatformInput::Touch`, handled so Windows does not
    emulate the mouse.
  - `vendor/gpui-pre-linux`: X11 XInput 2.2 TouchBegin/Update/End (mask set
    when the server has 2.2, `XINPUT_TOUCH`), Wayland `wl_touch`.
  - `touch.rs`: Onshape's recognizers (taps, single drag, pan, roll, pinch,
    two-finger long-press box) with unit tests; `Viewport::on_touch` claims
    touches whose start is on its canvas hitbox and runs `touch_actions`.
  - Every vendored change is marked `NOSHAPE PATCH`; Cargo.toml
    `[patch.crates-io]` lists them. Examples/tests of gpui-pre are omitted.
  - Cross checks: the Windows and macOS crates type-check for
    x86_64-pc-windows-msvc / aarch64-apple-darwin from scratch copies
    (`/tmp/nav/winchk`, `/tmp/nav/macchk`; macOS needs a placeholder
    `stitched_shaders.metal`, Windows `--no-default-features` without llvm-rc).

## Next

1. Touch on real hardware: nothing here can produce touches (Xvfb has no
   touch device). Unit tests cover the recognizer; the platform paths
   compile (Linux natively, Windows/macOS cross). First test on a Windows
   touch laptop / Linux touchscreen: tap-select, one-finger rotate, pinch,
   two-finger pan/roll, two-finger tap menu, long-press box, and that taps on
   the view cube and dialogs still click.
2. Linux pen as touch ("Use pen input as mouse" off): X11/Wayland tablets
   arrive as pointer devices; needs device-type detection in
   vendor/gpui-pre-linux.
3. Assembly fit bounds use the scene bounds (no default planes there).

Cross-check cache: /root/target-shared/cross (scratch copies of the
vendored crates go in /tmp; see "Round 2").

## Gotchas

* GPUI hitboxes may only be inserted during **prepaint** (inserting one in
  paint crashed every document window; ui2 fixed the touch hitbox: canvas
  prepaint returns it, paint registers the `TouchEvent` listener with its id).

* macOS gpui turns Ctrl+left-click into a plain right button (control
  cleared), so we cannot tell it from a real right click.
* Windows gpui turns Shift+wheel into horizontal lines; Linux too. That is
  exactly why Shift+wheel does nothing (as in Onshape's browser client).
* `on_mouse_up` only fires over the element; `on_mouse_up_out` ends drags
  released outside.
