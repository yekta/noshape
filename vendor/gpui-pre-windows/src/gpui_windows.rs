#![cfg(target_os = "windows")]

mod clipboard;
mod destination_list;
mod dialog;
mod direct_manipulation;
mod direct_write;
mod directx_atlas;
mod directx_devices;
mod directx_renderer;
mod dispatcher;
mod display;
mod events;
mod keyboard;
mod platform;
mod system_notifications;
mod system_settings;
mod util;
mod vsync;
mod window;
mod wrapper;

pub(crate) use clipboard::*;
pub(crate) use destination_list::*;
pub(crate) use direct_write::*;
pub(crate) use directx_atlas::*;
pub(crate) use directx_devices::*;
pub(crate) use directx_renderer::*;
pub(crate) use dispatcher::*;
pub(crate) use display::*;
pub(crate) use events::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
pub(crate) use system_notifications::*;
pub(crate) use system_settings::*;
pub(crate) use util::*;
pub(crate) use vsync::*;
pub(crate) use window::*;
pub(crate) use wrapper::*;

pub use platform::WindowsPlatform;

/// NOSHAPE PATCH: whether pen contacts act as a mouse (Onshape's "Use pen
/// input as mouse"); otherwise they are touches like fingers.
pub(crate) static PEN_AS_MOUSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// NOSHAPE PATCH: set "Use pen input as mouse" (see [`PEN_AS_MOUSE`]).
pub fn set_pen_as_mouse(on: bool) {
    PEN_AS_MOUSE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) use windows::Win32::Foundation::HWND;
