use noshape_render::*;

#[test]
fn renders_box_and_picks_faces() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let edges = r.upload_lines(&primitives::box_edges([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 10, 1.5));
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(mesh)));
    scene.instances.push(Instance::new(Drawable::Lines(edges)).with_color([0.02, 0.02, 0.03, 1.0]));
    scene.set_state(2, state::SELECTED);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.0175, 4.0 / 3.0);
    let style = Style::default();
    let f = r
        .render(RenderParams { scene: &scene, camera: &cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: &style, pick: true })
        .unwrap();
    assert_eq!(f.bgra.len(), 400 * 300 * 4);
    let pick = f.pick.unwrap();
    let center = pick.ids[(150 * 400 + 200) as usize];
    assert!(center != 0, "center pixel should hit the box");
    if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
        let mut rgba = f.bgra.clone();
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        image::save_buffer(format!("{dir}/box.png"), &rgba, 400, 300, image::ExtendedColorType::Rgba8).unwrap();
    }
}
