#![cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;
// NOSHAPE PATCH: application image cursors on Wayland (see custom_cursor.rs).
pub mod custom_cursor;

pub use linux::{current_platform, linux_platform};

// NOSHAPE PATCH: the newest mouse press's number and the time since the press before it by the X server's own event
// times (`None` before a second press). A busy UI thread handles a second click late; an app's own double-click rule
// (the feature list's 400 ms) measures this instead.
thread_local! {
    static LAST_PRESS: std::cell::Cell<(u64, Option<std::time::Duration>)> = const { std::cell::Cell::new((0, None)) };
}

/// See `LAST_PRESS`.
pub fn last_press() -> (u64, Option<std::time::Duration>) {
    LAST_PRESS.with(|c| c.get())
}

pub(crate) fn set_last_press(interval: Option<std::time::Duration>) {
    LAST_PRESS.with(|c| c.set((c.get().0 + 1, interval)));
}
