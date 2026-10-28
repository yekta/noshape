//! Creates the viewport's first-frame pipelines once, for the macOS release
//! job: with `NOSHAPE_MSL_DUMP=<dir>` the vendored wgpu-hal writes each
//! pipeline's MSL to `<dir>`, and the job compiles them into the metallibs the
//! app ships (vendor/wgpu-hal/src/metal/precompiled.rs). A first launch then
//! loads them instead of compiling the shaders (~0.5 s).
fn main() {
    let mut renderer = noshape_render::Renderer::new().expect("no GPU adapter");
    renderer.prewarm(noshape_render::Style::default().msaa);
}
