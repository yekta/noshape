//! Shared look of the drawing tool dialogs' header buttons (live drawing/124,
//! 127, 180, 261, 468, 494): Onshape's drawing dialogs draw the ✕ in
//! `#dd5228` (the Part Studio's is `#bd3039`), and a disabled ✓ keeps its
//! white check on the pale green `#cae8ca`.

use gpui_kit::*;

/// The disabled ✓ cell's fill.
pub const OK_DISABLED: u32 = 0xcae8ca;

/// The ✕ of a drawing tool dialog's header.
pub fn cancel_x(cx: &App) -> AnyElement {
    crate::assets::icon("drawing-cancel", 17., cx)
}

/// The drawing tool dialogs' shadow (live dom drawing-detail-dialog:
/// `rgba(154, 165, 175, 0.7) 0 0 4px`).
pub fn dialog_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(208. / 360., 0.12, 0.645, 0.7), offset: point(px(0.), px(0.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }]
}
