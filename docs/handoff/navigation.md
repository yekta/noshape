# Handoff: view navigation (agent "nav", interaction engineer)

Task: view navigation exactly like Onshape on every input device and
platform (mouse, trackpad incl. pinch, keyboard, view cube, touchscreen,
Preferences › Mouse controls) in Part Studio, Assembly and Drawing.
Checklist: `docs/parity/navigation.md`.

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
* Pinch: the model scales by the finger ratio about the pinch centre.
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

## Next

1. Verify in the app (xdotool right-drag, wheel, Alt-drag, arrows, Z) and
   compare with live screenshots.
2. Touchscreen (checklist §7): gpui's Windows/Linux/macOS backends emit no
   `TouchEvent`; needs WM_POINTER handling in a vendored
   `gpui-pre-windows` (like `vendor/gpui-pre-linux`) and an element-level
   touch listener. Lead decision needed before vendoring.
3. Pivot fallback: Onshape averages visible depth samples before using the
   model centre; we skip that step (no depth readback).

## Gotchas

* macOS gpui turns Ctrl+left-click into a plain right button (control
  cleared), so we cannot tell it from a real right click.
* Windows gpui turns Shift+wheel into horizontal lines; Linux too. That is
  exactly why Shift+wheel does nothing (as in Onshape's browser client).
* `on_mouse_up` only fires over the element; `on_mouse_up_out` ends drags
  released outside.
