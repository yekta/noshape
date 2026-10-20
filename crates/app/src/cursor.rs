//! Onshape's graphics-area cursors (docs/parity/cursors.md). GPUI only offers
//! named cursor styles, so the viewport's own cursors are set through the
//! platform: X11 (an ARGB cursor from the RENDER extension, or a theme
//! cursor by its CSS name, on the window), Wayland (the patched GPUI pointer,
//! vendor/gpui-pre-linux/src/custom_cursor.rs), macOS (`NSCursor`), Windows
//! (`HCURSOR` and a `WM_SETCURSOR` subclass). GPUI re-sets its own cursor
//! only when the hovered element's style changes; the viewport re-applies
//! ours after that (see `Viewport::sync_cursor`).
//!
//! Two kinds:
//! - **Images** Onshape draws itself (selection count, sketch crosshair,
//!   "click to accept" confirmation, box deselect, snap mate). The art is our
//!   own drawing of the same idea, at Onshape's sizes and hot spots; Windows
//!   gets the white-arrow variants Onshape picks there (`isWindows()`), every
//!   other OS the black-arrow ones.
//! - **CSS standard cursors** GPUI has no style for (move, zoom-in,
//!   no-drop): the platform's own cursor (X11 / Wayland theme name, Windows
//!   `IDC_*`); where the OS has none (macOS move / zoom-in, Windows zoom-in)
//!   Chrome draws its own, and so do we.

use gpui_kit::{App, Context, DevicePixels, SvgSize, Window, size};
use std::time::Duration;
use raw_window_handle::HasWindowHandle;

/// A cursor of the graphics area that GPUI can't show by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppCursor {
    /// N entities selected: 1..=5, 6 for "5+".
    Count(u8),
    /// Sketch drawing tools (`crosshair-cursor.svg`, 15 px, hot 7.5) and
    /// drawing tools (`DrawingCrosshair-01.cur`, the same plus, hot 7,7).
    Crosshair,
    /// The drawing Centermark tool (`DrawingCrosshairBox3-01.cur`, hot 7,7).
    CrosshairBox,
    /// Sketch fillet / chamfer / pattern ready to accept with a click.
    Confirmation,
    /// Box selection with Ctrl held (removes from the selection).
    RemoveSelection,
    /// Assembly drag snapping to a mate connector.
    SnapMate,
    /// CSS `move` (dimension values, plane manipulators).
    Move,
    /// CSS `zoom-in` (Zoom to window).
    ZoomIn,
    /// CSS `no-drop` (dragging a fixed assembly instance).
    NoDrop,
}

impl AppCursor {
    /// The selection-count cursor for `n` selected entities.
    pub fn count(n: usize) -> Self {
        Self::Count(n.clamp(1, 6) as u8)
    }

    /// The CSS name of a standard cursor.
    pub(crate) fn css(self) -> Option<&'static str> {
        Some(match self {
            Self::Move => "move",
            Self::ZoomIn => "zoom-in",
            Self::NoDrop => "no-drop",
            _ => return None,
        })
    }
}

/// The black arrow (mac / Linux art), tip at 3,3.4, white outline.
const MAC_ARROW: &str = r##"<path d="M3 3.4 L3 15.4 L6 12.4 L8.5 17.4 C8.8 17.6 9.3 17.6 9.6 17.3 C9.9 17.1 10.1 16.7 10 16.4 L7.5 11.4 L11 11.4 Z" fill="#000" stroke="#fff" stroke-width="2" stroke-linejoin="round" paint-order="stroke"/>"##;
/// The white arrow (Windows art), tip at 1,2, dark outline.
const WIN_ARROW: &str = r##"<path d="M1.5 2.2 L1.5 17.5 L5.2 13.9 L8 20 L10.6 18.9 L7.9 12.9 L13 12.9 Z" fill="#fff" stroke="#2b2b2b" stroke-width="1.1" stroke-linejoin="round"/>"##;

/// Outlined glyphs of `text` (bold Liberation Sans) as an SVG path whose ink
/// is `height` tall, horizontally centered on `cx`, baseline at `base`.
fn text_path(text: &str, cx: f64, base: f64, height: f64) -> String {
    let layout = noshape_sketch::text::layout_text(text, noshape_sketch::text::DEFAULT_FONT, true, false);
    let polys = layout.polylines(6);
    let (mut lo, mut hi) = (glam::DVec2::splat(f64::MAX), glam::DVec2::splat(f64::MIN));
    for p in polys.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let k = height / (hi.y - lo.y).max(1e-6);
    let x0 = cx - (hi.x - lo.x) * k * 0.5;
    let mut d = String::new();
    for poly in &polys {
        for (i, p) in poly.iter().enumerate() {
            let (x, y) = (x0 + (p.x - lo.x) * k, base - (p.y - lo.y) * k);
            d.push_str(&format!("{}{x:.3} {y:.3} ", if i == 0 { "M" } else { "L" }));
        }
        d.push_str("Z ");
    }
    d
}

/// An image cursor: its SVG on a `side`-unit canvas, the hot spot (in the
/// same units) and the side in CSS pixels.
struct Art {
    svg: String,
    side: f32,
    hot: (f32, f32),
}

fn wrap(side: f32, body: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}" viewBox="0 0 {side} {side}">
<defs><filter id="s" x="-20%" y="-20%" width="160%" height="160%"><feDropShadow dx="0" dy="1" stdDeviation="0.7" flood-color="#000" flood-opacity="0.35"/></filter></defs>
<g filter="url(#s)">{body}</g>
</svg>"##
    )
}

/// The art of an image cursor (`None` for the CSS standard ones).
fn art(cursor: AppCursor, windows: bool) -> Option<Art> {
    let arrow = if windows { WIN_ARROW } else { MAC_ARROW };
    let hot = if windows { (1., 2.) } else { (3., 3.) };
    // A label in black with a white outline at the arrow's lower right.
    let label = |text: &str, cx: f64, height: f64| {
        let d = text_path(text, cx, 24.5, height);
        format!(r##"<path d="{d}" fill="#000" stroke="#fff" stroke-width="2.4" stroke-linejoin="round" paint-order="stroke"/>"##)
    };
    Some(match cursor {
        AppCursor::Count(n) => {
            let body = if n >= 6 { label("5+", 21.0, 9.5) } else { label(&n.to_string(), 18.0, 9.5) };
            Art { svg: wrap(32., &format!("{arrow}{body}")), side: 32., hot }
        }
        // A 15 px plus: 1 px black lines on 3 px white, white 1 px caps at
        // the ends (hot 7.5,7.5). The drawing tools' crosshair is the same.
        AppCursor::Crosshair => Art {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 15 15"><path d="M7.5 0V15M0 7.5H15" stroke="#fff" stroke-width="3"/><path d="M7.5 1V14M1 7.5H14" stroke="#000" stroke-width="1"/></svg>"##.into(),
            side: 15.,
            hot: (7.5, 7.5),
        },
        // Drawing Centermark: the plus with a 7 px square at its center (a
        // hollow 3 px middle), white halo, arms to the edges (hot 7,7).
        AppCursor::CrosshairBox => Art {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 15 15"><path d="M6 0H9V4H6ZM6 11H9V15H6ZM0 6H4V9H0ZM11 6H15V9H11ZM3 3H12V12H3ZM6 6V9H9V6Z" fill="#fff" fill-rule="evenodd"/><path d="M7.5 0V4M7.5 11V15M0 7.5H4M11 7.5H15" stroke="#000" stroke-width="1"/><rect x="4.5" y="4.5" width="6" height="6" fill="none" stroke="#000" stroke-width="1"/></svg>"##.into(),
            side: 15.,
            hot: (7., 7.),
        },
        // Arrow and a mouse whose left button is green with a white check
        // ("click to accept"), hot 1,1.
        AppCursor::Confirmation => {
            let mouse = r##"<rect x="13.5" y="7.5" width="15" height="24" rx="7.5" fill="#fff" stroke="#000" stroke-width="1.3"/><path d="M21 7.5 A7.5 7.5 0 0 0 13.5 15 V17.5 H21 Z" fill="#0a8a2a" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M15.3 12.4 L17.4 14.6 L20.2 10.6" stroke="#fff" stroke-width="1.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/><rect x="20.2" y="11.5" width="1.6" height="5.5" rx="0.8" fill="#fff" stroke="#000" stroke-width="0.8"/><path d="M13.5 17.5 H28.5" stroke="#000" stroke-width="1"/>"##;
            let arrow = if windows { WIN_ARROW } else { MAC_ARROW };
            Art { svg: wrap(32., &format!(r##"<g transform="translate({} {})">{arrow}</g>{mouse}"##, if windows { 0.0 } else { -2.0 }, if windows { -1.0 } else { -2.4 })), side: 32., hot: (1., 1.) }
        }
        // Arrow and a minus sign.
        AppCursor::RemoveSelection => {
            let minus = r##"<rect x="14.5" y="19" width="9" height="2.6" fill="#000" stroke="#fff" stroke-width="2" paint-order="stroke"/>"##;
            Art { svg: wrap(32., &format!("{arrow}{minus}")), side: 32., hot }
        }
        // Arrow and a mate connector: two disc caps joined by a stem, hot 7.5.
        AppCursor::SnapMate => {
            let cap = |y: f64, flip: bool| {
                let (top, rim) = if flip { (y + 6.0, y) } else { (y, y + 6.0) };
                format!(r##"<ellipse cx="22" cy="{top}" rx="6.5" ry="2.6" fill="#fff" stroke="#000" stroke-width="1.3"/><path d="M15.5 {top} V{rim} M28.5 {top} V{rim}" stroke="#000" stroke-width="1.3"/><path d="M15.5 {rim} A6.5 2.6 0 0 0 28.5 {rim}" fill="#fff" stroke="#000" stroke-width="1.3"/>"##)
            };
            let body = format!(r##"<path d="M22 12 V22" stroke="#000" stroke-width="1.6"/>{}{}"##, cap(4.0, false), cap(22.0, true));
            Art { svg: wrap(32., &format!(r##"<g transform="translate(4.5 4.1)">{arrow}</g>{body}"##)), side: 32., hot: (7.5, 7.5) }
        }
        _ => return None,
    })
}

/// Our own images for CSS standard cursors where the OS has none (Chrome
/// draws its own there too): 4-way move arrows, a magnifier with "+".
fn standard_art(cursor: AppCursor) -> Option<Art> {
    let svg = match cursor {
        AppCursor::Move => r##"<path d="M12 1.5 L15.5 5.5 H13.3 V10.7 H18.5 V8.5 L22.5 12 L18.5 15.5 V13.3 H13.3 V18.5 H15.5 L12 22.5 L8.5 18.5 H10.7 V13.3 H5.5 V15.5 L1.5 12 L5.5 8.5 V10.7 H10.7 V5.5 H8.5 Z" fill="#000" stroke="#fff" stroke-width="1.4" stroke-linejoin="round" paint-order="stroke"/>"##.to_string(),
        AppCursor::ZoomIn => r##"<circle cx="9.5" cy="9.5" r="6.5" fill="#fff" stroke="#000" stroke-width="2"/><path d="M14.3 14.3 L21 21" stroke="#000" stroke-width="3.4" stroke-linecap="round"/><path d="M6.5 9.5 H12.5 M9.5 6.5 V12.5" stroke="#000" stroke-width="1.8"/>"##.to_string(),
        _ => return None,
    };
    let hot = if cursor == AppCursor::ZoomIn { (9.5, 9.5) } else { (12., 12.) };
    Some(Art { svg: wrap(24., &svg), side: 24., hot })
}

/// The cursor image: straight-alpha BGRA rows, its side and hot spot in
/// device pixels.
pub(crate) struct CursorImage {
    pub bgra: Vec<u8>,
    pub side: u32,
    pub hot: (u32, u32),
}

fn render(art: Art, scale: f32, cx: &App) -> Option<CursorImage> {
    let side = (art.side * scale).round().max(1.0) as u32;
    let r = cx.svg_renderer();
    let parsed = r.parse_svg(art.svg.as_bytes()).ok()?;
    let image = r.render_parsed(&parsed, SvgSize::ExactSize(size(DevicePixels(side as i32), DevicePixels(side as i32)))).ok()?;
    let bgra = image.as_bytes(0)?.to_vec();
    // Chrome truncates CSS hot spots to whole pixels (7.5 -> 7), then scales.
    let hot = ((art.hot.0.trunc() * scale).floor() as u32, (art.hot.1.trunc() * scale).floor() as u32);
    (bgra.len() == (side * side * 4) as usize).then_some(CursorImage { bgra, side, hot })
}

/// The image of `cursor` (custom art, or our drawing of a standard one).
pub(crate) fn image(cursor: AppCursor, windows: bool, scale: f32, cx: &App) -> Option<CursorImage> {
    render(art(cursor, windows).or_else(|| standard_art(cursor))?, scale, cx)
}

/// Show `cursor` over `window` (`None`: GPUI's own cursor). Returns false
/// where the platform can't.
pub fn set(window: &Window, cursor: Option<AppCursor>, cx: &App) -> bool {
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return false };
    let scale = window.scale_factor();
    platform::set(handle.as_raw(), (scale * 100.0).round() as u32, cursor, &mut |c, windows| image(c, windows, scale, cx))
}

/// Keeps an app cursor on the window for one view other than the viewport
/// (the drawing sheet): call [`CursorSync::sync`] from its render.
#[derive(Default)]
pub struct CursorSync {
    /// The cursor last set (`Some(None)`: GPUI's own).
    applied: Option<Option<AppCursor>>,
}

impl CursorSync {
    pub fn sync<V: 'static>(view: &mut V, want: Option<AppCursor>, window: &mut Window, cx: &mut Context<V>, field: fn(&mut V) -> &mut CursorSync) {
        let applied = field(view).applied;
        if applied == Some(want) || (want.is_none() && applied.is_none_or(|a| a.is_none())) {
            return;
        }
        let ok = set(window, want, cx);
        field(view).applied = Some(want);
        if want.is_none() {
            // Let GPUI put back the hovered element's cursor.
            window.refresh();
        } else if ok {
            // GPUI sets its own cursor at the end of a frame in which the
            // hovered style changed; set ours again once that frame is out.
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if field(this).applied == Some(want) {
                        set(window, want, cx);
                    }
                });
            })
            .detach();
        }
    }
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
    pub fn set(_: raw_window_handle::RawWindowHandle, _: u32, _: Option<super::AppCursor>, _: &mut dyn FnMut(super::AppCursor, bool) -> Option<super::CursorImage>) -> bool {
        false
    }
}
