//! Onshape's selection-count cursor: with N entities selected, the pointer
//! over the graphics area is an arrow with the digit N (1-5, "5" for five or
//! more) drawn into the cursor image (live: the canvas CSS cursor
//! `macSelectionCursor-N.svg`, hot spot 3,3 in a 32 px image).
//!
//! GPUI only offers named cursor styles, so the image cursor is set through
//! the platform: X11 (an ARGB cursor from the RENDER extension on the
//! window), macOS (`NSCursor` with an image), Windows (an icon cursor and a
//! `WM_SETCURSOR` subclass). GPUI re-sets its own cursor only when the hovered
//! element's style changes; the viewport re-applies ours after that (see
//! `Viewport::sync_cursor`).
//!
//! The images are our own drawing: a black arrow with a white outline and
//! the digit in bold Liberation Sans, white-outlined, at the arrow's lower
//! right.

use gpui_kit::{App, DevicePixels, SvgSize, Window, size};
use raw_window_handle::HasWindowHandle;

/// Cursor image side in logical pixels and its hot spot (live).
const SIZE: f32 = 32.;
const HOT: f32 = 3.;

/// The cursor for `count` selected entities as an SVG on a 32-unit canvas.
fn cursor_svg(count: usize) -> String {
    let digit = count.clamp(1, 5).to_string();
    // The digit, outlined from the font: its ink spans x 15..21 and
    // y 15..24.5 (measured on the live cursor at 8x).
    let layout = noshape_sketch::text::layout_text(&digit, noshape_sketch::text::DEFAULT_FONT, true, false);
    let polys = layout.polylines(6);
    let (mut lo, mut hi) = (glam::DVec2::splat(f64::MAX), glam::DVec2::splat(f64::MIN));
    for p in polys.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let k = 9.5 / (hi.y - lo.y).max(1e-6);
    let x0 = 18.0 - (hi.x - lo.x) * k * 0.5;
    let mut d = String::new();
    for poly in &polys {
        for (i, p) in poly.iter().enumerate() {
            let (x, y) = (x0 + (p.x - lo.x) * k, 24.5 - (p.y - lo.y) * k);
            d.push_str(&format!("{}{x:.3} {y:.3} ", if i == 0 { "M" } else { "L" }));
        }
        d.push_str("Z ");
    }
    // A slim classic arrow (tip at 3,3.4) and the digit, each black with a
    // white outline and a soft drop shadow, like the live cursor.
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
<defs><filter id="s" x="-20%" y="-20%" width="160%" height="160%"><feDropShadow dx="0" dy="1" stdDeviation="0.7" flood-color="#000" flood-opacity="0.35"/></filter></defs>
<g filter="url(#s)">
<path d="M3 3.4 L3 15.4 L6 12.4 L8.5 17.4 C8.8 17.6 9.3 17.6 9.6 17.3 C9.9 17.1 10.1 16.7 10 16.4 L7.5 11.4 L11 11.4 Z" fill="#000" stroke="#fff" stroke-width="2" stroke-linejoin="round" paint-order="stroke"/>
</g>
<g filter="url(#s)">
<path d="{d}" fill="#000" fill-rule="nonzero" stroke="#fff" stroke-width="2.4" stroke-linejoin="round" paint-order="stroke"/>
</g>
</svg>"##
    )
}

/// The cursor image: straight-alpha BGRA rows, its side and hot spot in
/// device pixels.
pub(crate) struct CursorImage {
    pub bgra: Vec<u8>,
    pub side: u32,
    pub hot: u32,
}

fn render(count: usize, scale: f32, cx: &App) -> Option<CursorImage> {
    let side = (SIZE * scale).round().max(1.0) as u32;
    let r = cx.svg_renderer();
    let parsed = r.parse_svg(cursor_svg(count).as_bytes()).ok()?;
    let image = r.render_parsed(&parsed, SvgSize::ExactSize(size(DevicePixels(side as i32), DevicePixels(side as i32)))).ok()?;
    let bgra = image.as_bytes(0)?.to_vec();
    (bgra.len() == (side * side * 4) as usize).then_some(CursorImage { bgra, side, hot: (HOT * scale).round() as u32 })
}

/// Show the selection-count cursor for `count` (1..) over `window`, or the
/// normal arrow for `None`. Returns false where the platform can't (then the
/// viewport draws the digit itself).
pub fn set(window: &Window, count: Option<usize>, cx: &App) -> bool {
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return false };
    let scale = window.scale_factor();
    let count = count.map(|n| n.clamp(1, 5));
    platform::set(handle.as_raw(), (scale * 100.0).round() as u32, count, &mut |n| render(n, scale, cx))
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[path = "cursor_x11.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "cursor_macos.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "cursor_windows.rs"]
mod platform;

#[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn set(_: raw_window_handle::RawWindowHandle, _: u32, _: Option<usize>, _: &mut dyn FnMut(usize) -> Option<super::CursorImage>) -> bool {
        false
    }
}
