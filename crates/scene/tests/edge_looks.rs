//! EDGE-HIGHLIGHT-LOOK: a part edge's looks at rest, hovered, selected and both, and a face's boundary hovered and
//! selected, as Onshape draws them (live behaviours "psui (2026-10-27, P1) EDGE-HIGHLIGHT-LOOK": a 300×200×3 plate,
//! light theme, Front view, F, ArrowDown ×2 (the lit top face over the dark front face), a 1673×886 viewport at
//! device pixel ratio 1 and 2; the profile is a pixel column across the top-front edge). The scene is built as the
//! app builds it (`builder::build`, `with_highlights`).

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::StudioDisplay;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_render::renderer::{RenderParams, Renderer};
use noshape_scene::builder::{self, DisplayMode, GpuCache, SceneOptions};
use noshape_units::{ExprValue, UnitPrefs};

/// The live plate: a 300 × 200 rectangle on Top, extruded 3 mm.
fn block() -> StudioDisplay {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let pts = [DVec2::new(0.0, 0.0), DVec2::new(0.3, 0.0), DVec2::new(0.3, 0.2), DVec2::new(0.0, 0.2)];
    for i in 0..4 {
        sk.add_line(pts[i], pts[(i + 1) % 4]);
    }
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let regen = |ps: &PartStudio| Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("3 mm")));
    ps.features.push(ex);
    let out = regen(&ps);
    assert_eq!(out.display.bodies.len(), 1);
    out.display
}

/// The top-front edge (y = 0, z = 3 mm) and the top face of the plate.
fn top_front(display: &StudioDisplay) -> (Reference, Reference) {
    let b = &display.bodies[0];
    let edge = b
        .edge_refs
        .iter()
        .enumerate()
        .find(|(i, _)| b.mesh.edges.get(*i).is_some_and(|e| e.len() >= 2 && e.iter().all(|p| p[1].abs() < 1e-6 && (p[2] - THICK).abs() < 1e-6)))
        .map(|(_, r)| r.clone())
        .expect("the top-front edge");
    let top = b
        .face_refs
        .iter()
        .enumerate()
        .find(|(fi, _)| {
            let Some(&(first, count)) = b.mesh.face_ranges.get(*fi) else { return false };
            b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&i| (b.mesh.positions[i as usize][2] - THICK).abs() < 1e-6)
        })
        .map(|(_, r)| r.clone())
        .expect("the top face");
    (edge, top)
}

const THICK: f32 = 0.003;

/// A state's name and its sRGB column across the edge.
type Profile = (&'static str, Vec<[u8; 3]>);

/// The sRGB column across the edge (`before` rows above the edge's own row and `after` below, at the viewport's
/// middle), per state, at a device pixel ratio.
fn profiles(dpr: f32, before: u32, after: u32) -> Option<Vec<Profile>> {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let display = block();
    let (edge, top) = top_front(&display);
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let up = r.uploader();
    let (w, h) = ((1673.0 * dpr) as u32, (886.0 * dpr) as u32);
    let mut cache = GpuCache::default();
    let built = builder::build(&display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() }, &mut cache, &up);
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap();
    // Front, then ArrowDown twice (15° each): looking 30° down onto the top face.
    let mut cam = noshape_render::Camera::default();
    let t = 30f64.to_radians();
    cam.rotation = noshape_render::Camera::look_rotation(glam::DVec3::new(0.0, t.cos(), -t.sin()), glam::DVec3::new(0.0, t.sin(), t.cos()));
    cam.fit_box(lo, hi, w as f64 / h as f64);
    let x = w / 2;
    let row = cam.project(glam::DVec3::new(0.15, 0.0, THICK as f64), glam::DVec2::new(w as f64, h as f64), 1.0).unwrap().y as u32;
    let (from, to) = (row - before, row + after);
    let states: [(&str, Vec<Reference>, Option<Reference>); 7] = [
        ("rest", vec![], None),
        ("edge hovered", vec![], Some(edge.clone())),
        ("edge selected", vec![edge.clone()], None),
        ("edge selected + hovered", vec![edge.clone()], Some(edge.clone())),
        ("face hovered", vec![], Some(top.clone())),
        ("face selected", vec![top.clone()], None),
        ("face selected + hovered", vec![top.clone()], Some(top.clone())),
    ];
    let mut out = Vec::new();
    for (name, sel, hover) in states {
        let scene = builder::with_highlights(&built, &display, &sel, hover.as_ref(), &[], Vec::new(), false, &up);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: dpr, style: &style, pick: false }).unwrap();
        if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
            let png = noshape_scene::png::encode_png(f.bgra.clone(), w, h, w, h).unwrap();
            std::fs::write(format!("{dir}/edge_looks_{}_{dpr}x.png", name.replace(' ', "_")), png).unwrap();
        }
        let col = (from..to).map(|y| {
            let o = ((y * w + x) * 4) as usize;
            [f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]]
        });
        out.push((name, col.collect()));
    }
    Some(out)
}

/// Prints the profiles (cargo test -p noshape-scene --test all edge_looks:: -- --ignored --nocapture).
#[test]
#[ignore]
fn print_edge_profiles() {
    for dpr in [1.0f32, 2.0] {
        let k = dpr as u32;
        let Some(p) = profiles(dpr, 8 * k, 9 * k) else { return };
        println!("== {dpr}x");
        for (name, col) in p {
            println!("{name:>24}: {}", col.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" "));
        }
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn close(a: [u8; 3], b: [u8; 3], tol: i32) -> bool {
    (0..3).all(|k| (a[k] as i32 - b[k] as i32).abs() <= tol)
}

/// `over` at `alpha` over `under` (sRGB, as WebGL blends).
fn mix(over: [u8; 3], under: [u8; 3], alpha: f32) -> [u8; 3] {
    let m = |k: usize| (over[k] as f32 * alpha + under[k] as f32 * (1.0 - alpha)).round() as u8;
    [m(0), m(1), m(2)]
}

/// Live's rules (behaviours "psui (2026-10-27, P1) EDGE-HIGHLIGHT-LOOK"), at 1× and 2×: a hovered or selected part
/// edge is a 5 CSS px band in one colour (#FFC685 / #FFA90A), its rim opaque and its core at 78 % over the scene (the
/// plain edge darkening the middle), with no dark outline and nothing changed outside it; hovered + selected looks
/// like selected; a hovered face keeps its colour and its boundary is a ~2 device px #FFC685 line; a selected face
/// is lit orange with a #FFA90A boundary; a plain edge is a 1 CSS px near-black line.
#[test]
fn edge_and_face_highlights_look_as_live() {
    const HOVER: [u8; 3] = [0xFF, 0xC6, 0x85];
    const SELECT: [u8; 3] = [0xFF, 0xA9, 0x0A];
    for dpr in [1.0f32, 2.0] {
        let k = dpr as usize;
        let (before, after) = (8 * k, 9 * k);
        let Some(p) = profiles(dpr, before as u32, after as u32) else { return };
        let get = |name: &str| p.iter().find(|(n, _)| *n == name).map(|(_, c)| c.clone()).unwrap();
        let rest = get("rest");
        let (top, side) = (rest[0], rest[rest.len() - 1]);
        let show = |c: &[[u8; 3]]| c.iter().map(|&x| hex(x)).collect::<Vec<_>>().join(" ");
        // The plain edge: near-black at its middle, about 1 CSS px.
        let dark = rest.iter().filter(|c| c.iter().all(|&v| v < 0x60)).count();
        assert!(rest.iter().any(|c| c.iter().all(|&v| v < 0x24)) && dark <= k + 1, "{dpr}x plain edge: {}", show(&rest));
        for (name, colour) in [("edge hovered", HOVER), ("edge selected", SELECT)] {
            let col = get(name);
            // The band: the rows that differ from the plain render.
            let lit: Vec<usize> = (0..col.len()).filter(|&i| !close(col[i], rest[i], 3)).collect();
            let (a, b) = (lit[0], *lit.last().unwrap());
            assert!(lit.len() == b - a + 1, "{dpr}x {name}: one band: {}", show(&col));
            let width = (b - a + 1) as f32 / dpr;
            assert!((4.5..=6.5).contains(&width), "{dpr}x {name}: {width} CSS px: {}", show(&col));
            // Opaque rim at the ends, the core at 78 % over the scene: over a face, or darker where the plain edge
            // shows through (between 78 % over the face and 78 % over what the plain render has there).
            assert!(col[a..=b].contains(&colour), "{dpr}x {name}: no opaque rim: {}", show(&col));
            let core = &col[a + k..=b - k];
            for (i, &c) in core.iter().enumerate() {
                let under = rest[a + k + i];
                let face = if a + k + i < before { top } else { side };
                let (lo, hi) = (mix(colour, under, 0.78), mix(colour, face, 0.78));
                let within = (0..3).all(|ch| (c[ch] as i32) >= lo[ch].min(hi[ch]) as i32 - 8 && (c[ch] as i32) <= lo[ch].max(hi[ch]) as i32 + 8);
                assert!(within || c == colour, "{dpr}x {name}: core {} isn't 78 % over {} / {}: {}", hex(c), hex(under), hex(face), show(&col));
            }
            // No dark outline or shadow: everything outside the band as in the plain render.
            for i in (0..a).chain(b + 1..col.len()) {
                assert!(close(col[i], rest[i], 3), "{dpr}x {name}: row {i} changed outside the band: {}", show(&col));
            }
            // Never darker than the face beside the band (a dark border).
            assert!(col[a..=b].iter().all(|c| c[0] > side[0]), "{dpr}x {name}: dark pixel in the band: {}", show(&col));
        }
        assert_eq!(get("edge selected + hovered"), get("edge selected"), "{dpr}x: hovering a selected edge keeps its look");
        // Faces.
        let fh = get("face hovered");
        assert_eq!(fh[0], top, "{dpr}x: a hovered face isn't tinted");
        let lit: Vec<usize> = (0..fh.len()).filter(|&i| !close(fh[i], rest[i], 3)).collect();
        assert!(fh.contains(&HOVER) && lit.len() <= 3, "{dpr}x face hovered boundary: {}", show(&fh));
        let fs = get("face selected");
        assert!(close(fs[0], [0xE5, 0xA2, 0x22], 4), "{dpr}x selected face {} (live E5A222)", hex(fs[0]));
        assert!(fs.contains(&SELECT), "{dpr}x face selected boundary: {}", show(&fs));
        assert_eq!(get("face selected + hovered"), fs, "{dpr}x: hovering a selected face changes nothing");
    }
}

/// PART-HOVER-RED: with the edited feature failing, its inputs (the selection) are drawn red, but a hovered part
/// (`accent`: its faces and edges) keeps the hover colour (live: the light orange part outline).
#[test]
fn a_failing_feature_leaves_the_hovered_part_unred() {
    let r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    use noshape_render::scene::state;
    let display = block();
    let (edge, top) = top_front(&display);
    let up = r.uploader();
    let mut cache = GpuCache::default();
    let built = builder::build(&display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() }, &mut cache, &up);
    let scene = builder::with_highlights(&built, &display, std::slice::from_ref(&edge), None, std::slice::from_ref(&top), Vec::new(), true, &up);
    let flags = |r: &Reference| built.picks.ids_for(r).iter().map(|&id| scene.states[id as usize]).collect::<Vec<u32>>();
    assert!(flags(&edge).iter().all(|f| f & state::ERROR != 0), "the failing feature's input is red: {:?}", flags(&edge));
    assert!(flags(&top).iter().all(|f| f & state::ACCENT != 0 && f & state::ERROR == 0), "the hovered part's face isn't: {:?}", flags(&top));
}
