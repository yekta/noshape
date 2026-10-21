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
use noshape_render::overlay::{Label, Mask, Rgba as ORgba};
use std::sync::Arc;

/// `NOSHAPE_GPUI_LABELS`: draw the value boxes as GPUI elements (the old
/// way, for comparing pixels).
pub fn gpui_labels() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NOSHAPE_GPUI_LABELS").is_some())
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
    let mask = Mask { width: m.size.width.0 as u32, height: m.size.height.0 as u32, alpha: Arc::new(m.alpha), offset: [0, 0] };
    Some(Label { anchor, offset: b.offset, rects, masks: vec![(mask, [m.origin.x.0, m.origin.y.0], rgba(b.text_color))] })
}
