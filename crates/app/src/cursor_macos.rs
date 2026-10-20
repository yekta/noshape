//! macOS backend of the graphics-area cursors (see cursor.rs): an `NSCursor`
//! made from the image, or AppKit's own cursor where Chrome uses one
//! (`no-drop` = `operationNotAllowedCursor`; Chrome draws move and zoom-in
//! itself, and so do we). GPUI re-sets its cursor through
//! cursor rects only when the hovered style changes; the viewport re-applies
//! ours.

use super::{AppCursor, CursorImage};
use image::ImageEncoder as _;
use objc2::AllocAnyThread as _;
use objc2::rc::Retained;
use objc2_app_kit::{NSCursor, NSImage};
use objc2_foundation::{NSData, NSPoint, NSSize};
use raw_window_handle::RawWindowHandle;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CURSORS: RefCell<HashMap<(AppCursor, u32), Retained<NSCursor>>> = RefCell::new(HashMap::new());
}

fn make(img: &CursorImage, scale: u32) -> Option<Retained<NSCursor>> {
    // Straight BGRA → RGBA PNG; the NSImage is sized in points so a 2x image
    // stays sharp on Retina screens.
    let mut rgba = img.bgra.clone();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(&rgba, img.side, img.side, image::ExtendedColorType::Rgba8).ok()?;
    let data = NSData::with_bytes(&png);
    let ns = NSImage::initWithData(NSImage::alloc(), &data)?;
    let pts = img.side as f64 * 100.0 / scale.max(1) as f64;
    ns.setSize(NSSize::new(pts, pts));
    let k = 100.0 / scale.max(1) as f64;
    Some(NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &ns, NSPoint::new(img.hot.0 as f64 * k, img.hot.1 as f64 * k)))
}

pub fn set(raw: RawWindowHandle, scale: u32, cursor: Option<AppCursor>, image: &mut dyn FnMut(AppCursor, bool) -> Option<CursorImage>) -> bool {
    if !matches!(raw, RawWindowHandle::AppKit(_)) {
        return false;
    }
    match cursor {
        None => {
            NSCursor::arrowCursor().set();
            true
        }
        Some(AppCursor::NoDrop) => {
            NSCursor::operationNotAllowedCursor().set();
            true
        }
        Some(c) => CURSORS.with(|m| {
            let mut m = m.borrow_mut();
            let cur = match m.entry((c, scale)) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    let Some(cur) = image(c, false).and_then(|img| make(&img, scale)) else { return false };
                    e.insert(cur)
                }
            };
            cur.set();
            true
        }),
    }
}
