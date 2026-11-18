# Noshape's changes to vendored gpui

Every change to this crate carries a `NOSHAPE PATCH` comment: `grep -rn "NOSHAPE PATCH" vendor/gpui-pre` lists them.
Carry each one over when gpui is updated. These change what existing code means, so a merge can undo them without
a compile error:

- **Box shadow blur is CSS's** (`BoxShadow::blur_sigma`, style.rs; used by `paint_drop_shadows` and
  `paint_inset_shadows`, window.rs). `blur_radius` is a CSS blur radius and the shaders blur with half of it as the
  Gaussian sigma, as CSS defines. Upstream passes the radius as the sigma, which draws every shadow twice as soft.
  Callers write live's CSS values as they are (`0 5px 10px` is `blur_radius: px(10.)`), never halved. Pinned by
  app-core `ui::tests::box_shadow_blur_is_css`.
- **Tooltips fade out over 150 ms** (`TOOLTIP_FADE_OUT`, elements/div.rs; `prepaint_tooltip` returns its opacity,
  window.rs). Pinned by the chrome feel row "tooltip / fades out".
- **X11 press times** (vendor/gpui-pre-linux: `last_press`, gpui_linux.rs; set in linux/x11/client.rs's button
  press): each press's number and the time since the one before by the X server's event times. The feature list's
  double-click (part_studio `ROW_DOUBLE_CLICK`, via app-core `viewport::platform_last_press`) uses it, because a
  busy UI thread handles a second click late (qa ROW-DBLCLICK).
