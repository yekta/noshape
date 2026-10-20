//! The 3D viewport's colors (Onshape's viewport CSS tokens and sampled live
//! values), shared by the app's theme and the headless scene builder.

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

/// Linear RGBA from sRGB hex (renderer colors).
pub fn lin(hex: u32) -> [f32; 4] {
    noshape_render::srgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// sRGB bytes with full alpha (per-primitive renderer colors).
pub const fn rgb8(hex: u32) -> [u8; 4] {
    [((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8, 255]
}
