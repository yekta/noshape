//! A Part Studio with hundreds of parts renders (ui's report 10-21: a
//! 900-part studio drew one line and logged "GPU wait failed", then a small
//! studio drew nothing). Built through the agent API: a pin (cylinder), a
//! Part pattern of 900 new parts (copies sharing one shape, so the renderer
//! draws them from one mesh with instancing), then a second Part Studio with
//! a single pin, rendered by the same renderer afterwards. Every frame must
//! show the parts (shaded, edge and picked pixels), not a blank viewport;
//! also when the GPU is slower than every wait (a 1 µs wait slice: a slow
//! frame used to be dropped as "GPU wait failed", and every frame queued
//! behind it timed out too).

use glam::DVec3;
use noshape_ops::registry;
use noshape_ops::Ctx;
use noshape_render::renderer::RenderParams;
use noshape_render::Projection;
use noshape_scene::builder::{self, DisplayMode, SceneOptions};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

/// A pin (r 2 mm, 10 mm) at the origin in `tab`; its feature id.
fn pin(ctx: &mut Ctx, tab: &str) -> String {
    let sk = call(ctx, "sketch_create", json!({ "tab": tab, "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    let st = call(ctx, "sketch_draw", json!({ "tab": tab, "sketch": sketch, "tool": "circle", "points": [[0, 0], [2, 0]] }));
    let region = st["regions"][0]["id"].as_str().unwrap().to_string();
    let f = call(ctx, "feature_add", json!({ "tab": tab, "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    f["feature"].as_str().unwrap().to_string()
}

/// (shaded pixels, near-black edge pixels) of a BGRA frame on the white
/// background.
fn coverage(bgra: &[u8]) -> (usize, usize) {
    let shaded = bgra.chunks(4).filter(|p| p[0] < 235 || p[1] < 235 || p[2] < 235).count();
    let edges = bgra.chunks(4).filter(|p| p[0] < 60 && p[1] < 60 && p[2] < 60).count();
    (shaded, edges)
}

#[test]
#[ignore = "T2: big model / perf"]
fn nine_hundred_parts_and_then_a_small_studio_render() {
    let dir = std::env::temp_dir().join(format!("noshape-many-parts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.noshape");
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let seed = pin(&mut ctx, "Part Studio 1");
    let p = call(
        &mut ctx,
        "feature_add",
        json!({ "tab": "Part Studio 1", "type": "linearPattern", "params": {
            "patternType": "PART", "entities": [format!("body:{seed}.0")],
            "directionOne": ["plane:right"], "distance": "6 mm", "instanceCount": "30",
            "directionTwo": ["plane:front"], "distanceTwo": "6 mm", "instanceCountTwo": "30", "hasSecondDir": true,
            "operationType": "NEW" } }),
    );
    assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "Pin" }));
    pin(&mut ctx, "Pin");

    let out = std::env::var("NOSHAPE_TEST_OUT").ok();
    let mut failures = Vec::new();
    for (slice, (tab_name, parts)) in [std::time::Duration::from_secs(10), std::time::Duration::from_micros(1)].into_iter().flat_map(|s| [("Part Studio 1", 900usize), ("Pin", 1)].map(|t| (s, t))) {
        let od = ctx.doc(path.to_str()).unwrap();
        let tab = noshape_ops::doc::find_tab(od.doc(), tab_name).unwrap();
        let r = od.regenerate(tab, None).unwrap();
        assert_eq!(r.display.bodies.len(), parts, "{tab_name}: parts");
        // Rendering needs a GPU adapter; CI's Linux runner has none.
        let off = match ctx.offscreen() {
            Ok(off) => off,
            Err(e) if e.code == "render_failed" => return eprintln!("render skipped: {}", e.message),
            Err(e) => panic!("offscreen: {}", e.to_json()),
        };
        off.renderer.set_wait_slice(slice);
        let up = off.renderer.uploader();
        let opts = SceneOptions { display_mode: DisplayMode::ShadedWithEdges, ..Default::default() };
        let built = builder::build(&r.display, &opts, &mut off.cache, &up);
        let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
        let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).expect("bounds");
        for projection in [Projection::Orthographic, Projection::Perspective] {
            let (w, h) = (640u32, 480u32);
            let mut camera = noshape_scene::png::fit_camera(lo, hi, w as f64 / h as f64, Some(noshape_render::StandardView::Isometric));
            camera.projection = projection;
            // A frame of the whole studio, then one close to a part.
            for close in [false, true] {
                let mut cam = camera;
                if close {
                    cam.fit_box(lo, lo + DVec3::splat(0.01), w as f64 / h as f64);
                }
                let f = match off.renderer.render(RenderParams { scene: &built.scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: true }) {
                    Ok(f) => f,
                    Err(e) => {
                        failures.push(format!("{tab_name}, {projection:?}, close {close}, wait slice {slice:?}: {e}"));
                        continue;
                    }
                };
                let (shaded, edges) = coverage(&f.bgra);
                let picked = f.pick.as_ref().map(|p| p.ids.iter().filter(|&&i| i != 0).count()).unwrap_or(0);
                if shaded < 2000 || edges < 50 || picked < 1000 {
                    failures.push(format!("{tab_name}, {projection:?}, close {close}, wait slice {slice:?}: {shaded} shaded px, {edges} edge px, {picked} picked px"));
                }
                if let Some(dir) = &out {
                    let png = noshape_scene::png::encode_png(f.bgra.clone(), w, h, w, h).unwrap();
                    std::fs::write(format!("{dir}/parts_{}_{projection:?}_{close}_{}us.png", tab_name.replace(' ', "_"), slice.as_micros()), png).unwrap();
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The user's fin plate (`tests/perf/fins-400.noshape`, perf.yml `fixture`;
/// qa4's v0.4.8 gate (a), /root/qa4-scratch/mk-fins2.py): plate 810 × 20 × 10
/// mm, a 1 × 20 × 5 mm wall on top patterned ×400 every 2 mm with Reapply
/// (LP1), the same wall under the plate (LP2). `NOSHAPE_FIXTURE_OUT=<path>`
/// writes it.
#[test]
#[ignore]
fn write_fins_fixture() {
    let out = std::env::var("NOSHAPE_FIXTURE_OUT").expect("NOSHAPE_FIXTURE_OUT");
    let _ = std::fs::remove_file(&out);
    let n = 400;
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": out }));
    let region = |ctx: &mut Ctx, i: usize, a: [f64; 2], b: [f64; 2]| {
        call(ctx, "sketch_create", json!({ "plane": "plane:top" }));
        call(ctx, "sketch_draw", json!({ "sketch": format!("Sketch {i}"), "tool": "corner_rectangle", "points": [a, b] }));
        call(ctx, "sketch_get", json!({ "sketch": format!("Sketch {i}") }))["regions"][0]["id"].as_str().unwrap().to_string()
    };
    let ok = |f: &Value| assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    let plate = region(&mut ctx, 1, [0.0, 0.0], [(2 * n + 10) as f64, 20.0]);
    ok(&call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [plate], "depth": "10 mm", "operationType": "NEW" } })));
    let pattern = |ctx: &mut Ctx, f: &str| call(ctx, "feature_add", json!({ "type": "linearPattern", "params": { "patternType": "FEATURE", "instanceFunction": [format!("feature:{f}")], "directionOne": ["plane:right"], "distance": "2 mm", "instanceCount": n.to_string(), "operationType": "ADD", "defaultScope": true, "fullFeaturePattern": true } }));
    let top = region(&mut ctx, 2, [2.0, 0.0], [3.0, 20.0]);
    let w1 = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [top], "operationType": "ADD", "defaultScope": true, "startOffset": true, "startOffsetBound": "BLIND", "startOffsetDistance": "10 mm", "depth": "5 mm" } }));
    ok(&w1);
    ok(&pattern(&mut ctx, w1["feature"].as_str().unwrap()));
    let bottom = region(&mut ctx, 3, [2.0, 0.0], [3.0, 20.0]);
    let w2 = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [bottom], "operationType": "ADD", "defaultScope": true, "oppositeDirection": true, "depth": "5 mm" } }));
    ok(&w2);
    ok(&pattern(&mut ctx, w2["feature"].as_str().unwrap()));
}

/// The 1000-part benchmark fixture (`tests/perf/parts-1000.noshape`, perf.yml
/// `fixture`): the pin as a 40 × 25 Part pattern of new parts (one shared
/// shape, drawn with instancing). `NOSHAPE_FIXTURE_OUT=<path>` writes it.
#[test]
#[ignore]
fn write_parts_1000_fixture() {
    let out = std::env::var("NOSHAPE_FIXTURE_OUT").expect("NOSHAPE_FIXTURE_OUT");
    let _ = std::fs::remove_file(&out);
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": out }));
    let seed = pin(&mut ctx, "Part Studio 1");
    let p = call(
        &mut ctx,
        "feature_add",
        json!({ "tab": "Part Studio 1", "type": "linearPattern", "params": {
            "patternType": "PART", "entities": [format!("body:{seed}.0")],
            "directionOne": ["plane:right"], "distance": "6 mm", "instanceCount": "40",
            "directionTwo": ["plane:front"], "distanceTwo": "6 mm", "instanceCountTwo": "25", "hasSecondDir": true,
            "operationType": "NEW" } }),
    );
    assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    let od = ctx.doc(Some(out.as_str())).unwrap();
    let tab = noshape_ops::doc::find_tab(od.doc(), "Part Studio 1").unwrap();
    assert_eq!(od.regenerate(tab, None).unwrap().display.bodies.len(), 1000);
}

/// The 1000-instance assembly benchmark fixture
/// (`tests/perf/pins-assembly-1000.noshape`, perf.yml `fixture`): the pin
/// from a Part Studio inserted 1000 times into an Assembly, the instances on
/// the 40 × 25 grid of `parts-1000` (6 mm), the Assembly first so the app
/// opens on it. `NOSHAPE_FIXTURE_OUT=<path>` writes it.
#[test]
#[ignore]
fn write_pins_assembly_fixture() {
    use noshape_model::assembly::{InstanceId, Placement};
    use noshape_model::document::TabContent;
    let out = std::env::var("NOSHAPE_FIXTURE_OUT").expect("NOSHAPE_FIXTURE_OUT");
    let _ = std::fs::remove_file(&out);
    let dir = std::env::temp_dir().join(format!("noshape-pins-asm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scratch = dir.join("x.noshape");
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch.to_str().unwrap() }));
    // The new Part Studio goes after Assembly 1.
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "Pin" }));
    call(&mut ctx, "tab_delete", json!({ "tab": "Part Studio 1" }));
    pin(&mut ctx, "Pin");
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "Pin" }));
    let part = parts["parts"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Pin", "part": part }));
    // The other 999 copies in one go (a single microversion keeps the file small).
    let mut doc = ctx.doc(scratch.to_str()).unwrap().doc().clone();
    let tab = noshape_ops::doc::find_tab(&doc, "Assembly 1").unwrap();
    let Some(TabContent::Assembly(asm)) = doc.tab_mut(tab).map(|t| &mut t.content) else { panic!("Assembly 1") };
    let seed = asm.instances[0].clone();
    asm.instances.clear();
    for i in 0..1000u64 {
        let (x, y) = ((i % 40) as f64 * 0.006, (i / 40) as f64 * 0.006);
        let name = asm.next_instance_name(seed.name.rsplit_once(' ').map_or(seed.name.as_str(), |(base, _)| base));
        let id = InstanceId(asm.next_id());
        asm.instances.push(noshape_model::assembly::Instance { id, name, placement: Placement { translation: [x, y, 0.0], ..Placement::default() }, ..seed.clone() });
    }
    ctx.create(std::path::Path::new(&out), doc).unwrap();
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(list["instances"].as_array().unwrap().len(), 1000);
    let info = call(&mut ctx, "document_info", json!({}));
    assert_eq!(info["tabs"][0]["name"], "Assembly 1", "{info}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// GPU cost of the 1000-part fixture (perf.yml `fixture` parts-1000):
/// 20 orbit frames at the CI runners' 1194×794, the app's style; mean and
/// worst frame. Run once per diagnostic (`NOSHAPE_PERF_SKIP=lines|meshes|
/// instancing`, `NOSHAPE_PERF_MSAA=1`, `NOSHAPE_TEST_AO=0`) to see what the
/// frame spends.
#[test]
#[ignore]
fn thousand_pins_frame_time() {
    let dir = std::env::temp_dir().join(format!("noshape-pins-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.noshape");
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let seed = pin(&mut ctx, "Part Studio 1");
    call(
        &mut ctx,
        "feature_add",
        json!({ "tab": "Part Studio 1", "type": "linearPattern", "params": {
            "patternType": "PART", "entities": [format!("body:{seed}.0")],
            "directionOne": ["plane:right"], "distance": "6 mm", "instanceCount": "40",
            "directionTwo": ["plane:front"], "distanceTwo": "6 mm", "instanceCountTwo": "25", "hasSecondDir": true,
            "operationType": "NEW" } }),
    );
    let od = ctx.doc(path.to_str()).unwrap();
    let tab = noshape_ops::doc::find_tab(od.doc(), "Part Studio 1").unwrap();
    let r = od.regenerate(tab, None).unwrap();
    let off = ctx.offscreen().unwrap();
    let up = off.renderer.uploader();
    let built = builder::build(&r.display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, ..Default::default() }, &mut off.cache, &up);
    let mut style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    if std::env::var("NOSHAPE_TEST_AO").is_ok_and(|v| v == "0") {
        style.ambient_occlusion = 0.0;
    }
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).expect("bounds");
    let (w, h) = (1194u32, 794u32);
    let mut cam = noshape_scene::png::fit_camera(lo, hi, w as f64 / h as f64, Some(noshape_render::StandardView::Isometric));
    let tris: usize = r.display.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
    let mut times = Vec::new();
    for i in 0..21 {
        let t = std::time::Instant::now();
        off.renderer.render(RenderParams { scene: &built.scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
        if i > 0 {
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        cam.rotation = glam::DQuat::from_rotation_z(0.05) * cam.rotation;
    }
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let worst = times.iter().cloned().fold(0.0, f64::max);
    eprintln!("1000 pins, {} instances, {tris} triangles, {w}x{h}: frame mean {mean:.1} ms, worst {worst:.1} ms (skip {:?}, msaa {:?}, ao {})", built.scene.instances.len(), std::env::var("NOSHAPE_PERF_SKIP").ok(), std::env::var("NOSHAPE_PERF_MSAA").ok(), style.ambient_occlusion);
    let _ = std::fs::remove_dir_all(&dir);
}
