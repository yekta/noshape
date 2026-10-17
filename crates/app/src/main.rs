mod gpu;
mod viewport;

use gpui_kit::*;
use noshape_render::{primitives, Drawable, Instance, Scene};
use std::sync::Arc;
use viewport::Viewport;

struct Demo {
    viewport: Entity<Viewport>,
}

impl Render for Demo {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.viewport.clone())
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let gpu = gpu::Gpu::new().expect("no GPU available");
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let gpu = gpu.clone();
        gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
            let viewport = cx.new(|cx| {
                let mut vp = Viewport::new(gpu.clone(), window, cx);
                let up = gpu.uploader();
                let mesh = up.upload_mesh(&primitives::box_mesh([-0.01; 3], [0.01; 3], 1));
                let edges = up.upload_lines(&primitives::box_edges([-0.01; 3], [0.01; 3], 10, 1.5));
                let mut scene = Scene::default();
                scene.instances.push(Instance::new(Drawable::Mesh(mesh)));
                scene.instances.push(Instance::new(Drawable::Lines(edges)).with_color([0.02, 0.02, 0.03, 1.0]));
                vp.set_scene(Arc::new(scene), true, cx);
                vp.camera.fit_sphere(glam::DVec3::ZERO, 0.0175, 1.3);
                vp
            });
            cx.new(|_| Demo { viewport })
        })
        .expect("failed to open window");
    });
}
