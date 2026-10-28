# View navigation parity with Onshape

Every way of moving the camera in Onshape (mouse, touchpad, keyboard, view
cube, touchscreen, the Mouse controls preference) and its state in noshape.

Sources:
* Help: `docs-raw/md/View__view_navigation_and_the_view_cube.md` (the
  Windows/macOS navigation table and the touchscreen table).
* Preferences › Mouse controls: live DOM `live/dom/preferences-material-libraries.txt`
  (lines 185–247) and the client templates in `bundle-raw/woolsthorpe.*.js`
  (`mouseActionSettingsMap`, `viewManipulationMapping`).
* The behaviour itself, read from Onshape's client code
  (`bundle-raw/webgl.*.js`: `zoomOnMouseWheelScroll`, `zoomOnCanvasPoint`,
  `pinchZoom`, `onDrag`, `updateManipulationMode`, `getClosestMode`,
  `performStopRotationAnimation`, `rotateInDirection`, `animateRotation`,
  `animatePan`, `animateZoom`, `canZoom`; constants from the
  `woolsthorpe` config: `ROTATE_ANGLE` 15, `PRECISION_ROTATE_ANGLE` 5,
  `COARSE_ROTATE_ANGLE` 90, `ROTATION_CYCLES_PER_SCREEN` 3,
  `ROTATION_INERTIA` .77, `DEFAULT_PAN_AMOUNT_PX` 150, `DEFAULT_ZOOM_FACTOR`
  1.75, `DRAG_ZOOM_FACTOR` 10, `MIN_MODEL_PIXEL_SIZE` 100,
  `USE_CENTRAL_ZOOM_OUT` 0, `CAMERA_SHORT_ANIMATION_TIME_MS` 250).

Code: `crates/app-core/src/navigation.rs` (all input→camera math, unit tested:
`cargo test -p noshape navigation`), `viewport.rs` (3D input), the drawing
canvas (`drawing_tools.rs` / `drawing_view.rs`, 2D).

Status: ✅ works · 🔧 fixed/added in this pass · ⏳ partial · ❌ missing.
Evidence tags in the tables: (L = live behaviours.md "nav2 navigation numbers"; R = Linux replay, `.github/nav`; U = unit test)

## Chrome's input mapping (what Onshape's client actually receives)

Onshape's zoom reads only the legacy DOM `wheelDelta` of `mousewheel` events
(`zoomOnMouseWheelScroll`: i = 0.5/120 × wheelDelta, ×3 when
`navigator.platform` contains "Win"; scale 2^(−i/6)). So every trackpad,
wheel and pinch number depends on how the browser turns OS input into
`wheelDelta`. Chromium's own source (fetched 10-24, copies in
/root/nav2-scratch/chromium) says:

* **Blink** (`wheel_event.cc`): `wheelDelta = int(wheel_ticks × 120)`, in f32,
  truncated; `wheelDelta` is the vertical value unless that is 0.
* **macOS** (`web_input_event_builders_mac.mm`): notched wheels give
  `wheel_ticks = kCGScrollWheelEventDeltaAxis1` (raw ±1 per notch, *no*
  acceleration, however fast the wheel spins); continuous devices (trackpad,
  Magic Mouse) `PointDeltaAxis1 / 40`, i.e. wheelDelta = 3 × points. Shift
  moves a notched wheel to axis 2 (ignored); trackpad point deltas stay
  vertical with Shift. Momentum events go the same way.
* **Windows** (`web_input_event_builders_win.cc`): `GET_WHEEL_DELTA_WPARAM`
  as is (120 per notch, high-resolution wheels and legacy touchpads less).
  **Shift+wheel stays vertical** (the Shift swap in `mouse_wheel_event_queue.cc`
  is only for page scrolling, after the page saw the event), so it zooms.
  Precision touchpads (`direct_manipulation_event_handler_win.cc`): whole
  DIP pixels of the DirectManipulation translation per update, wheelDelta =
  pixels.
* **X11** (`events_x_utils.cc`): 120 per wheel button or per scroll-valuator
  increment (touchpads: 120 × Δvaluator / increment). **No touchpad pinch.**
* **Wayland** (`wayland_pointer.cc`, `wayland_event_source.cc`): wheels
  `value120` (rounded per frame), finger / continuous scroll −12 × the axis
  value (`kWaylandUnscaledTouchpadScrolling` is off by default).
* **Touchpad pinch** (macOS magnify, Windows precision touchpad, Wayland
  `zwp_pointer_gestures`; `touchpad_pinch_event_queue.cc`): each pinch update
  becomes one Ctrl+`mousewheel` with `wheel_ticks` = +1 if its scale is above
  1, else −1. Onshape zooms **one notch per update** (×1.06, ×1.19 on Windows)
  whatever the size of the step; begin/end events zoom nothing; smart zoom
  (two-finger double tap) is wheelDelta 0 (nothing).
* **CDP** (`input_handler.cc`) and synthesized gestures always send ±1 tick per
  event, which is why the live capture saw "one event = one ×1.06 step
  whatever its size". Live checks of fractional wheelDelta must dispatch DOM
  `WheelEvent('mousewheel', {wheelDeltaY})` (requested: REQUESTS.md "nav2 (P1)").

noshape: the vendored platform layers report Chromium's `wheel_ticks` in
`ScrollWheelEvent::wheel_ticks` (NOSHAPE PATCH in gpui-pre, -macos, -windows,
-linux), and `navigation.rs` applies Blink's truncation and Onshape's
formula. Replay harness (real OS input → camera trace → independent
recomputation from the raw OS/protocol events): `.github/nav/`.

## 1. Mouse (desktop, default "Onshape" mouse controls)

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 1.1 | Right-drag: 3D rotate, free (about the camera's own up/right axes, not a turntable) | help table; `onDrag` ROTATE | ✅ L: 200 px → 111.6066° = 3 turns per canvas diagonal; R: 133.127° for 200 px, exact | 0.4.10+ / 2a6d0167 10-27 |
| 1.2 | Rotation speed: 3 full turns per viewport diagonal | `ROTATION_CYCLES_PER_SCREEN` | ✅ L, R, U `drag_rotate_rate_matches_live` |  |
| 1.3 | Rotate centre: the geometry under the cursor; else the average of ~2000 visible-depth samples (planes excluded, ≥ 4 hits); else the same with the planes' depth (only planes visible); else the model centre when it fits the view; else the cursor ray at the model's depth | `getRotateCenterForCanvasLocation`, `getRotateCenterBasedOnWindowDepths({includePlanes})`, `DepthSamplingPass` | ✅ fixed 10-24: moves wait for the visible-depth centre (Onshape finds it before rotating; ours comes back from the render worker a frame or two later), then apply about it; R: the centre stays put on screen (< 0.01 px) on Wayland and X11 | 0.4.10+ / 2a6d0167 10-27 |
| 1.4 | Rotation inertia: speed averaged per step, continues if released within 50 ms, decays ×0.77 per 60 Hz frame (time based), stops under 1 px/step | `performStopRotationAnimation` | ✅ L (client code: speed ½(speed+Δ), ×0.77 per 16.67 ms, > 1 px) = U `inertia_decays_like_onshape`; R: no coast under 20 fps, as Onshape |  |
| 1.5 | Alt+right-drag: constrained rotate (horizontal = about world Z, vertical = pitch); first animates to the nearest floor-down view (125 ms, 250 ms if over 45°) | help (Windows column); `AXIS_ROTATE` | ✅ fixed 10-24: the levelling event adds no rotation (Onshape `axisRotateCanvasTag`); L 190 px of 200 turn; R: 53.2507° for 80 px after levelling, elevation and right.z unchanged | 0.4.10+ / fa61ba6f 10-27 |
| 1.6 | Ctrl+right-drag: 2D pan | help table | ✅ | 0.4.10+ / 2a6d0167 10-27 |
| 1.7 | Middle-drag: 2D pan | help table | ✅ | 0.4.10+ / 2a6d0167 10-27 |
| 1.8 | Pan keeps the point under the cursor under it (perspective: in the plane through the geometry under the cursor) | `pan` | ✅ L: start point ends under the pointer to 1e-4 px; R: (200, 100) drag → 223.607 px | 8627322e 10-28 (wf-block, middle-drag from the part by (200, 100) → its silhouette moves exactly (200.0, 100.0) px, the grabbed point stays under the pointer) |
| 1.9 | Shift+right-drag rotates (Shift is not bound; it does not pan) | `getClosestMode`, `usesShiftKey` | ✅ qa4 10-23 (Front → the Right face comes in, no pan) | 0.4.10+ / 2a6d0167 10-27 |
| 1.10 | Ctrl+middle-drag pans (closest binding) | `getClosestMode` | ✅ | 0.4.10+ / fa61ba6f 10-27 |
| 1.11 | Pressing/releasing Ctrl or Alt mid-drag switches mode (rotate ↔ pan ↔ constrained) without lifting the button | `updateManipulationMode` per drag event | ✅ R: right-drag −100 px, Ctrl held +100 px, released −100 px → the view turns 200 px worth (the Ctrl part pans) | 0.4.10+ / 41f60e47 10-28 (Ctrl down mid-drag → pan, up → rotate) |
| 1.12 | A second button pressed mid-drag re-resolves the mode | `updateInputState` | ✅ R: right-drag, middle pressed (closest binding other than Rotate = Pan, as `getClosestMode`), middle released (Rotate) → turns 200 px worth. Fixed 10-25: on Wayland gpui forgot the right button when the middle one was released, ending the drag |  |
| 1.13 | Releasing outside the graphics area ends the drag | — | ✅ fixed 10-25: the drag also follows the pointer over the panels (Onshape's dragHelper is jQuery UI `mouse`, bound to the document; ours stopped at the viewport edge) and ends on release there; R: 220 px from the viewport into the feature list → 146.442° = 3 turns/diagonal, no change after the release | 0.4.10+ / 41f60e47 10-28 |
| 1.14 | Right-click (no drag): context menu; a right-drag opens none | Home__context_menus | ✅ | 0.4.10+ / 2a6d0167 10-27 |
| 1.15 | Double-click middle button: zoom to fit (animated) | `onMiddleMouseButtonDoubleClick` | ✅ | 0.4.10+ / 2a6d0167 10-27 |
| 1.16 | Scroll wheel up zooms in, down zooms out, about the cursor (also when zooming out) | help; `zoomOnCanvasPoint`, `USE_CENTRAL_ZOOM_OUT` 0; live navigation/11–14 (fixed point = pointer both ways) | ✅ L, R (Wayland value120, X11 valuators, both directions, fixed point < 0.5 px) | 0.4.10+ / 2a6d0167 10-27 |
| 1.17 | Zoom per notch: scale 2^(−i/6) with i = 0.5 per notch (×3 on Windows): ×1.06 per notch on macOS/Linux, ×1.19 on Windows | `zoomOnMouseWheelScroll`; live (Linux Chrome): ×1.060 per notch, up then down returns exactly | ✅ L: 2^(−W/1440) for W = 120, 60, 30, 12, 3, −3, −120, 360; R: hi-res 30 and 7 → wheelDelta 30 and 7, exact | 8627322e 10-28 (wf-block, pointer on the part, 12 wheel notches in: silhouette 165×96 → 327×192 px = ×1.98 / ×2.00 ≈ 1.06^12; the part's centre under the pointer stays (1000,510 → 1000,511)) |
| 1.18 | Perspective zoom dollies the eye along the cursor ray by distance × i/6, within 0.05–50 scene diameters | `zoomPoint`, `dollyAlong` | ✅ live "nav2 perspective zoom" (10-25): fit 446.8626 mm and nine wheel events at the live pointer match to 2 µm (U `perspective_wheel_matches_live_onshape`); right-drag 3 turns/diagonal as in ortho. ⏳ the zoom-out stop (live 3240.83 mm, ours one step earlier) awaits the live `canZoom` values (REQUESTS "nav2 (P1) (10-25 b") |  |
| 1.19 | Zoom limits: no zoom out once the model is under 100 px across, no zoom in past 2^24 px/m | `canZoom` | ✅ U `zoom_limits_like_can_zoom` (app-core navigation, from the bundle) |  |
| 1.20 | Horizontal scroll (tilt wheel, sideways swipe) does nothing | `zoomOnMouseWheelScroll` (`wheelDeltaX > wheelDeltaY` → ignored); live navigation/17 | ✅ R: horizontal wheel → no zoom (X11 and Wayland) | 0.4.10+ / 2a6d0167 10-27 |
| 1.20b | Shift+wheel and Ctrl+wheel zoom exactly like the wheel (Linux; macOS and Chrome on Windows turn Shift+wheel horizontal before Onshape sees it, so it does nothing there) | live navigation/15–16 | ✅ R: Shift / Ctrl+wheel zoom like the wheel; Windows Shift+wheel now zooms too (Chromium keeps it vertical) | 0.4.10+ / fa61ba6f 10-27 |
| 1.21 | Scrolling during a box selection does nothing | `onMouseWheelScroll` | ✅ R: three wheel notches during a left-drag box → no camera change | 0.4.10+ / 41f60e47 10-28 |
| 1.22 | Left-drag: box select (left→right window, right→left crossing); never navigates | Home__selection; no flag for the left button | ✅ | 0.4.10+ / 2a6d0167 10-27 |

## 2. Touchpad / trackpad

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 2.1 | Windows: right-touchpad-button click + drag rotates; Ctrl + it pans | help table | ✅ (they are right-button events) | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.2 | macOS: two-finger click + drag rotates (a secondary click) | help table | ✅ (NSRightMouseDown) | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.3 | macOS: Ctrl + two-finger click + drag pans | help table | ✅ (gpui keeps Ctrl on a real right button) | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.4 | macOS: two-finger scroll zooms (up = in, down = out), about the pointer, 40 px of scroll = one notch | help table; Chrome `wheelDelta` = 3 × pixels (the live CDP test saw one ×1.06 step per event whatever its size, but CDP synthesizes ±1 wheel tick per event; real precise deltas reach Onshape's `i = 0.5/120 × wheelDelta` proportionally) | ✅ 10-24: wheelDelta = 3 × CGEvent point delta (Chromium `PointDeltaAxis / 40`, read from the CGEvent in vendor/gpui-pre-macos); U `trackpad_scroll_zooms_by_chromium_wheel_delta`; macOS CGEvent replay in nav.yml | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.5 | Scroll momentum after lifting the fingers keeps zooming smoothly (each momentum event zooms by its own small delta; no jumps) | Chrome forwards momentum `mousewheel` events | ✅ momentum events take the same path (same CGEvent fields); U (whole wheelDelta units, sub-unit momentum = 0) | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.6 | Windows precision touchpad: two-finger scroll zooms (100 px = one notch, ×3) | Chrome DirectManipulation wheel | ✅ 10-24: wheelDelta = whole DIP pixels ×3 (Chromium DirectManipulation handler; vendor/gpui-pre-windows now truncates like it); U. Touchpads can't be injected on Windows (InjectSyntheticPointerInput: PT_TOUCH/PT_PEN only) | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.7 | Touchpad pinch (macOS, Windows precision touchpad, Wayland) zooms one wheel notch per pinch update about the pointer, whatever the step size: Chrome hands each update to Onshape as a Ctrl+`mousewheel` of ±120; on X11 Chrome has no touchpad pinch | `touchpad_pinch_event_queue.cc`, `zoomOnMouseWheelScroll`; live nav2 (2) | ✅ fixed 10-24: one wheel notch per pinch update (×1.06, ×1.19 on Windows), not the finger ratio. L: synthesizePinchGesture ×2 → 5 Ctrl+wheel events of ±120 → 2^(−5/12); R Wayland: 40 updates = 40 notches; X11: none (Chromium has no X11 touchpad pinch); U `pinch_updates_zoom_one_notch_each`. Also fixed: gpui delivered every Wayland pinch update twice | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.8 | Linux touchpads: two-finger scroll zooms (X11 smooth scroll in notches, Wayland pixels at 45 px per notch), pinch as 2.7 | Chrome on Linux | ✅ R: Wayland finger scroll −12 × axis value, X11 120 × valuator / increment, matching the protocol log event for event | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 2.9 | macOS one-finger Ctrl+click: a click opens the context menu, a drag does nothing (Chrome reports a left button with Ctrl, which never navigates) | Chrome on macOS | 🔧 (vendor/gpui-pre-macos marks the converted press) |  |

## 3. Keyboard

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 3.1 | Arrow keys rotate 15° about the camera's up (←/→) or right (↑/↓) axis through the camera target; ← from Front brings the Right face in, ↑ the Bottom (= dragging that way) | help; `rotateInDirection`; live navigation/75–80 | ✅ R: ← turns exactly 15.000°, ↑ 15.000° (Wayland and X11); live navigation/75–80 directions, qa4 in-app | 0.4.10+ / 2a6d0167 10-27 |
| 3.2 | Shift+arrows 90°, Ctrl+arrows 5° | help | ✅ angles / 🔧 axes | 0.4.10+ / fa61ba6f 10-27 |
| 3.3 | Each step animates linearly over 250 ms | `animateRotation`, `CAMERA_SHORT_ANIMATION_TIME_MS` | 🔧 (was 500 ms, eased) |  |
| 3.4 | Holding an arrow rotates continuously: every key repeat starts another step that runs alongside the earlier ones (they add up) | help; `setAllowRepeat` for ROTATE_*, `startAnimation`; live navigation/910–921: a 1 s hold (X repeat 660 ms / 25 Hz, 11 keydowns) turns 165° | ✅ nav 10-19: a 1 s held ← turns 165°, as live navigation/910–921 |  |
| 3.5 | Ctrl+Shift+arrows pan: the model moves 150 px toward the arrow (measured at the scene centre), 250 ms smootherstep; ignored while the camera animates | help; `animatePan`, `animateTo` | ✅ R: Ctrl+Shift+→ moves the model exactly 150 px, no rotation | 0.4.10+ / 2a6d0167 10-27 |
| 3.6 | Z zooms out ×1.75, Shift+Z in ×1.75, about the view centre, 250 ms, clamped to the zoom limits | help; `animateZoom`, `adjustCameraToValidZoom` | ✅ R: Z ×1.75 out, Shift+Z ×1/1.75 in (world per pixel, exact) | 0.4.10+ / 2a6d0167 10-27 |
| 3.7 | All of 3.1–3.6 work in Assemblies too | `getIsInPartStudio() \|\| getIsInAssembly()` | 🔧 (Assembly had none) |  |
| 3.8 | Up/Down with the rollback bar selected move the bar instead | `handleRotateUp` | ✅ (FeatureList context) | 0.4.10+ / fa61ba6f 10-27 |
| 3.9 | View animations (standard views, fit, N) use Onshape's smootherstep easing | `animateTo` | 🔧 (was smoothstep) |  |

## 4. View cube

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 4.1 | Click a face: plane view; a corner/edge: that view — each zoomed to fit at the new orientation | help; `animateToStandardView` → `animateToFrameWithZoomFit` | ✅ qa4 10-23 (Top / Front, each zoomed to fit); R: a tap on the cube lands on a face, edge or corner view | 0.4.10+ / 2a6d0167 10-27 |
| 4.4 | Shift+1–7 standard views zoom to fit too | `handleFrontView` … | ✅ qa4 10-23 (Shift+1 = Front, zoomed to fit) | 0.4.10+ / 2a6d0167 10-27 |
| 4.5 | Zoom to fit (F, double middle click) frames the visible planes as well as parts and sketches (the model node's bounds), their box's projected corners padded ×1.05 | `getFitBounds`, `zoomToFit`, `CAMERA_ZOOM_TO_FIT_PADDING` | ✅ U `zoom_to_fit_frames_like_onshape` (parts only, ×1.12: the part looked ~1.39× bigger than in Onshape) | 0.4.10+ / 2a6d0167 10-27 |
| 4.2 | Click the ◀ ▶ ▲ ▼ arrows: rotate 15° (Shift 90°, Ctrl 5°), same animation as the keys | help | ✅ qa4 10-23 (▶ turns 15°, the Right face edge comes in) | 0.4.10+ / 2a6d0167 10-27 |
| 4.3 | Curved arrows: roll 15° about the line of sight | `rotateInDirection("clockwise")` | ✅ qa4 10-23 (the curved arrow rolls 15°) | 0.4.10+ / 2a6d0167 10-27 |

## 5. Mouse controls preference (My account › Preferences)

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 5.1 | Section "Mouse controls" after Units, before Part Studio settings: "View settings" dropdown (170 px) | live DOM | 🔧 (first section of our page: Language/Units sections are not local) |  |
| 5.2 | Presets: Onshape (default), SOLIDWORKS, NX 10, Creo, AutoCAD | client `viewManipulationMapping`; live 62 | ✅ U `presets_follow_their_tables` (app-core navigation, from the bundle) | 0.4.10+ / fa61ba6f 10-27 |
| 5.3 | Table "3D Part & Assembly" / "2D Drawing" with rows Rotate, Constrained rotate, Pan, Zoom per preset; "Alt + " prefix on Constrained rotate (moves to Rotate when constrained is the default) | live DOM; `mouseActionSettingsMap` | ✅ U `presets_follow_their_tables` (app-core navigation, from the bundle) | 0.4.10+ / fa61ba6f 10-27 |
| 5.4 | ☐ Reverse scroll wheel zoom direction | live DOM; `isScrollZoomDefault` | 🔧 |  |
| 5.5 | Choosing SOLIDWORKS ticks Reverse scroll wheel zoom direction (other presets leave it; switching back does not untick) | `setDefaultMouseScrollDirection`; live navigation/63–64 | 🔧 |  |
| 5.6 | ☐ Set default rotation behavior to constrained rotate (Alt then gives free rotate) | live DOM; `axisRotationLock` | ✅ U `constrained_rotate_default_swaps_alt` (app-core navigation, from the bundle) | 0.4.10+ / fa61ba6f 10-27 |
| 5.7 | ☐ Use pen input as mouse | live DOM | ✅ 10-25: stored and applied (`gpui::set_pen_as_mouse`): off = pen contacts are touches on every platform (row 7.10), on = mouse events | needs platform (pen input) |
| 5.8 | "Save mouse controls" (disabled until changed) saves and applies at once; nothing applies before saving | live DOM; live 61 | 🔧 |  |

## 6. Drawings (2D: pan and zoom only)

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 6.1 | Pan: right-drag, Ctrl+right-drag, middle-drag, Ctrl+middle-drag (default preset); the sheet follows 1:1; no context menu after a right-drag | Mouse controls 2D column; live navigation/41–43 | 🔧 |  |
| 6.2 | Other presets' 2D bindings (e.g. SOLIDWORKS Shift+middle zoom, Creo Ctrl+middle zoom) | Mouse controls 2D column | 🔧 |  |
| 6.3 | Wheel zoom about the pointer ×1.06 per notch (same as 3D); Shift/Ctrl+wheel the same; horizontal nothing; trackpad scroll and pinch as in 3D | live navigation/32–40 | 🔧 |  |
| 6.4 | No rotation of any kind; plain arrow keys do nothing | Mouse controls; live navigation/52–53 | ✅ | 0.4.10+ / fa61ba6f 10-27 |
| 6.5 | Z zooms out ×½, Shift+Z in ×2, about the view centre | live navigation/50–51 | 🔧 (was ×0.8/×1.25) |  |
| 6.6 | Ctrl+Shift+arrows move the sheet 10 % of the canvas width toward the arrow | live navigation/54–55 | 🔧 (missing) |  |
| 6.7 | Double middle click and F zoom to fit the sheet | live navigation/45–46, 56 | 🔧 (double middle missing) |  |
| 6.8 | Left-drag on empty sheet: box select | live navigation/44 | 🔧 dx 10-28 (after qa DRAWING-BOX-SELECT): the window box (#1651B0 20 % / 60 %) and what it wholly encloses selected, Shift adds (`Drag::SelectBox`, `select_in_box`); to verify on green | 867a7720 10-28 (wf-block drawing: drag from empty sheet → window box #1651B0 20 % fill, the enclosed view selected) |

## 7. Touchscreen (iOS, Android and browser table)

Raw touches reach the viewport through vendored gpui crates: core
(`vendor/gpui-pre`: touches go to `on_mouse_event::<TouchEvent>` listeners,
which can claim them), Windows (`vendor/gpui-pre-windows`: `WM_POINTER*` for
touch and pen), X11 (`vendor/gpui-pre-linux`: XInput 2.2 touch events) and
Wayland (`wl_touch`). Gestures are `touch.rs` (Onshape's recognizers).

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 7.1 | One-finger tap selects; double tap clears the selection / edits the feature under it (a double click) | help; `onTap` | ✅ L: a tap selects like a left click; R: Tap (1 finger) → click | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.2 | One-finger drag: 3D rotate (2 turns per diagonal, 2 px slop), inertia on release; on a tool's draggable geometry it drags it | help; `POINTER_ROTATION_CYCLES_PER_SCREEN`, single-pointer drag | ✅ L: 70.6841° = 190 px × 2 turns/diagonal (the first move only crosses the threshold); R: 84.31° = 190 px | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.3 | Two-finger tap: context menu | help | ✅ L: two-finger tap opens the context menu; R: Tap (2 fingers) → context menu | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.4 | Two-finger long press (750 ms, fingers within 10 px): box selection between the fingers (window), blocks other gestures | help; `TWO_POINTER_DELAYED_DRAG` | ✅ L: 750 ms hold → box between the fingers, camera still; R: BoxStart … BoxStop, camera unchanged. Fixed 10-25: timers already due fire before the next touch event, so a late timer on a busy UI thread can't turn the long press into a pan | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.5 | Two-finger drag pans (skipped when the fingers move very differently) | help; pan recognizer | ✅ L exact; R: 150 px move → 149.99 px | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.6 | Pinch zooms by the finger ratio about their centre (centre still within 15 px, turn under 10°) | help; pinch recognizer | ✅ L ×2.000000; R ×2.00004 | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.7 | Two-finger rotate rolls the view about the fingers' centre | help; `rotateAboutCanvasPoint` | ✅ L 30.0000°; R 29.993° | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.8 | Taps: lifted within 200 ms; reported 200 ms after the last (double tap); fingers overlapping in time are one multi-finger tap | tap recognizer | ✅ fixed 10-24: a tap is reported 200 ms after the lift even while the long-press timer runs (it waited 750 ms after the touch); R | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.9 | Touches on overlays above the graphics area (view cube, dialogs) stay normal taps | — | ✅ R: a tap on the view cube turns the view to a standard view; the viewport recognizer sees nothing | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.10 | Pen acts as a finger unless "Use pen input as mouse" | Preferences | ✅ 10-25: Onshape sends `pointerType` "pen" to its touch recognizers unless "Use pen input as mouse" (`handlesEvent`). Windows (WM_POINTER PT_PEN), macOS (tablet-point mouse events), X11 (stylus devices: an "Abs Pressure" valuator) and Wayland (`zwp_tablet_v2`, new) now deliver pen contacts as touches; hovering moves the mouse. R: a pen drag of 200 px turns 190 px at 2 turns/diagonal, a pen tap is a one-finger tap, hovering moves nothing (Wayland and X11) | needs platform (touchpad / touch / macOS / Windows; skipped on Linux Xvfb) |
| 7.11 | Trimetric / plane views from the view cube menu, 3D Rotate Lock | help (mobile apps) | ❌ mobile apps only |  |

## Re-check of the 🔧 rows (qa4, 2026-10-23 23:30–00:30, app /root/qa4-scratch/bin/noshape-a0e = main a0e360a3, Xvfb 1600×1000, demo plate; screenshots /tmp/qa4/cw)
Method: xdotool input on X11, model extents measured from pixels (front face colour).

**Verified in the app ✅**
| Row | Measured |
|---|---|
| 1.8 pan keeps the point under the cursor | middle-drag (+100, +50) px → model moved (+100, +46) px; Ctrl+right-drag back → (0, 0) |
| 1.9 Shift+right-drag rotates | Front → Right face comes in (no pan) |
| 1.5 Alt+right-drag | from a tilted view (↑ then ←) snaps floor-down and turns about Z (ends on Right, Z up) |
| 1.19 zoom-out limit | 40 out-notches stop with the model ≈ 100 px across (front face 71 px) |
| 1.20 horizontal scroll | 0 px change |
| 1.20b Shift/Ctrl+wheel | 3 Shift + 3 Ctrl notches → ×1.416 (= 1.06⁶ 1.419) |
| 3.1 arrows | ← from Front brings Right in, ↑ brings Bottom in |
| 3.2 Shift+arrow | Shift+← from Front lands exactly on Right |
| 3.5 Ctrl+Shift+→ | model moves 150 px |
| 3.6 Z | ×1/1.77 about the centre (= 1.75; edge antialiasing) |
| 4.1 / 4.4 view cube face, Shift+1 | Top / Front, each zoomed to fit from a zoomed-in camera |
| 4.2 / 4.3 cube arrows | ▶ rotates 15° (Right face edge comes in), curved arrow rolls 15° |

**Covered by unit tests, not separately in the app:** 1.1, 1.2 (`drag_rotate_rate_matches_live`), 1.4 (`inertia_decays_like_onshape`),
1.16–1.18 (zoom tests), 3.3 / 3.9 (animation timing), 2.7 pinch ratio (`ortho_zoom_amounts_and_pinch_follow_fingers`).

**Not conclusive here:** 1.11 (Ctrl pressed mid-drag): the screenshot after rotate → Ctrl-pan is plausible but I couldn't separate the two
parts from one frame; 1.3 rotate centre, 1.12, 1.13, 1.21, 3.4 (key repeat), 3.7 (Assembly), 5.x (Mouse controls page), 6.x (drawing) not
re-run this pass.

**Needs real hardware (cannot be tested on Xvfb / xdotool):** 2.4–2.9 (touchpad scroll, momentum, precision touchpad, Linux smooth scroll,
pinch, macOS Ctrl-click: X11 xdotool only sends discrete wheel buttons), 7.1–7.10 (touchscreen taps, drags, two-finger gestures, pen; no
touch device on this server), 5.7 pen-as-mouse. Proposal: a CI macOS runner can drive 2.4 / 2.7 / 2.9 with synthetic NSEvent scroll and
magnify events; touchscreen rows need a Windows touch device or a recorded event stream fed to `touch.rs` in tests.

## Replay with real OS input (nav2, 2026-10-24)
`.github/nav/linux-replay.sh <noshape> .github/nav/block.noshape <out> wayland|x11` (root: uinput)
drives the app through the kernel: a uinput mouse, keyboard, clickpad and touch screen read by
libinput in a headless sway, the app on Wayland or on X11 through Xwayland. `check.py`
recomputes every expected number independently (Chromium's rules on the WAYLAND_DEBUG protocol
log or on the posted events, Onshape's formulas) and compares each camera change.
Local run 10-25 14:45: **Wayland 485/485, X11 254/254 checks pass** (fewer lines than before: wheel events are now checked by running totals) (wheel notches and 120ths,
Shift/Ctrl+wheel, horizontal wheel, two-finger scroll up/down/sideways, pinch out/in, right-drag
with its rotate centre fixed on screen, Alt+right-drag, middle-drag, touch rotate/pan/pinch/roll,
tap, two-finger tap, two-finger long press, Ctrl pressed mid-drag, wheel during a box selection, a tap on the view cube, a drag released over the feature list, a second button pressed mid-drag, pen drag / tap / hover, arrow / Shift / Ctrl / Ctrl+Shift arrow keys, Z and Shift+Z). The checks assert per-event numbers, never timing. macOS (CGEvent scroll with
phases and momentum, drags) and Windows (SendInput wheel, InjectTouchInput touch, drags) run in
`.github/workflows/nav.yml`; run 40306609140 (10-26): all three pass, Windows touch pan / pinch /
roll included. The Windows driver warms up UI Automation before the steps (gpui's a11y otherwise
starts mid-run and stalls the app) and holds each touch frame with keep-alive updates, steps
sized to Onshape's per-event limits: on the runners' software renderer Windows coalesces touch
updates, and slow fingers would start the two-finger long-press box.
