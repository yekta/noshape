//! NOSHAPE PATCH (not in upstream gpui-pre-linux 0.3.9): an application
//! image cursor for Wayland.
//!
//! GPUI only knows named cursor styles. On Wayland the cursor belongs to
//! GPUI's `wl_pointer` (setting it needs the pointer-enter serial), so an
//! application can't set an image cursor from outside. With this patch the
//! application stores an image here; while one is set, the Wayland client
//! shows it instead of the style cursor (on pointer enter, on every style
//! change and right away), as a `wl_shm` buffer on its cursor surface.
//! Clearing it restores the style cursor. X11 and the other platforms
//! ignore it (noshape sets image cursors there through the platform APIs).

use std::cell::RefCell;
use std::rc::Rc;

/// A cursor image: premultiplied ARGB8888 (little-endian, i.e. BGRA bytes),
/// `width` x `height` buffer pixels at buffer scale `scale`; the hot spot in
/// buffer pixels.
#[derive(Clone)]
pub struct CustomCursor {
    pub argb: Rc<[u8]>,
    pub width: u32,
    pub height: u32,
    pub hot_x: u32,
    pub hot_y: u32,
    pub scale: i32,
}

thread_local! {
    static CURRENT: RefCell<Option<CustomCursor>> = const { RefCell::new(None) };
    /// Re-applies the cursor in the Wayland client (registered by it).
    static APPLY: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Show `cursor` instead of GPUI's cursor style (`None` restores the style
/// cursor). Returns false when no backend supports it (not on Wayland).
pub fn set_custom_cursor(cursor: Option<CustomCursor>) -> bool {
    CURRENT.with(|c| *c.borrow_mut() = cursor);
    APPLY.with(|a| match &*a.borrow() {
        Some(apply) => {
            apply();
            true
        }
        None => false,
    })
}

pub(crate) fn current() -> Option<CustomCursor> {
    CURRENT.with(|c| c.borrow().clone())
}

#[cfg(feature = "wayland")]
pub(crate) fn set_apply_hook(hook: Box<dyn Fn()>) {
    APPLY.with(|a| *a.borrow_mut() = Some(hook));
}
