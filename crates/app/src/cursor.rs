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
const MAC_ARROW: &str = r##"<path d="M3 3.4 L3 15.4 L6 12.4 L8.5 17.4 C8.8 17.6 9.3 17.6 9.6 17.3 C9.9 17.1 10.1 16.7 10 16.4 L7.5 11.4 L11 11.4 Z" fill="#000" stroke="#fff" stroke-width="2" stroke-linejoin="miter" paint-order="stroke"/>"##;
/// The white arrow (Windows art), tip at 1.5,1.8, thin navy outline.
const WIN_ARROW: &str = r##"<path d="M1.5 1.8 L1.5 17.3 L5.6 14.2 L7.9 19.1 L10.1 18.2 L8.3 13.3 L12.6 13.25 Z" fill="#fff" stroke="#191d2f" stroke-width="1" stroke-linejoin="miter"/>"##;

/// Outlined glyphs of `text` (bold Liberation Sans) as an SVG path whose ink
/// is `height` tall, its left edge at `x0`, baseline at `base`.
fn text_path(text: &str, x0: f64, base: f64, height: f64) -> String {
    let layout = noshape_sketch::text::layout_text(text, noshape_sketch::text::DEFAULT_FONT, true, false);
    let polys = layout.polylines(6);
    let (mut lo, mut hi) = (glam::DVec2::splat(f64::MAX), glam::DVec2::splat(f64::MIN));
    for p in polys.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let k = height / (hi.y - lo.y).max(1e-6);
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

/// The SVG of an image cursor: `arrow` and `body` with Onshape's drop
/// shadow. The Windows arrow casts its own softer one.
fn wrap(side: f32, arrow: &str, body: &str, windows: bool) -> String {
    let (soft, sharp) = if windows { (arrow, "") } else { ("", arrow) };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}" viewBox="0 0 {side} {side}">
<defs><filter id="s" x="-20%" y="-20%" width="160%" height="160%"><feDropShadow dx="0" dy="1" stdDeviation="1" flood-color="#000" flood-opacity="0.8"/></filter><filter id="w" x="-20%" y="-20%" width="160%" height="160%"><feDropShadow dx="1" dy="1" stdDeviation="1.5" flood-color="#000" flood-opacity="0.4"/></filter></defs>
<g filter="url(#w)">{soft}</g><g filter="url(#s)">{sharp}{body}</g>
</svg>"##
    )
}

/// The art of an image cursor (`None` for the CSS standard ones).
fn art(cursor: AppCursor, windows: bool) -> Option<Art> {
    let arrow = if windows { WIN_ARROW } else { MAC_ARROW };
    let hot = if windows { (1., 2.) } else { (3., 3.) };
    // Onshape's Windows files are 38 px; their art beside the arrow sits 1 px
    // right and down of the mac one (the count label 3 right, 5 up).
    let side = |mac: f32| -> (f32, (i32, i32)) { if windows { (38., (1, 1)) } else { (mac, (0, 0)) } };
    let place = |side: f32, shift: (i32, i32), body: String| wrap(side, arrow, &format!(r##"<g transform="translate({} {})">{body}</g>"##, shift.0, shift.1), windows);
    // A label in black with a white outline at the arrow's lower right.
    let label = |text: &str| {
        let d = text_path(text, 15.0, 24.5, 9.5);
        format!(r##"<path d="{d}" fill="#000" stroke="#fff" stroke-width="2.4" stroke-linejoin="miter" paint-order="stroke"/>"##)
    };
    Some(match cursor {
        AppCursor::Count(n) => {
            let body = if n >= 6 { format!(r##"{}<path d="M22.4 19.75h6.8v1h-6.8zM25.3 16.85h1v6.8h-1z" fill="#000" stroke="#fff" stroke-width="2.4" paint-order="stroke"/>"##, label("5")) } else { label(&n.to_string()) };
            let (side, _) = side(32.);
            Art { svg: place(side, if windows { (3, -5) } else { (0, 0) }, body), side, hot }
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
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 15 15"><path d="M6 0H9V4H6ZM6 11H9V15H6ZM0 6H4V9H0ZM11 6H15V9H11ZM3 3H12V12H3ZM6 6V9H9V6Z" fill="#fff" fill-rule="evenodd"/><path d="M7 0H8V4H7ZM7 11H8V15H7ZM0 7H4V8H0ZM11 7H15V8H11ZM4 4H11V11H4ZM5 5V10H10V5Z" fill="#000" fill-rule="evenodd"/></svg>"##.into(),
            side: 15.,
            hot: (7., 7.),
        },
        // The arrow beside a 19x31 mouse whose left button is green with a
        // white check ("click to accept"), hot 1,1.
        AppCursor::Confirmation => {
            let mouse = r##"<path d="M14.6 11 C14.6 6 18.8 2.5 23.8 2.5 C28.8 2.5 33 6 33 11 L32.6 28.5 C32.5 31.5 30.5 33.25 27.5 33.25 H20 C17 33.25 15.1 31.5 15 28.5 Z" fill="#fff" stroke="#000" stroke-width="1"/><path d="M24 2 V15.4 H14.1 V11 C14.1 5.7 18.5 2 24 2 Z" fill="#009400"/><path d="M15.6 8.4 L18.7 11.5 L22.7 6.6" fill="none" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/><path d="M25 15.5 H32.8" stroke="#3c3c3c" stroke-width="0.75"/><rect x="23" y="11.9" width="1.9" height="7.2" rx="0.95" fill="#fff" stroke="#000" stroke-width="0.8"/>"##;
            let (side, shift) = side(37.);
            Art { svg: place(side, shift, mouse.into()), side, hot: (1., 1.) }
        }
        // The arrow and a minus sign.
        AppCursor::RemoveSelection => {
            let minus = r##"<rect x="16" y="18" width="8" height="1.5" fill="#000" stroke="#fff" stroke-width="2" paint-order="stroke"/>"##;
            let (side, shift) = side(32.);
            Art { svg: place(side, shift, minus.into()), side, hot }
        }
        // The arrow and a mate connector: a disc on a neck, a stem, and a cup
        // with a socket and a pin (hot 7.5,7.5).
        AppCursor::SnapMate => {
            let body = r##"<g fill="#fff" stroke="#000" stroke-width="1.1"><path d="M16.5 5 V9.3 A7 2 0 0 0 30.5 9.3 V5"/><ellipse cx="23.5" cy="5" rx="7" ry="3.5"/><path d="M20.5 10.6 V13 Q20.5 14.5 22 14.5 H24.5 Q26 14.5 26 13 V10.6"/><path d="M23.5 14.5 V18.5" stroke-width="1"/><path d="M16.5 20.8 V27.5 A7 4 0 0 0 30.5 27.5 V20.8 A7 2.3 0 0 0 16.5 20.8 Z"/><path d="M16.9 21.7 A7 2.3 0 0 0 20.5 22.9 V27.2 H26 V22.9 A7 2.3 0 0 0 30.1 21.7" fill="none"/><rect x="23" y="21" width="1" height="3" fill="#000" stroke="none"/></g>"##;
            let (side, shift) = side(35.);
            Art { svg: place(side, shift, body.into()), side, hot: (7.5, 7.5) }
        }
        _ => return None,
    })
}

/// Our own images for CSS standard cursors where the OS has none (Chrome
/// draws its own there too): 4-way move arrows, a magnifier with "+".
fn standard_art(cursor: AppCursor) -> Option<Art> {
    let svg = match cursor {
        // The same art as GPUI's `CursorStyle::Move` on macOS
        // (vendor/gpui-pre-macos/src/cursors/src/move.svg).
        AppCursor::Move => r##"<path d="M11.05 6 L8.3 6 L12 1.8 L15.7 6 L12.95 6 L12.95 11.05 L18 11.05 L18 8.3 L22.2 12 L18 15.7 L18 12.95 L12.95 12.95 L12.95 18 L15.7 18 L12 22.2 L8.3 18 L11.05 18 L11.05 12.95 L6 12.95 L6 15.7 L1.8 12 L6 8.3 L6 11.05 L11.05 11.05 Z" fill="#fff" stroke="#fff" stroke-width="2" stroke-linejoin="round"/><path d="M11.05 6 L8.3 6 L12 1.8 L15.7 6 L12.95 6 L12.95 11.05 L18 11.05 L18 8.3 L22.2 12 L18 15.7 L18 12.95 L12.95 12.95 L12.95 18 L15.7 18 L12 22.2 L8.3 18 L11.05 18 L11.05 12.95 L6 12.95 L6 15.7 L1.8 12 L6 8.3 L6 11.05 L11.05 11.05 Z" fill="#000"/>"##.to_string(),
        AppCursor::ZoomIn => r##"<circle cx="9.5" cy="9.5" r="6.5" fill="#fff" stroke="#000" stroke-width="2"/><path d="M14.3 14.3 L21 21" stroke="#000" stroke-width="3.4" stroke-linecap="round"/><path d="M6.5 9.5 H12.5 M9.5 6.5 V12.5" stroke="#000" stroke-width="1.8"/>"##.to_string(),
        _ => return None,
    };
    let hot = if cursor == AppCursor::ZoomIn { (9.5, 9.5) } else { (12., 12.) };
    Some(Art { svg: wrap(24., "", &svg, false), side: 24., hot })
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

/// A file-name stem for `cursor` ("count-3", "snap-mate").
fn stem(cursor: AppCursor) -> String {
    match cursor {
        AppCursor::Count(n) => format!("count-{n}"),
        AppCursor::Crosshair => "crosshair".into(),
        AppCursor::CrosshairBox => "crosshair-box".into(),
        AppCursor::Confirmation => "confirmation".into(),
        AppCursor::RemoveSelection => "remove-selection".into(),
        AppCursor::SnapMate => "snap-mate".into(),
        AppCursor::Move => "move".into(),
        AppCursor::ZoomIn => "zoom-in".into(),
        AppCursor::NoDrop => "no-drop".into(),
    }
}

/// `img` as a Windows .cur file: one 32-bpp image (straight alpha, bottom-up
/// rows, an empty AND mask) with its hot spot.
fn cur_file(img: &CursorImage) -> Vec<u8> {
    let side = img.side;
    let mask_row = side.div_ceil(32) * 4;
    let bmp_len = 40 + side * side * 4 + mask_row * side;
    let mut out = Vec::with_capacity(22 + bmp_len as usize);
    out.extend_from_slice(&[0, 0, 2, 0, 1, 0]);
    // 256 is written as 0.
    out.extend_from_slice(&[(side % 256) as u8, (side % 256) as u8, 0, 0]);
    out.extend_from_slice(&(img.hot.0 as u16).to_le_bytes());
    out.extend_from_slice(&(img.hot.1 as u16).to_le_bytes());
    out.extend_from_slice(&bmp_len.to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes());
    for v in [40u32, side, side * 2] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for row in (0..side as usize).rev() {
        let w = side as usize * 4;
        out.extend_from_slice(&img.bgra[row * w..(row + 1) * w]);
    }
    out.resize(out.len() + (mask_row * side) as usize, 0);
    out
}

/// Writes every image cursor of `cursors` into `dir` as Windows .cur files,
/// `<name>-<win|mac>-<1|2>x.cur`, so a real Windows machine can load and draw
/// them (perf's smoke job, `NOSHAPE_CURSOR_DUMP`). Returns how many.
pub fn dump(dir: &std::path::Path, cursors: &[AppCursor], cx: &App) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    let mut n = 0;
    for &c in cursors {
        for windows in [true, false] {
            for scale in [1.0f32, 2.0] {
                let Some(img) = image(c, windows, scale, cx) else { continue };
                let name = format!("{}-{}-{}x.cur", stem(c), if windows { "win" } else { "mac" }, scale as u32);
                std::fs::write(dir.join(name), cur_file(&img))?;
                n += 1;
            }
        }
    }
    Ok(n)
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
