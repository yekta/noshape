//! "Hidden edges removed" (the default): no edge behind a face may show
//! (user reports 10-21: short edge segments and triangles at the roots of
//! thin fins at close zoom, on a part with 1000 fins). The user's fins,
//! built through the agent API: a block 100 × 2010 × 40 mm, a 1 mm fin
//! 20 mm high on top (x 10..90), flush with the block's front face, and a
//! linear pattern of 1000 fins 2 mm apart, as new parts (a Part pattern:
//! the renderer draws the copies from one mesh; the fins' edges and faces
//! sit where a Feature pattern's merged fins have theirs, and the model's
//! own fin tests are kern's pattern_fins.rs). The scene is the app's (scene
//! builder, shaded with edges); magenta
//! probe lines lie exactly on edges the fins hide (each fin's root on its
//! back side, and its far vertical end edge), drawn by the edge pipeline at
//! the same depth as the real edges, inset a few pixels from the visible
//! corners. No magenta may survive, orthographic or perspective, at 1x and 2x,
//! from a close view of the fin roots to a far one.

use glam::{DVec3, Vec3};
use noshape_ops::registry;
use noshape_ops::Ctx;
use noshape_render::renderer::RenderParams;
use noshape_render::{Camera, Drawable, Instance, LineData, LineSegment, Projection};
use noshape_scene::builder::{self, DisplayMode, SceneOptions};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

/// A sketch on Top with one rectangle (mm); its region.
fn rect_region(ctx: &mut Ctx, lo: [f64; 2], hi: [f64; 2]) -> String {
    let sk = call(ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    call(ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": [lo, hi] }));
    let st = call(ctx, "sketch_get", json!({ "sketch": sketch }));
    st["regions"][0]["id"].as_str().unwrap().to_string()
}

const FINS: usize = 1000;
const MM: f64 = 1e-3;

#[test]
fn edges_behind_fins_stay_hidden() {
    let dir = std::env::temp_dir().join(format!("noshape-hidden-edges-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fins.noshape");
    let _ = std::fs::remove_file(&path);
    let mut ctx = Ctx::new();
    // NOSHAPE_TEST_FINS_DOC: a document with the same part made another way
    // (renderer work while the pattern itself is being changed).
    if let Ok(doc) = std::env::var("NOSHAPE_TEST_FINS_DOC") {
        std::fs::copy(doc, &path).unwrap();
        call(&mut ctx, "document_open", json!({ "path": path.to_str().unwrap() }));
    } else {
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let block = rect_region(&mut ctx, [0.0, 0.0], [100.0, (2 * FINS + 10) as f64]);
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [block], "depth": "40 mm" } }));
        let fin = rect_region(&mut ctx, [10.0, 0.0], [90.0, 1.0]);
        let f = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [fin], "startOffset": true, "startOffsetBound": "BLIND", "startOffsetDistance": "40 mm", "depth": "20 mm", "operationType": "NEW" } }));
        let fin_feature = f["feature"].as_str().unwrap().to_string();
        let p = call(
            &mut ctx,
            "feature_add",
            json!({ "type": "linearPattern", "params": { "patternType": "PART", "entities": [format!("body:{fin_feature}.0")], "directionOne": ["plane:front"], "oppositeDirection": true, "distance": "2 mm", "instanceCount": FINS.to_string(), "operationType": "NEW" } }),
        );
        assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    }

    let od = ctx.doc(path.to_str()).unwrap();
    let tab = noshape_ops::doc::tab_of_kind(od.doc(), None, "part studio", noshape_ops::doc::is_part_studio).unwrap();
    let r = od.regenerate(tab, None).unwrap();
    // The block and the fins (one merged part when NOSHAPE_TEST_FINS_DOC is
    // a Feature pattern's or a comb extrude's).
    let faces: usize = r.display.bodies.iter().map(|b| b.face_refs.len()).sum();
    assert!(faces == 6 + 6 * FINS || faces == 5 * FINS + 5, "the block with {FINS} fins: {} parts, {faces} faces", r.display.bodies.len());
    // Rendering needs a GPU adapter; CI's Linux runner has none.
    let off = match ctx.offscreen() {
        Ok(off) => off,
        Err(e) if e.code == "render_failed" => return eprintln!("render skipped: {}", e.message),
        Err(e) => panic!("offscreen: {}", e.to_json()),
    };
    let up = off.renderer.uploader();
    let opts = SceneOptions { display_mode: DisplayMode::ShadedWithEdges, ..Default::default() };
    let built = builder::build(&r.display, &opts, &mut off.cache, &up);
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());

    // Fin k: x 10..90, y 2k..2k+1, z 40..60 mm. Looking toward +x, +y, -z:
    // the fins' front (-y) sides, near (x 10) ends and tops face the camera;
    // each fin hides its root on its back (+y) side and its back far end.
    let look = DVec3::new(0.55, 0.75, -0.45).normalize();
    let out = std::env::var("NOSHAPE_TEST_OUT").ok();
    let mut failures = Vec::new();
    for projection in [Projection::Orthographic, Projection::Perspective] {
        // Half extents of the view around a fin root mid-pattern (mm).
        for zoom in [4.0, 12.0, 40.0] {
            for dpr in [1.0f32, 2.0] {
                let (w, h) = ((480.0 * dpr) as u32, (320.0 * dpr) as u32);
                let mut camera = Camera { rotation: Camera::look_rotation(look, DVec3::Z), projection, ..Default::default() };
                let root = DVec3::new(10.0, (2 * (FINS / 2)) as f64, 40.0) * MM;
                camera.fit_box(root - DVec3::splat(zoom * MM), root + DVec3::splat(zoom * MM), w as f64 / h as f64);
                // Probes inset 4 px from the visible corners (line widths
                // overlap there).
                let inset = 4.0 * camera.world_per_pixel(h as f64) / MM;
                let p3 = |x: f64, y: f64, z: f64| -> [f32; 3] { Vec3::new((x * MM) as f32, (y * MM) as f32, (z * MM) as f32).into() };
                let mut probes = LineData::default();
                for k in (FINS / 2 - 30)..(FINS / 2 + 30) {
                    // Control, cyan, on edges nothing hides: each fin's top
                    // front edge (convex; the fins in front are no taller)
                    // and its root at the near end (concave, facing the
                    // camera). (Its front root is hidden by the fin in front.)
                    let front = (2 * k) as f64;
                    let cyan = |a, b| LineSegment { a, b, color: [0, 255, 255, 255], pick_id: 0, width: 1.25, style: 0 };
                    probes.segments.push(cyan(p3(10.0 + inset, front, 60.0), p3(90.0 - inset, front, 60.0)));
                    probes.segments.push(cyan(p3(10.0, front + 0.25, 40.0), p3(10.0, front + 0.75, 40.0)));
                    let y = (2 * k + 1) as f64;
                    for (a, b) in [((10.0 + inset, y, 40.0), (90.0 - inset, y, 40.0)), ((90.0, y, 40.0 + inset), (90.0, y, 60.0 - inset))] {
                        if a.0 < b.0 || a.2 < b.2 {
                            probes.segments.push(LineSegment { a: p3(a.0, a.1, a.2), b: p3(b.0, b.1, b.2), color: [255, 0, 255, 255], pick_id: 0, width: 1.25, style: 0 });
                        }
                    }
                }
                let mut scene = built.scene.clone();
                scene.instances.push(Instance::new(Drawable::Lines(off.renderer.upload_lines(&probes))).per_primitive_color().layer(1));
                let f = off.renderer.render(RenderParams { scene: &scene, camera: &camera, width: w, height: h, device_pixel_ratio: dpr, style: &style, pick: false }).unwrap();
                // Magenta: no part color or shading of one comes near it.
                let shown = f.bgra.chunks(4).filter(|p| p[2] > 150 && p[1] < 90 && p[0] > 150).count();
                if shown > 0 {
                    failures.push(format!("{projection:?}, ±{zoom} mm, {dpr}x: {shown} pixels of hidden edges show"));
                }
                let control = f.bgra.chunks(4).filter(|p| p[2] < 90 && p[1] > 150 && p[0] > 150).count();
                if control < 20 {
                    failures.push(format!("{projection:?}, ±{zoom} mm, {dpr}x: only {control} pixels of the visible control edges show"));
                }
                if let Some(dir) = &out {
                    let png = noshape_scene::png::encode_png(f.bgra.clone(), w, h, w, h).unwrap();
                    std::fs::write(format!("{dir}/fins_{projection:?}_{zoom}mm_{dpr}x.png"), png).unwrap();
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}
