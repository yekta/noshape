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
    /// Modal dialogs (Create Drawing, Select a part…): opaque white over a
    /// 50% black backdrop (live drawing/01, assembly/100).
    pub modal_bg: Rgba,
    pub modal_backdrop: Rgba,
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
    /// Active tab titles and other emphasized text (`--os-emphasis-fill`;
    /// the tab's underline stays `accent`).
    pub emphasis: Rgba,
    /// Primary button hover (`--os-button-fill-primary--hover`).
    pub accent_hover: Rgba,
    /// Selected list rows (`--os-tree-item-background--selected`).
    pub accent_soft: Rgba,
    /// Hovered list rows and menu items (`--os-tree-item-background--hover`).
    pub row_hover: Rgba,
    /// Table header cells (`--os-background-septenary`: light #eaeaea like
    /// hovers, but #555 in dark; live dk-928).
    pub table_header: Rgba,
    /// `--os-background-senary`: white, #1c1c1c in dark. Table data cells and
    /// the open panel's strip button (live dk-928, dk-929, dk-961).
    pub bg_senary: Rgba,
    /// The right strip's other buttons while a panel is open (#eaeaea; dark
    /// #1c1c1c like every strip button, live dk-929).
    pub strip_inactive: Rgba,
    /// A selection (query) box while it takes the picks
    /// (`--os-info-background-secondary`; dark live dk-926).
    pub query_active: Rgba,
    /// Feature rows related to the selected one, e.g. the sketch of a
    /// selected extrude (light sampled; dark live dk-926 rgb(9,32,70)).
    pub row_related: Rgba,
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
    modal_bg: rgb_const(0xffffff),
    modal_backdrop: rgba_const(0x00000080),
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
    emphasis: rgb_const(0x1651b0),
    accent_hover: rgb_const(0x769fcc),
    accent_soft: rgb_const(0xb2ddf6),
    row_hover: rgb_const(0xdef1fb),
    table_header: rgb_const(0xeaeaea),
    bg_senary: rgb_const(0xffffff),
    strip_inactive: rgb_const(0xeaeaea),
    query_active: rgb_const(0xdef1fb),
    row_related: rgb_const(0xecf0f4),
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

/// Viewport colors (shared with the headless scene builder).
pub use noshape_scene::palette::{pick, viewport, viewport_dark};

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

/// Onshape's dark theme (Preferences › Theming › Dark mode): the same tokens
/// resolved under `data-os-theme="dark"` (live dom/dark-os-vars-computed.json).
pub const PALETTE_DARK: Palette = Palette {
    app_bg: rgb_const(0x333333),
    panel: rgb_const(0x333333),
    panel_raised: rgb_const(0x333333),
    panel_hover: rgb_const(0x666666),
    panel_active: rgb_const(0x555555),
    tool_hover: rgb_const(0x666666),
    tool_hover_group: rgb_const(0x666666),
    tool_active: rgb_const(0x134e96),
    rail: rgb_const(0x2c2c2c),
    focus: rgb_const(0x3e94ff),
    rollback: rgb_const(0xb8b8b8),
    text_rolled_back: rgb_const(0xcccccc),
    text_invalid: rgb_const(0xff6b6b),
    dialog_bg: rgba_const(0x333333f2),
    modal_bg: rgb_const(0x333333),
    modal_backdrop: rgba_const(0x00000080),
    search_fill: rgb_const(0x555860),
    info_fill: rgb_const(0x094174),
    info_outline: rgb_const(0x467bb6),
    info_text: rgb_const(0xd0dbe5),
    error_fill: rgb_const(0x683232),
    error_outline: rgb_const(0x932229),
    warn_fill: rgb_const(0x6a4f06),
    warn_outline: rgb_const(0x987107),
    field_outline: rgb_const(0x888888),
    menu_outline: rgb_const(0xcccccc),
    panel_inactive: rgb_const(0x1c1c1c),
    border: rgb_const(0x666666),
    border_strong: rgb_const(0x888888),
    text: rgb_const(0xffffff),
    text_muted: rgb_const(0xcccccc),
    text_faint: rgb_const(0x999999),
    accent: rgb_const(0x3e94ff),
    emphasis: rgb_const(0x90cef1),
    accent_hover: rgb_const(0x769fcc),
    accent_soft: rgb_const(0x134e96),
    row_hover: rgb_const(0x0a4780),
    table_header: rgb_const(0x555555),
    bg_senary: rgb_const(0x1c1c1c),
    strip_inactive: rgb_const(0x1c1c1c),
    query_active: rgb_const(0x142434),
    row_related: rgb_const(0x092046),
    tooltip: rgb_const(0x4b5761),
    tab_bar: rgb_const(0x555555),
    tab_inactive: rgb_const(0x1c1c1c),
    accept: rgb_const(0x009400),
    cancel: rgb_const(0xff6b6b),
    success: rgb_const(0x009400),
    warning: rgb_const(0xfab601),
    error: rgb_const(0xbd3039),
    info: rgb_const(0x90cef1),
    overlay_shadow: rgba_const(0x00000066),
};

/// The palette in effect: dark while Preferences › Theming › Dark mode is on,
/// except inside `gpui_kit::with_light_colors` (parts that stay light, as a
/// drawing's canvas does in Onshape).
pub fn p() -> &'static Palette {
    if gpui_kit::hex_theme_active() { &PALETTE_DARK } else { &PALETTE }
}

/// Theming › Dark mode: the dark palette, and literal hex colours mapped
/// through [`DARK_HEX`] (the patched gpui's `rgb()` / `rgba()` hook).
pub fn set_dark(on: bool) {
    gpui_kit::set_hex_theme(on.then_some(&DARK_HEX[..]));
    noshape_scene::palette::set_dark(on);
}

/// Light theme hex → dark theme hex for literal `rgb()` / `rgba()` colours, sorted by the light value:
/// for each light value the dark value most of Onshape's themed tokens give it (live
/// dom/dark-tokens-table.md), with the palette roles deciding ties (backgrounds, text).
pub static DARK_HEX: [(u32, u32); 50] = [
    (0x00007f, 0xdef1fb),
    (0x0000d0, 0x90cef1),
    (0x028502, 0xa2db7d),
    (0x094174, 0xdef1fb),
    (0x134e96, 0xa2b9df),
    (0x1651b0, 0x3e94ff),
    (0x1c1c1c, 0xffffff),
    (0x333333, 0xffffff),
    (0x3d4647, 0xffffff),
    (0x3e94ff, 0x90cef1),
    (0x40aa1d, 0xffffff),
    (0x467bb6, 0x90cef1),
    (0x555555, 0xf5f5f5),
    (0x666666, 0xcccccc),
    (0x672f8f, 0xa64dff),
    (0x6a4f06, 0xdba517),
    (0x777777, 0x999999),
    (0x8000ff, 0xa64dff),
    (0x888888, 0xcccccc),
    (0x90cef1, 0xdef1fb),
    (0x932229, 0xbd3039),
    (0x987107, 0xfcda7f),
    (0x999999, 0xd7d7d7),
    (0x9aa5af, 0xcccccc),
    (0xa2b9df, 0x134e96),
    (0xa2db7d, 0x028502),
    (0xaa252d, 0xdd979b),
    (0xaaaaaa, 0x888888),
    (0xb2ddf6, 0x134e96),
    (0xbd3039, 0xff6b6b),
    (0xcccccc, 0x555555),
    (0xcd0000, 0xdd979b),
    (0xcedbe5, 0x666666),
    (0xd0dbe5, 0x4b5761),
    (0xd7d7d7, 0x666666),
    (0xdd979b, 0x683232),
    (0xddf6cc, 0x0b450b),
    (0xdef1fb, 0x0a4780),
    (0xeaeaea, 0x666666),
    (0xecf0f4, 0x555555),
    (0xecf5fc, 0x092046),
    (0xeeeeee, 0x666666),
    (0xf1f5fa, 0x142434),
    (0xf5f5f5, 0x333333),
    (0xf9ebec, 0x683232),
    (0xfab601, 0xdba517),
    (0xfafafa, 0x2c2c2c),
    (0xfcda7f, 0xfff8e6),
    (0xfff8e6, 0x6a4f06),
    (0xffffff, 0x333333),
];



pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

/// Linear RGBA from sRGB hex (renderer colors).
pub use noshape_scene::palette::lin;

/// sRGB bytes with full alpha (per-primitive renderer colors).
pub use noshape_scene::palette::rgb8;

#[cfg(test)]
mod dark_tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn dark_hex_table_is_sorted_and_keeps_shadows_and_accept() {
        assert!(DARK_HEX.windows(2).all(|w| w[0].0 < w[1].0), "binary search needs a sorted table");
        // Black (shadows) and the ✓ green stay as they are.
        assert!(!DARK_HEX.iter().any(|(from, _)| *from == 0x000000 || *from == 0x009400));
        assert!(DARK_HEX.contains(&(0xffffff, 0x333333)) && DARK_HEX.contains(&(0x333333, 0xffffff)));
    }
}
