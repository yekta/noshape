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

Code: `crates/app/src/navigation.rs` (all input→camera math, unit tested:
`cargo test -p noshape navigation`), `viewport.rs` (3D input), the drawing
canvas (`drawing_tools.rs` / `drawing_view.rs`, 2D).

Status: ✅ works · 🔧 fixed/added in this pass · ⏳ partial · ❌ missing.

## 1. Mouse (desktop, default "Onshape" mouse controls)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 1.1 | Right-drag: 3D rotate, free (about the camera's own up/right axes, not a turntable) | help table; `onDrag` ROTATE | 🔧 (was a world-Z turntable) |
| 1.2 | Rotation speed: 3 full turns per viewport diagonal | `ROTATION_CYCLES_PER_SCREEN` | 🔧 (was π·1.2 per height) |
| 1.3 | Rotate centre: the geometry under the cursor; else the model centre when it fits the view; else the cursor ray at the model's depth | `getRotateCenterForCanvasLocation` | 🔧 (fallback was the camera target; Onshape's middle step, the average depth of visible geometry, is approximated by the model centre) |
| 1.4 | Rotation inertia: speed averaged per step, continues if released within 50 ms, decays ×0.77 per 60 Hz frame (time based), stops under 1 px/step | `performStopRotationAnimation` | 🔧 |
| 1.5 | Alt+right-drag: constrained rotate (horizontal = about world Z, vertical = pitch); first animates to the nearest floor-down view (125 ms, 250 ms if over 45°) | help (Windows column); `AXIS_ROTATE` | 🔧 |
| 1.6 | Ctrl+right-drag: 2D pan | help table | ✅ |
| 1.7 | Middle-drag: 2D pan | help table | ✅ |
| 1.8 | Pan keeps the point under the cursor under it (perspective: in the plane through the geometry under the cursor) | `pan` | 🔧 (perspective used the target depth) |
| 1.9 | Shift+right-drag rotates (Shift is not bound; it does not pan) | `getClosestMode`, `usesShiftKey` | 🔧 (panned) |
| 1.10 | Ctrl+middle-drag pans (closest binding) | `getClosestMode` | ✅ |
| 1.11 | Pressing/releasing Ctrl or Alt mid-drag switches mode (rotate ↔ pan ↔ constrained) without lifting the button | `updateManipulationMode` per drag event | 🔧 |
| 1.12 | A second button pressed mid-drag re-resolves the mode | `updateInputState` | 🔧 |
| 1.13 | Releasing outside the graphics area ends the drag | — | 🔧 (`on_mouse_up_out`) |
| 1.14 | Right-click (no drag): context menu; a right-drag opens none | Home__context_menus | ✅ |
| 1.15 | Double-click middle button: zoom to fit (animated) | `onMiddleMouseButtonDoubleClick` | ✅ |
| 1.16 | Scroll wheel up zooms in, down zooms out, about the cursor (also when zooming out) | help; `zoomOnCanvasPoint`, `USE_CENTRAL_ZOOM_OUT` 0; live navigation/11–14 (fixed point = pointer both ways) | ✅ (direction) / 🔧 (amounts) |
| 1.17 | Zoom per notch: scale 2^(−i/6) with i = 0.5 per notch (×3 on Windows): ×1.06 per notch on macOS/Linux, ×1.19 on Windows | `zoomOnMouseWheelScroll`; live (Linux Chrome): ×1.060 per notch, up then down returns exactly | 🔧 (was e^(−0.1)) |
| 1.18 | Perspective zoom dollies the eye along the cursor ray by distance × i/6, within 0.05–50 scene diameters | `zoomPoint`, `dollyAlong` | 🔧 |
| 1.19 | Zoom limits: no zoom out once the model is under 100 px across, no zoom in past 2^24 px/m | `canZoom` | 🔧 |
| 1.20 | Horizontal scroll (tilt wheel, sideways swipe) does nothing | `zoomOnMouseWheelScroll` (`wheelDeltaX > wheelDeltaY` → ignored); live navigation/17 | 🔧 (Shift+trackpad scroll panned) |
| 1.20b | Shift+wheel and Ctrl+wheel zoom exactly like the wheel (Linux; macOS and Chrome on Windows turn Shift+wheel horizontal before Onshape sees it, so it does nothing there) | live navigation/15–16 | 🔧 (gpui's Linux Shift swap undone) |
| 1.21 | Scrolling during a box selection does nothing | `onMouseWheelScroll` | 🔧 |
| 1.22 | Left-drag: box select (left→right window, right→left crossing); never navigates | Home__selection; no flag for the left button | ✅ |

## 2. Touchpad / trackpad

| # | Behaviour | Source | Status |
|---|---|---|---|
| 2.1 | Windows: right-touchpad-button click + drag rotates; Ctrl + it pans | help table | ✅ (they are right-button events) |
| 2.2 | macOS: two-finger click + drag rotates (a secondary click) | help table | ✅ (NSRightMouseDown) |
| 2.3 | macOS: Ctrl + two-finger click + drag pans | help table | ✅ (gpui keeps Ctrl on a real right button) |
| 2.4 | macOS: two-finger scroll zooms (up = in, down = out), about the pointer, 40 px of scroll = one notch | help table; Chrome `wheelDelta` = 3 × pixels (the live CDP test saw one ×1.06 step per event whatever its size, but CDP synthesizes ±1 wheel tick per event; real precise deltas reach Onshape's `i = 0.5/120 × wheelDelta` proportionally) | 🔧 |
| 2.5 | Scroll momentum after lifting the fingers keeps zooming smoothly (each momentum event zooms by its own small delta; no jumps) | Chrome forwards momentum `mousewheel` events | 🔧 (unit tested: tiny deltas give tiny zooms) |
| 2.6 | Windows precision touchpad: two-finger scroll zooms (100 px = one notch, ×3) | Chrome DirectManipulation wheel | 🔧 |
| 2.7 | Pinch zooms about the pinch centre; the model scales exactly with the fingers (pinch in/out of the same distance returns to the same zoom) | help (Windows pinch); `pinchZoom` (zoom by 1/ratio about the centre) | 🔧 (pinch was ignored) — macOS magnify, Windows DirectManipulation, X11 XI2.4 and Wayland gestures all deliver `PinchEvent` |
| 2.8 | Linux touchpads: two-finger scroll zooms (X11 smooth scroll in notches, Wayland pixels at 45 px per notch), pinch as 2.7 | Chrome on Linux | 🔧 |
| 2.9 | macOS one-finger Ctrl+click is a right click | macOS | ⏳ gpui turns Ctrl+left into a plain right button, so a one-finger Ctrl+click-drag rotates (Chrome reports it as a left button with Ctrl, which does not navigate); two-finger Ctrl+click (2.3) is unaffected |

## 3. Keyboard

| # | Behaviour | Source | Status |
|---|---|---|---|
| 3.1 | Arrow keys rotate 15° about the camera's up (←/→) or right (↑/↓) axis through the camera target; ← from Front brings the Right face in, ↑ the Bottom (= dragging that way) | help; `rotateInDirection`; live navigation/75–80 | 🔧 (was about world Z, opposite sense) |
| 3.2 | Shift+arrows 90°, Ctrl+arrows 5° | help | ✅ angles / 🔧 axes |
| 3.3 | Each step animates linearly over 250 ms | `animateRotation`, `CAMERA_SHORT_ANIMATION_TIME_MS` | 🔧 (was 500 ms, eased) |
| 3.4 | Holding an arrow rotates continuously: every key repeat starts another step that runs alongside the earlier ones (they add up) | help; `setAllowRepeat` for ROTATE_*, `startAnimation` (concurrent, unnamed) | 🔧 (each repeat restarted one animation). Live CDP hold turned only ~105°/1.2 s; re-measure with real X autorepeat requested |
| 3.5 | Ctrl+Shift+arrows pan: the model moves 150 px toward the arrow (measured at the scene centre), 250 ms smootherstep; ignored while the camera animates | help; `animatePan`, `animateTo` | 🔧 (Part Studio: 500 ms; Assembly: missing) |
| 3.6 | Z zooms out ×1.75, Shift+Z in ×1.75, about the view centre, 250 ms, clamped to the zoom limits | help; `animateZoom`, `adjustCameraToValidZoom` | 🔧 (was ×1.25 instant) |
| 3.7 | All of 3.1–3.6 work in Assemblies too | `getIsInPartStudio() \|\| getIsInAssembly()` | 🔧 (Assembly had none) |
| 3.8 | Up/Down with the rollback bar selected move the bar instead | `handleRotateUp` | ✅ (FeatureList context) |
| 3.9 | View animations (standard views, fit, N) use Onshape's smootherstep easing | `animateTo` | 🔧 (was smoothstep) |

## 4. View cube

| # | Behaviour | Source | Status |
|---|---|---|---|
| 4.1 | Click a face: plane view; a corner: trimetric/isometric view | help | ✅ |
| 4.2 | Click the ◀ ▶ ▲ ▼ arrows: rotate 15° (Shift 90°, Ctrl 5°), same animation as the keys | help | 🔧 (axes/animation as 3.1–3.4) |
| 4.3 | Curved arrows: roll 15° about the line of sight | `rotateInDirection("clockwise")` | 🔧 (animation) |

## 5. Mouse controls preference (My account › Preferences)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 5.1 | Section "Mouse controls" after Units, before Part Studio settings: "View settings" dropdown (170 px) | live DOM | 🔧 (first section of our page: Language/Units sections are not local) |
| 5.2 | Presets: Onshape (default), SOLIDWORKS, NX 10, Creo, AutoCAD | client `viewManipulationMapping`; live 62 | 🔧 |
| 5.3 | Table "3D Part & Assembly" / "2D Drawing" with rows Rotate, Constrained rotate, Pan, Zoom per preset; "Alt + " prefix on Constrained rotate (moves to Rotate when constrained is the default) | live DOM; `mouseActionSettingsMap` | 🔧 |
| 5.4 | ☐ Reverse scroll wheel zoom direction | live DOM; `isScrollZoomDefault` | 🔧 |
| 5.5 | Choosing SOLIDWORKS ticks Reverse scroll wheel zoom direction (other presets leave it; switching back does not untick) | `setDefaultMouseScrollDirection`; live navigation/63–64 | 🔧 |
| 5.6 | ☐ Set default rotation behavior to constrained rotate (Alt then gives free rotate) | live DOM; `axisRotationLock` | 🔧 |
| 5.7 | ☐ Use pen input as mouse | live DOM | ⏳ (stored; no pen input path in gpui yet) |
| 5.8 | "Save mouse controls" (disabled until changed) saves and applies at once; nothing applies before saving | live DOM; live 61 | 🔧 |

## 6. Drawings (2D: pan and zoom only)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.1 | Pan: right-drag, Ctrl+right-drag, middle-drag, Ctrl+middle-drag (default preset); the sheet follows 1:1; no context menu after a right-drag | Mouse controls 2D column; live navigation/41–43 | 🔧 |
| 6.2 | Other presets' 2D bindings (e.g. SOLIDWORKS Shift+middle zoom, Creo Ctrl+middle zoom) | Mouse controls 2D column | 🔧 |
| 6.3 | Wheel zoom about the pointer ×1.06 per notch (same as 3D); Shift/Ctrl+wheel the same; horizontal nothing; trackpad scroll and pinch as in 3D | live navigation/32–40 | 🔧 |
| 6.4 | No rotation of any kind; plain arrow keys do nothing | Mouse controls; live navigation/52–53 | ✅ |
| 6.5 | Z zooms out ×½, Shift+Z in ×2, about the view centre | live navigation/50–51 | 🔧 (was ×0.8/×1.25) |
| 6.6 | Ctrl+Shift+arrows move the sheet 10 % of the canvas width toward the arrow | live navigation/54–55 | 🔧 (missing) |
| 6.7 | Double middle click and F zoom to fit the sheet | live navigation/45–46, 56 | 🔧 (double middle missing) |
| 6.8 | Left-drag on empty sheet: box select | live navigation/44 | ✅ |

## 7. Touchscreen (iOS, Android and browser table)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 7.1 | One-finger tap selects, double-tap clears the selection | help | ❌ (gpui's desktop platforms deliver no touch events; Windows synthesizes mouse input) |
| 7.2 | One-finger drag: 3D rotate (2 turns per diagonal) | help; `POINTER_ROTATION_CYCLES_PER_SCREEN` | ❌ |
| 7.3 | Two-finger tap: context menu; two-finger long press: box select | help | ❌ |
| 7.4 | Two-finger pinch zooms, two-finger drag pans, two-finger rotate rolls the view about the centre | help; `PINCH_ZOOM`, `PAN`, `ROTATE` gestures | ❌ |
| 7.5 | View cube tap opens the view list (mobile apps) | help | ❌ (mobile only) |
| 7.6 | 3D Rotate Lock button (mobile apps) | help | ❌ (mobile only) |
