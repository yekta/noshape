//! The 3D viewport's colors (Onshape's viewport CSS tokens and sampled live
//! values), shared by the app's theme and the headless scene builder.

use std::sync::atomic::{AtomicBool, Ordering};

/// Viewport colors (sRGB hex; convert with [`lin`] for the renderer).
pub mod viewport {
    /// B-rep edges on shaded parts (`LINE_POINT_COLOR`).
    pub const EDGE: u32 = 0x000000;
    /// Edges in hidden-line / wireframe modes (`UNSHADED_BODY_LINE_COLOR`).
    pub const EDGE_UNSHADED: u32 = 0x000000;
    /// Wire bodies and inactive sketches (`--os-sketch-inactive-color`).
    pub const INACTIVE: u32 = 0x999999;
    /// Pre-highlight (`PRE_HIGHLIGHT_COLOR_BORDER`).
    pub const PREHIGHLIGHT: u32 = 0xffc685;
    /// Selection (`ENTITY_HIGHLIGHT_COLOR_BORDER`).
    pub const SELECTION: u32 = 0xffa90a;
    /// The fill of a picked sketch region, flat at `PICKED_REGION_ALPHA`
    /// over everything see-through, a feature preview included (live
    /// part-studio/230: the region tints the preview from above). Fitted in
    /// the app to live's (156,150,124) / (176,177,154) where the region shows
    /// through the preview's front-left / right faces (ours (162,152,121) /
    /// (173,169,149)).
    pub const PICKED_REGION: u32 = 0xb69d4c;
    pub const PICKED_REGION_ALPHA: f32 = 0.34;
    /// A point target (vertex, edge midpoint): an 11 px disc, a darker ring
    /// 3–4 px out and a light centre; [disc, ring, centre] hovered and
    /// selected (live selection/08, part-studio/qa4-r5-s9-6d, 6e).
    pub const POINT_HOVER: [u32; 3] = [0xf8c488, 0xc49a6a, 0xffd8ac];
    pub const POINT_SELECTED: [u32; 3] = [0xfac012, 0xc4960b, 0xffd65b];
    /// Bodies a feature creates, while its dialog is open (shaded, seen
    /// through; live part-studio/230 reads `#8494a0` on the lit faces).
    pub const FEATURE_PREVIEW: u32 = 0x92aec5;
    /// A Remove / Intersect tool body: cutting (dark gray) or missing the
    /// parts (dark red; live `#803f43` shaded).
    pub const TOOL_PREVIEW: u32 = 0x3a3d40;
    pub const TOOL_PREVIEW_FAIL: u32 = 0x8a3a3f;
    /// "Highlight boundary edges" (help: open edges in red).
    pub const BOUNDARY_EDGE: u32 = 0xe00000;
    /// Errors (`ERROR_HIGHLIGHT_COLOR1`).
    pub const ERROR: u32 = 0xbd3039;
    /// Faces of a failed feature's error geometry (a Feature pattern's
    /// ghost instances). Live "lp wall over" 2258, solved over two
    /// backgrounds: rgb(123,30,36) at α 0.60 on both the top and the front
    /// faces (so drawn flat here; the browser blends in sRGB). Our renderer
    /// blends in linear light, where [`ERROR_FILL_ALPHA`] gives the live
    /// results (#825d6b over the top, #693c45 over the front) within a few
    /// levels.
    pub const ERROR_FILL: u32 = 0x7b1e24;
    /// FeatureScript debug entities (`DebugColor.MAGENTA`; live 2111: the
    /// Helix end profiles' circles read #ff2bc1, their centre dots #e224e3).
    pub const DEBUG_MAGENTA: u32 = 0xff2bc1;
    pub const DEBUG_MAGENTA_POINT: u32 = 0xe224e3;
    pub const ERROR_FILL_ALPHA: f32 = 0.75;
    /// Construction plane fill (`--os-construction-plane-ambient-color`),
    /// drawn unlit at 20%: one plane reads `#f5f8fa`, three stacked `#e7edf2`.
    pub const PLANE_FILL: u32 = 0xcedbe5;
    /// Construction plane outline (`--os-construction-plane-outline-color`
    /// `#1d60aa` at 35%, which the browser blends in sRGB). Our renderer
    /// blends in linear light, so the fit is this color at
    /// [`PLANE_OUTLINE_ALPHA`]: one edge reads `#aac3de` on the plane fill
    /// like live, and the two edges of a plane seen edge-on (a sketch's
    /// normal view) add up darker, `#79a1cc` live (sketch/03).
    pub const PLANE_OUTLINE: u32 = 0x4191c8;
    pub const PLANE_OUTLINE_ALPHA: f32 = 0.6;
    /// Construction plane label (`--os-construction-plane-text-color`).
    pub const PLANE_TEXT: u32 = 0x1651b0;
    /// Origin and point bodies (`--os-origin-color`).
    pub const ORIGIN: u32 = 0x000000;
    /// Sketch: fully defined (`--os-dimension-line-arrow-color`).
    pub const SKETCH_FULL: u32 = 0x000000;
    /// Sketch: over-defined (`--os-sketch-overconstrained-line-point-color`).
    pub const SKETCH_OVER: u32 = 0xcd0000;
    /// Closed sketch regions of the sketch being edited, drawn at ~26%
    /// (`--os-sketch-region-fill-color` `#9aa5af33`; sampled live).
    pub const SKETCH_REGION: u32 = 0x9aa5af;
    /// Driven dimensions (`--os-dimension-driven-line-point-color`).
    pub const DIM_DRIVEN: u32 = 0x666666;
    /// Drawing editor: the flat gray surround of the sheet (sampled).
    pub const DRAWING_BG: u32 = 0xdcdcdc;
    /// Axis colors (`--os-axis-{x,y,z}-color`).
    pub const AXIS_X: u32 = 0xcd0000;
    pub const AXIS_Y: u32 = 0x009400;
    pub const AXIS_Z: u32 = 0x0000d0;
}

/// The dark theme's viewport colors where they differ from [`viewport`]
/// (the dark `--os-*` values; live dark/dk-929, dk-931).
pub mod viewport_dark {
    /// Viewport background (`--os-background-primary`).
    pub const BACKGROUND: u32 = 0x333333;
    /// Construction plane fill: `#666` at 20%, which the browser blends in
    /// sRGB; fitted for our linear blending, one plane reads `#3d3d3d` and
    /// three stacked `#4c4c4c` like live.
    pub const PLANE_FILL: u32 = 0x595959;
    /// Construction plane outline: `#fff` at 35% in sRGB; an edge peaks at
    /// `#848484` on the plane like live.
    pub const PLANE_OUTLINE: u32 = 0xffffff;
    pub const PLANE_OUTLINE_ALPHA: f32 = 0.2;
    pub const PLANE_TEXT: u32 = 0xffffff;
    pub const ORIGIN: u32 = 0xffffff;
    pub const SKETCH_FULL: u32 = 0xffffff;
    pub const SKETCH_OVER: u32 = 0xff6b6b;
    /// `--os-sketch-underconstrained-line-point-color`.
    pub const SKETCH_UNDER: u32 = 0x3e94ff;
    pub const SKETCH_REGION: u32 = 0xf2f2f2;
    pub const DIM_DRIVEN: u32 = 0xcccccc;
    pub const AXIS_X: u32 = 0xdd979b;
    pub const AXIS_Y: u32 = 0xa2db7d;
    pub const AXIS_Z: u32 = 0x90cef1;
}

static DARK: AtomicBool = AtomicBool::new(false);

/// Preferences › Theming › Dark mode, for the viewport (the app's theme sets it).
pub fn set_dark(on: bool) {
    DARK.store(on, Ordering::Relaxed);
}

/// Whether the viewport draws in Onshape's dark theme.
pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// `light`, or `dark` in the dark theme: `pick(viewport::ORIGIN, viewport_dark::ORIGIN)`.
pub fn pick<T>(light: T, dark: T) -> T {
    if is_dark() { dark } else { light }
}

/// A sketch color from the Graphics preferences as drawn in the theme: in
/// dark, the light defaults become Onshape's dark ones (fully defined
/// `#000` → `#fff`, underdefined `#0000d0` → `#3e94ff`; live dk-931–933);
/// colors the user picked stay as they are.
pub fn sketch_color_for_theme(hex: u32) -> u32 {
    match hex {
        0x000000 if is_dark() => viewport_dark::SKETCH_FULL,
        0x0000d0 if is_dark() => viewport_dark::SKETCH_UNDER,
        _ => hex,
    }
}

/// Viewport background: white, `#333` in the dark theme.
pub fn background() -> [f32; 4] {
    lin(pick(0xffffff, viewport_dark::BACKGROUND))
}

/// Linear RGBA from sRGB hex (renderer colors).
pub fn lin(hex: u32) -> [f32; 4] {
    noshape_render::srgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// sRGB bytes with full alpha (per-primitive renderer colors).
pub const fn rgb8(hex: u32) -> [u8; 4] {
    [((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8, 255]
}
