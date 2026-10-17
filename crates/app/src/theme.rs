//! noshape's visual identity: a dark graphite workspace with an indigo
//! accent. Deliberately distinct from Onshape's look; the workflow is what
//! matches Onshape, not the appearance.

use gpui_kit::{Hsla, Rgba, rgb, rgba};

#[derive(Clone, Copy)]
pub struct Palette {
    pub app_bg: Rgba,
    pub panel: Rgba,
    pub panel_raised: Rgba,
    pub panel_hover: Rgba,
    pub panel_active: Rgba,
    pub border: Rgba,
    pub border_strong: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub text_faint: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_soft: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub info: Rgba,
    pub selection: Rgba,
    pub sketch_under: Rgba,
    pub sketch_full: Rgba,
    pub sketch_over: Rgba,
    pub sketch_construction: Rgba,
    pub overlay_shadow: Rgba,
}

pub const PALETTE: Palette = Palette {
    app_bg: rgb_const(0x0f1115),
    panel: rgb_const(0x15181d),
    panel_raised: rgb_const(0x1b1f26),
    panel_hover: rgb_const(0x232832),
    panel_active: rgb_const(0x2b3140),
    border: rgb_const(0x262b34),
    border_strong: rgb_const(0x353c48),
    text: rgb_const(0xe7e9ee),
    text_muted: rgb_const(0xa0a8b6),
    text_faint: rgb_const(0x6b7382),
    accent: rgb_const(0x8b7cff),
    accent_hover: rgb_const(0xa498ff),
    accent_soft: rgba_const(0x8b7cff33),
    success: rgb_const(0x3ecf8e),
    warning: rgb_const(0xf5b84b),
    error: rgb_const(0xff5c5c),
    info: rgb_const(0x5aa9ff),
    selection: rgb_const(0xffb020),
    sketch_under: rgb_const(0x5aa9ff),
    sketch_full: rgb_const(0xe7e9ee),
    sketch_over: rgb_const(0xff5c5c),
    sketch_construction: rgb_const(0x9aa3b2),
    overlay_shadow: rgba_const(0x00000080),
};

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

/// sRGB hex to an Hsla (for APIs that take Hsla).
pub fn hsla(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

#[allow(dead_code)]
pub fn rgba_hex(hex: u32) -> Rgba {
    rgba(hex)
}

/// Default part colors (cycled for new parts), linear RGBA for the renderer.
pub const PART_COLORS_SRGB: [u32; 8] = [0xb8bec9, 0x9fb7d6, 0xd6b89f, 0xa7cfae, 0xcfa7c7, 0xd6d29f, 0x9fd3d6, 0xc9a9a9];

pub fn part_color(i: usize) -> [f32; 4] {
    let hex = PART_COLORS_SRGB[i % PART_COLORS_SRGB.len()];
    let c = noshape_render::srgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8);
    [c[0], c[1], c[2], 1.0]
}

/// Linear RGBA from sRGB hex (renderer colors).
pub fn lin(hex: u32) -> [f32; 4] {
    noshape_render::srgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}
