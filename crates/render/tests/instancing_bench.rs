//! 1000 copies of one part (shared mesh, edges and vertices; own transform
//! and pick ids): frame CPU time on the render thread and total frame time.
//! `cargo test --release -p noshape-render --test all instancing_bench:: -- --ignored --nocapture`

use noshape_render::renderer::{FrameRequest, Present};
use noshape_render::*;
use std::sync::Arc;
use std::time::Instant;

#[test]
#[ignore]
fn thousand_copies() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (lo, hi) = ([0.0f32; 3], [0.02f32, 0.02, 0.01]);
    let mesh = r.upload_mesh(&primitives::box_mesh(lo, hi, 1));
    let edges = r.upload_lines(&primitives::box_edges(lo, hi, 1, 1.25));
    let corners: Vec<PointSprite> = (0..8u32).map(|k| PointSprite { position: [[lo[0], hi[0]][(k & 1) as usize], [lo[1], hi[1]][(k >> 1 & 1) as usize], [lo[2], hi[2]][(k >> 2) as usize]], color: [255; 4], pick_id: k + 1, size: 6.0, style: 0, _pad: 0 }).collect();
    let points = r.upload_points(&PointData { points: corners });
    let mut scene = Scene::default();
    for i in 0..1000u32 {
        let t = glam::Mat4::from_translation(glam::Vec3::new((i % 40) as f32 * 0.03, (i / 40) as f32 * 0.03, 0.0));
        let base = i * 40;
        scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_color([0.45, 0.6, 0.75, 1.0]).with_transform(t).pick_base(base));
        scene.instances.push(Instance::new(Drawable::Lines(edges.clone())).with_color([0.0, 0.0, 0.0, 1.0]).with_transform(t).pick_base(base + 10));
        scene.instances.push(Instance::new(Drawable::Points(points.clone())).with_color([0.0, 0.0, 0.0, 0.0]).with_transform(t).pick_base(base + 20));
    }
    scene.states = vec![0; 40_001];
    let scene = Arc::new(scene);
    let (w, h) = (1280u32, 800u32);
    let mut cam = Camera::default();
    let (blo, bhi) = scene.bounds().unwrap();
    cam.fit_box(blo.as_dvec3(), bhi.as_dvec3(), w as f64 / h as f64);
    r.set_present(1, Present::Texture);
    let mut cpu = std::time::Duration::ZERO;
    let n = 30u32;
    let t_all = Instant::now();
    for i in 0..n + 3 {
        let mut c = cam;
        c.rotation = glam::DQuat::from_rotation_z(i as f64 * 0.01) * c.rotation;
        let req = FrameRequest { scene: scene.clone(), camera: c, width: w, height: h, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: true };
        let t = Instant::now();
        r.submit_frame(1, i as u64, req).unwrap();
        if i >= 3 {
            cpu += t.elapsed();
        }
        while r.has_pending() {
            r.wait_oldest();
            drop(r.collect());
        }
        if i == 2 {
            cpu = std::time::Duration::ZERO;
        }
    }
    let total = t_all.elapsed() / (n + 3);
    eprintln!("BENCH instancing: 1000 copies x (mesh, edges, points), encode+submit CPU {:.3} ms/frame, frame incl. GPU {:.2} ms", cpu.as_secs_f64() * 1e3 / n as f64, total.as_secs_f64() * 1e3);
}
