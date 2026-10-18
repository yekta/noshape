#![cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;
// NOSHAPE PATCH: application image cursors on Wayland (see custom_cursor.rs).
pub mod custom_cursor;

pub use linux::{current_platform, linux_platform};
