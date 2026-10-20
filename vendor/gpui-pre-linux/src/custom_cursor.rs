//! NOSHAPE PATCH (not in upstream gpui-pre-linux 0.3.9): an application
//! image cursor, or a CSS standard cursor GPUI has no style for, on Wayland.
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
    /// A CSS standard cursor GPUI has no style for ("move", "zoom-in",
    /// "no-drop", "wait", "help"), shown from the cursor-shape protocol or
    /// the cursor theme.
    static NAMED: RefCell<Option<&'static str>> = const { RefCell::new(None) };
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

/// Show the standard cursor with this CSS name instead of GPUI's cursor
/// style (`None` restores it). Returns false when no backend supports it.
pub fn set_named_cursor(css: Option<&'static str>) -> bool {
    NAMED.with(|c| *c.borrow_mut() = css);
    APPLY.with(|a| match &*a.borrow() {
        Some(apply) => {
            apply();
            true
        }
        None => false,
    })
}

pub(crate) fn current_named() -> Option<&'static str> {
    NAMED.with(|c| *c.borrow())
}

/// An application cursor (image or named) replaces the style cursor.
pub(crate) fn active() -> bool {
    current().is_some() || current_named().is_some()
}

#[cfg(feature = "wayland")]
pub(crate) fn set_apply_hook(hook: Box<dyn Fn()>) {
    APPLY.with(|a| *a.borrow_mut() = Some(hook));
}

thread_local! {
    static STYLE_STALE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The app has set the window's cursor itself (X11): GPUI's next cursor
/// style request applies again, even when its style hasn't changed.
pub fn invalidate_style() {
    STYLE_STALE.with(|s| s.set(true));
}

pub(crate) fn take_style_stale() -> bool {
    STYLE_STALE.with(|s| s.replace(false))
}
