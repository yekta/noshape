//! Value boxes drawn in the viewport's own frame (`overlay::Label`) instead
//! of as GPUI divs, so they follow the camera without a window redraw: the
//! section offset box, analysis readouts. Each is the GPUI element it
//! replaces, painted the way GPUI paints it:
//!
//! `div().absolute().left(x + dx).top(y + dy).px(pad).border_1()
//!  .border_color(border).bg(bg).text_size(size).text_color(color).child(text)`
//!
//! GPUI lays that out with the box's corner rounded to device pixels (half
//! toward zero), the border and padding inside it, and the text leaf's
//! measured width rounded up to device pixels; its height is the line
//! height (the inherited `phi()` × font size, the root sets only the font
//! family). The quad is the background with the border painted over its
//! edges, the text is `paint_line` from the leaf's corner: rasterized here
//! once with GPUI's own text system, at the same offsets, so only the
//! corner moves with the camera.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use noshape_render::overlay::{Label, Mask, Quad, Rgba as ORgba};
use std::sync::Arc;

/// `NOSHAPE_GPUI_LABELS`: draw the value boxes as GPUI elements (the old
/// way, for comparing pixels).
pub fn gpui_labels() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NOSHAPE_GPUI_LABELS").is_some())
}

/// `NOSHAPE_FRAME_SKETCH_LABELS`: draw the sketch's dimension labels at
/// rest in the 3D frame (`row_box`) instead of as GPUI elements; off until
/// the pixel crops against GPUI are approved.
pub fn frame_sketch_labels() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !gpui_labels() && std::env::var_os("NOSHAPE_FRAME_SKETCH_LABELS").is_some())
}

/// What a value box shows (logical px and GPUI colors).
pub struct ValueBox<'a> {
    pub text: &'a str,
    pub text_size: f32,
    pub text_color: Hsla,
    pub background: Hsla,
    /// 1 px border.
    pub border: Option<Hsla>,
    pub pad_x: f32,
    /// From the anchor's projection to the box's corner.
    pub offset: [f32; 2],
}

fn rgba(c: Hsla) -> ORgba {
    let c = Rgba::from(c);
    [c.r, c.g, c.b, c.a]
}

/// `b` anchored at `anchor`, for `window`'s scale factor; `None` if its
/// text can't be rasterized (emoji, LCD text).
pub fn value_box(window: &Window, cx: &App, b: &ValueBox, anchor: glam::DVec3) -> Option<Label> {
    let scale = window.scale_factor();
    let font = Font { family: cx.theme().font_family.clone(), ..Default::default() };
    let size = px(b.text_size);
    // The inherited line height: GPUI's default, phi() × the font size.
    let line_height = px(b.text_size * 1.618_034);
    let run = TextRun { len: b.text.len(), font: font.clone(), color: b.text_color, background_color: None, underline: None, strikethrough: None };
    let line = window.text_system().shape_line(SharedString::from(b.text.to_string()), size, &[run], None);
    let border = if b.border.is_some() { 1.0 } else { 0.0 };
    // Device px: the border, the text leaf's corner, the box's size.
    let edge = border * scale;
    let text_x = ((border + b.pad_x) * scale).round();
    let text_w = (f32::from(line.width) * scale).ceil();
    let text_h = (f32::from(line_height) * scale).round();
    // Border and padding on both sides of the leaf.
    let (w, h) = (2.0 * text_x + text_w, edge * 2.0 + text_h);
    let mut rects = vec![([0.0, 0.0, w, h], rgba(b.background))];
    if let Some(c) = b.border {
        let c = rgba(c);
        rects.push(([0.0, 0.0, w, edge], c));
        rects.push(([0.0, h - edge, w, h], c));
        rects.push(([0.0, edge, edge, h - edge], c));
        rects.push(([w - edge, edge, w, h - edge], c));
    }
    // The text, as `paint_line` paints it from the leaf's corner (logical
    // px from the box's corner, which sits on a whole device pixel).
    let origin = point(px(text_x / scale), px(edge / scale));
    let m = window.rasterize_text_line(SharedString::from(b.text.to_string()), font, size, line_height, b.text_color, origin)?;
    let mask = Mask { width: m.size.width.0 as u32, height: m.size.height.0 as u32, alpha: Arc::new(m.alpha), offset: [0, 0], density: 1 };
    Some(Label { anchor, offset: b.offset, rects, masks: vec![(mask, [m.origin.x.0, m.origin.y.0], rgba(b.text_color))], ..Default::default() })
}

/// A text leaf of a [`RowBox`]: `div().child(text)` (italic or not, its own
/// color) with a right margin (logical px).
pub struct Leaf<'a> {
    pub text: &'a str,
    pub italic: bool,
    pub color: Hsla,
    pub margin_right: f32,
}

/// A fixed-height row of text leaves, as the sketch's dimension labels are
/// made: `div().h(height).px(pad_x).flex().items_center().rounded(radius)
/// [.bg(background)][.border_1().border_color(border)].text_size(size)
/// .whitespace_nowrap().children(leaves)`, its corner at the anchor's
/// projection plus `offset`.
pub struct RowBox<'a> {
    pub leaves: Vec<Leaf<'a>>,
    pub text_size: f32,
    pub height: f32,
    pub pad_x: f32,
    pub radius: f32,
    pub background: Option<Hsla>,
    pub border: Option<Hsla>,
    pub offset: [f32; 2],
}

/// GPUI (taffy) rounds absolute positions half toward zero; from a corner
/// on a whole positive device pixel that is "half down" for an offset.
fn round_half_down(v: f32) -> f32 {
    (v - 0.5).ceil()
}

/// `b` anchored at `anchor` as a hit-tested label (`hit`); `None` if a
/// leaf's text can't be rasterized (emoji, LCD text).
pub fn row_box(window: &Window, cx: &App, b: &RowBox, anchor: glam::DVec3, hit: u64) -> Option<Label> {
    let scale = window.scale_factor();
    let size = px(b.text_size);
    let line_height = px(b.text_size * 1.618_034);
    let edge = if b.border.is_some() { scale } else { 0.0 };
    let h = (b.height * scale).round();
    let inner_h = h - 2.0 * edge;
    let mut x = edge + (b.pad_x * scale).round();
    let mut masks = Vec::new();
    for leaf in &b.leaves {
        let font = Font { family: cx.theme().font_family.clone(), style: if leaf.italic { FontStyle::Italic } else { FontStyle::Normal }, ..Default::default() };
        let run = TextRun { len: leaf.text.len(), font: font.clone(), color: leaf.color, background_color: None, underline: None, strikethrough: None };
        let line = window.text_system().shape_line(SharedString::from(leaf.text.to_string()), size, &[run], None);
        let w = (f32::from(line.width) * scale).ceil();
        // Centered in the row (items_center), from the rounded corner.
        let top = edge + round_half_down((inner_h - f32::from(line_height) * scale) * 0.5);
        let origin = point(px(x / scale), px(top / scale));
        if !leaf.text.is_empty() {
            let m = window.rasterize_text_line(SharedString::from(leaf.text.to_string()), font, size, line_height, leaf.color, origin)?;
            let mask = Mask { width: m.size.width.0 as u32, height: m.size.height.0 as u32, alpha: Arc::new(m.alpha), offset: [0, 0], density: 1 };
            masks.push((mask, [m.origin.x.0, m.origin.y.0], rgba(leaf.color)));
        }
        x += w + (leaf.margin_right * scale).round();
    }
    let w = x + (b.pad_x * scale).round() + edge;
    let quad = Quad {
        bounds: [0.0, 0.0, w, h],
        corner_radius: b.radius * scale,
        background: b.background.map(rgba).unwrap_or([0.0; 4]),
        border_width: edge,
        border_color: b.border.map(rgba).unwrap_or([0.0; 4]),
    };
    Some(Label { anchor, offset: b.offset, quads: vec![quad], masks, hit: Some(hit), ..Default::default() })
}
