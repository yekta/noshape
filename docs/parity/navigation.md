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
| 1.3 | Rotate centre: the geometry under the cursor; else the average of ~2000 visible-depth samples (planes excluded, ≥ 4 hits); else the same with the planes' depth (only planes visible); else the model centre when it fits the view; else the cursor ray at the model's depth | `getRotateCenterForCanvasLocation`, `getRotateCenterBasedOnWindowDepths({includePlanes})`, `DepthSamplingPass` | 🔧 (render worker draws two ~2000 px sample passes, without and with plane depth, true depth without the pick bias, read back off the UI thread at the press) |
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
| 2.9 | macOS one-finger Ctrl+click: a click opens the context menu, a drag does nothing (Chrome reports a left button with Ctrl, which never navigates) | Chrome on macOS | 🔧 (vendor/gpui-pre-macos marks the converted press) |

## 3. Keyboard

| # | Behaviour | Source | Status |
|---|---|---|---|
| 3.1 | Arrow keys rotate 15° about the camera's up (←/→) or right (↑/↓) axis through the camera target; ← from Front brings the Right face in, ↑ the Bottom (= dragging that way) | help; `rotateInDirection`; live navigation/75–80 | 🔧 (was about world Z, opposite sense) |
| 3.2 | Shift+arrows 90°, Ctrl+arrows 5° | help | ✅ angles / 🔧 axes |
| 3.3 | Each step animates linearly over 250 ms | `animateRotation`, `CAMERA_SHORT_ANIMATION_TIME_MS` | 🔧 (was 500 ms, eased) |
| 3.4 | Holding an arrow rotates continuously: every key repeat starts another step that runs alongside the earlier ones (they add up) | help; `setAllowRepeat` for ROTATE_*, `startAnimation`; live navigation/910–921: a 1 s hold (X repeat 660 ms / 25 Hz, 11 keydowns) turns 165° | 🔧 verified: noshape turns 165° for the same hold |
| 3.5 | Ctrl+Shift+arrows pan: the model moves 150 px toward the arrow (measured at the scene centre), 250 ms smootherstep; ignored while the camera animates | help; `animatePan`, `animateTo` | 🔧 (Part Studio: 500 ms; Assembly: missing) |
| 3.6 | Z zooms out ×1.75, Shift+Z in ×1.75, about the view centre, 250 ms, clamped to the zoom limits | help; `animateZoom`, `adjustCameraToValidZoom` | 🔧 (was ×1.25 instant) |
| 3.7 | All of 3.1–3.6 work in Assemblies too | `getIsInPartStudio() \|\| getIsInAssembly()` | 🔧 (Assembly had none) |
| 3.8 | Up/Down with the rollback bar selected move the bar instead | `handleRotateUp` | ✅ (FeatureList context) |
| 3.9 | View animations (standard views, fit, N) use Onshape's smootherstep easing | `animateTo` | 🔧 (was smoothstep) |

## 4. View cube

| # | Behaviour | Source | Status |
|---|---|---|---|
| 4.1 | Click a face: plane view; a corner/edge: that view — each zoomed to fit at the new orientation | help; `animateToStandardView` → `animateToFrameWithZoomFit` | 🔧 (kept the zoom) |
| 4.4 | Shift+1–7 standard views zoom to fit too | `handleFrontView` … | 🔧 |
| 4.5 | Zoom to fit (F, double middle click) frames the visible planes as well as parts and sketches (the model node's bounds), their box's projected corners padded ×1.05 | `getFitBounds`, `zoomToFit`, `CAMERA_ZOOM_TO_FIT_PADDING` | 🔧 (parts only, ×1.12: the part looked ~1.39× bigger than in Onshape) |
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

Raw touches reach the viewport through vendored gpui crates: core
(`vendor/gpui-pre`: touches go to `on_mouse_event::<TouchEvent>` listeners,
which can claim them), Windows (`vendor/gpui-pre-windows`: `WM_POINTER*` for
touch and pen), X11 (`vendor/gpui-pre-linux`: XInput 2.2 touch events) and
Wayland (`wl_touch`). Gestures are `touch.rs` (Onshape's recognizers).

| # | Behaviour | Source | Status |
|---|---|---|---|
| 7.1 | One-finger tap selects; double tap clears the selection / edits the feature under it (a double click) | help; `onTap` | 🔧 |
| 7.2 | One-finger drag: 3D rotate (2 turns per diagonal, 2 px slop), inertia on release; on a tool's draggable geometry it drags it | help; `POINTER_ROTATION_CYCLES_PER_SCREEN`, single-pointer drag | 🔧 |
| 7.3 | Two-finger tap: context menu | help | 🔧 |
| 7.4 | Two-finger long press (750 ms, fingers within 10 px): box selection between the fingers (window), blocks other gestures | help; `TWO_POINTER_DELAYED_DRAG` | 🔧 |
| 7.5 | Two-finger drag pans (skipped when the fingers move very differently) | help; pan recognizer | 🔧 |
| 7.6 | Pinch zooms by the finger ratio about their centre (centre still within 15 px, turn under 10°) | help; pinch recognizer | 🔧 |
| 7.7 | Two-finger rotate rolls the view about the fingers' centre | help; `rotateAboutCanvasPoint` | 🔧 |
| 7.8 | Taps: lifted within 200 ms; reported 200 ms after the last (double tap); fingers overlapping in time are one multi-finger tap | tap recognizer | 🔧 |
| 7.9 | Touches on overlays above the graphics area (view cube, dialogs) stay normal taps | — | 🔧 (only touches starting on the viewport's own topmost hitbox are claimed) |
| 7.10 | Pen acts as a finger unless "Use pen input as mouse" | Preferences | 🔧 Windows; ⏳ Linux (tablet pens arrive as pointer devices) |
| 7.11 | Trimetric / plane views from the view cube menu, 3D Rotate Lock | help (mobile apps) | ❌ mobile apps only |

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
