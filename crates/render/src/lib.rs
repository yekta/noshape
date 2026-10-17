//! noshape-render: offscreen wgpu viewport renderer.
//!
//! See `docs/ARCHITECTURE.md`. The renderer draws a [`Scene`] from a
//! [`Camera`] into an sRGB BGRA8 [`Frame`] (ready for a GPUI `RenderImage`)
//! and optionally a [`PickBuffer`] of per-pixel pick ids.

pub mod camera;
pub mod primitives;
pub mod renderer;
pub mod scene;
pub mod worker;

pub use camera::{Camera, Projection, StandardView};
pub use renderer::{Frame, PickBuffer, RenderError, RenderParams, Renderer};
pub use scene::*;
