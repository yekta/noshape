//! noshape-render: offscreen wgpu viewport renderer.
//!
//! See `docs/ARCHITECTURE.md`. The renderer draws a [`Scene`] from a
//! [`Camera`] into an sRGB BGRA8 [`Frame`] (ready for a GPUI `RenderImage`)
//! and optionally a [`PickBuffer`] of per-pixel pick ids.

mod ao;
pub mod camera;
pub mod overlay;
mod overlay_gpu;
pub mod perf;
pub mod primitives;
pub mod renderer;
pub mod scene;
pub mod worker;

pub use camera::{Camera, Projection, StandardView};
pub use renderer::{Completed, DepthSample, Frame, FrameRequest, PickBuffer, PickRect, Present, PresentTexture, RenderError, RenderParams, Renderer};
pub use scene::*;
/// The wgpu the renderer uses (for platform interop, e.g. zero-copy presentation).
pub use wgpu;
