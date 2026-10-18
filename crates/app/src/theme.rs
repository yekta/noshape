//! noshape's visual identity: Onshape's light theme. Colors are Onshape's
//! design tokens (`onshape-design-tokens.css`, light theme); the token each
//! value comes from is noted next to it.

use gpui_kit::Rgba;

#[derive(Clone, Copy)]
pub struct Palette {
    /// Main window and viewport background (`--os-background-primary`).
    pub app_bg: Rgba,
    /// Panels: feature list, toolbar, dialogs (`--os-panel-background--active`).
    pub panel: Rgba,
    /// Popovers and menus (`--os-context-menu-background--idle`).
    pub panel_raised: Rgba,
    /// Hovered buttons and icon buttons (`--os-icon-button-fill--hover`).
    pub panel_hover: Rgba,
    /// Toggled-on icon buttons (`--os-icon-button-fill--selected`).
    pub panel_active: Rgba,
    /// Toolbar tool hover (measured live: `#eaeaea`).
    pub tool_hover: Rgba,
    /// Hover of a split tool group, both halves (`--os-icon-button-fill--hover-group`).
    pub tool_hover_group: Rgba,
    /// Toolbar tool whose dialog is open (`--os-select-primary`).
    pub tool_active: Rgba,
    /// Left and right panel rails (`--os-left-panel-background`).
    pub rail: Rgba,
    /// Keyboard focus ring and focused form fields (`--os-focus`).
    pub focus: Rgba,
    /// Rollback bar (`#cccccc` at brightness .9).
    pub rollback: Rgba,
    /// Rolled-back feature rows (`--os-color-noreaster-200`).
    pub text_rolled_back: Rgba,
    /// Invalid text: failed features, invalid dialog titles (`--os-text-invalid`).
    pub text_invalid: Rgba,
    /// Feature dialog body (`--os-feature-dialog-background`, white at 95%).
    pub dialog_bg: Rgba,
    /// Tool search box (`--os-tool-search-button-fill--idle`).
    pub search_fill: Rgba,
    /// Info alerts: fill, outline, text (`--os-info-*`).
    pub info_fill: Rgba,
    pub info_outline: Rgba,
    pub info_text: Rgba,
    /// Error alerts: fill and outline (`--os-error-*`).
    pub error_fill: Rgba,
    pub error_outline: Rgba,
    /// Warning alerts: fill and outline (`--os-warn-*`).
    pub warn_fill: Rgba,
    pub warn_outline: Rgba,
    /// Boxed form field outline (`--os-form-field-outline`).
    pub field_outline: Rgba,
    /// Context menu outline (`--os-context-menu-outline`).
    pub menu_outline: Rgba,
    /// Inactive panels, the document bar (`--os-navbar-background`).
    pub panel_inactive: Rgba,
    /// Dividers and panel outlines (`--os-outline-tertiary`).
    pub border: Rgba,
    /// Input and button outlines (`--os-outline-secondary`).
    pub border_strong: Rgba,
    /// Primary text (`--os-text-primary`).
    pub text: Rgba,
    /// Secondary text (`--os-text-secondary`).
    pub text_muted: Rgba,
    /// Inactive text, rolled-back features (`--os-tree-item-text--inactive`).
    pub text_faint: Rgba,
    /// Onshape blue (`--os-accent-primary`).
    pub accent: Rgba,
    /// Primary button hover (`--os-button-fill-primary--hover`).
    pub accent_hover: Rgba,
    /// Selected list rows (`--os-tree-item-background--selected`).
    pub accent_soft: Rgba,
    /// Hovered list rows and menu items (`--os-tree-item-background--hover`).
    pub row_hover: Rgba,
    /// Tooltip fill (`--os-tooltip-fill`).
    pub tooltip: Rgba,
    /// Bottom tab bar background (`--os-tab-bar-background`).
    pub tab_bar: Rgba,
    /// Inactive tab (`--os-tab-bar-tab--inactive`).
    pub tab_inactive: Rgba,
    /// Feature dialog accept check (`--os-feature-dialog-accept--idle`).
    pub accept: Rgba,
    /// Feature dialog cancel X (`--os-icon-accent-cancel`).
    pub cancel: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub info: Rgba,
    pub overlay_shadow: Rgba,
}

pub const PALETTE: Palette = Palette {
    app_bg: rgb_const(0xffffff),
    panel: rgb_const(0xffffff),
    panel_raised: rgb_const(0xffffff),
    panel_hover: rgb_const(0xeaeaea),
    panel_active: rgb_const(0xeaeaea),
    tool_hover: rgb_const(0xeaeaea),
    tool_hover_group: rgb_const(0xd7d7d7),
    tool_active: rgb_const(0xb2ddf6),
    rail: rgb_const(0xfafafa),
    focus: rgb_const(0x3e94ff),
    rollback: rgb_const(0xb8b8b8),
    text_rolled_back: rgb_const(0x999999),
    text_invalid: rgb_const(0xaa252d),
    dialog_bg: rgba_const(0xfffffff2),
    search_fill: rgb_const(0xeeeeee),
    info_fill: rgb_const(0xdef1fb),
    info_outline: rgb_const(0xb2ddf6),
    info_text: rgb_const(0x094174),
    error_fill: rgb_const(0xf9ebec),
    error_outline: rgb_const(0xdd979b),
    warn_fill: rgb_const(0xfff8e6),
    warn_outline: rgb_const(0xfcda7f),
    field_outline: rgb_const(0xaaaaaa),
    menu_outline: rgb_const(0xcccccc),
    panel_inactive: rgb_const(0xeaeaea),
    border: rgb_const(0xd7d7d7),
    border_strong: rgb_const(0xaaaaaa),
    text: rgb_const(0x333333),
    text_muted: rgb_const(0x666666),
    text_faint: rgb_const(0x999999),
    accent: rgb_const(0x1651b0),
    accent_hover: rgb_const(0x769fcc),
    accent_soft: rgb_const(0xb2ddf6),
    row_hover: rgb_const(0xdef1fb),
    tooltip: rgb_const(0xd0dbe5),
    tab_bar: rgb_const(0xd7d7d7),
    tab_inactive: rgb_const(0xeaeaea),
    accept: rgb_const(0x009400),
    cancel: rgb_const(0xbd3039),
    success: rgb_const(0x009400),
    warning: rgb_const(0xfab601),
    error: rgb_const(0xbd3039),
    info: rgb_const(0x1651b0),
    overlay_shadow: rgba_const(0x00000033),
};

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
    /// Errors (`ERROR_HIGHLIGHT_COLOR1`).
    pub const ERROR: u32 = 0xbd3039;
    /// Construction plane fill (`--os-construction-plane-ambient-color`),
    /// drawn unlit at 20%: one plane reads `#f5f8fa`, three stacked `#e7edf2`.
    pub const PLANE_FILL: u32 = 0xcedbe5;
    /// Construction plane outline (`--os-construction-plane-outline-color`
    /// `#1d60aa` at 35%) as it renders on white,
    /// drawn opaque because the renderer blends in linear light.
    pub const PLANE_OUTLINE_ON_WHITE: u32 = 0xaac3de;
    /// Construction plane label (`--os-construction-plane-text-color`).
    pub const PLANE_TEXT: u32 = 0x1651b0;
    /// Origin and point bodies (`--os-origin-color`).
    pub const ORIGIN: u32 = 0x000000;
    /// Sketch: under-defined (`--os-sketch-underconstrained-line-point-color`).
    pub const SKETCH_UNDER: u32 = 0x0000d0;
    /// Sketch: fully defined (`--os-dimension-line-arrow-color`).
    pub const SKETCH_FULL: u32 = 0x000000;
    /// Sketch: over-defined (`--os-sketch-overconstrained-line-point-color`).
    pub const SKETCH_OVER: u32 = 0xcd0000;
    /// Sketch: entities used from outside the sketch. A noshape convention
    /// (Onshape draws them in the constraint-state color); `--os-accent-nonary`.
    pub const SKETCH_EXTERNAL: u32 = 0x672f8f;
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

const fn rgb_const(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

const fn rgba_const(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 24) & 0xff) as f32 / 255.0,
        g: ((hex >> 16) & 0xff) as f32 / 255.0,
        b: ((hex >> 8) & 0xff) as f32 / 255.0,
        a: (hex & 0xff) as f32 / 255.0,
    }
}

pub fn p() -> &'static Palette {
    &PALETTE
}

pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

/// Linear RGBA from sRGB hex (renderer colors).
pub fn lin(hex: u32) -> [f32; 4] {
    noshape_render::srgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// sRGB bytes with full alpha (per-primitive renderer colors).
pub const fn rgb8(hex: u32) -> [u8; 4] {
    [((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8, 255]
}
