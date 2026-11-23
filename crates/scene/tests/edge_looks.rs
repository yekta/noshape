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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
    let part = Reference::Body(display.bodies[0].id.clone());
    let states: [(&str, Vec<Reference>, Option<Reference>); 10] = [
        ("rest", vec![], None),
        ("edge hovered", vec![], Some(edge.clone())),
        ("edge selected", vec![edge.clone()], None),
        ("edge selected + hovered", vec![edge.clone()], Some(edge.clone())),
        ("face hovered", vec![], Some(top.clone())),
        ("face selected", vec![top.clone()], None),
        ("face selected + hovered", vec![top.clone()], Some(top.clone())),
        ("part hovered", vec![], Some(part.clone())),
        ("part selected", vec![part.clone()], None),
        ("face + part selected", vec![top.clone(), part.clone()], None),
    ];
    let mut out = Vec::new();
    for (name, sel, hover) in states {
        let scene = builder::with_highlights(&built, &display, &sel, hover.as_ref(), &[], Vec::new(), Default::default(), &up);
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

/// Prints a failed tool body's top, front and side colours over the background (cargo test -p noshape-scene --test all
/// edge_looks::print_failed_tool -- --ignored --nocapture).
#[test]
#[ignore = "T2: prints the failed tool look"]
fn print_failed_tool() {
    let Ok(mut r) = Renderer::new() else { return };
    let mut display = block();
    let c = noshape_scene::palette::lin(noshape_scene::palette::viewport::TOOL_PREVIEW_FAIL);
    for alpha in [0.9f32, 1.0] {
        let b = &mut display.bodies[0];
        b.color = [c[0], c[1], c[2], alpha];
        b.mesh = std::sync::Arc::new(noshape_model::display::MeshBuffers { edges: Vec::new(), vertices: Vec::new(), ..(*b.mesh).clone() });
        let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
        let up = r.uploader();
        let (w, h) = (800u32, 600u32);
        let mut cache = GpuCache::default();
        let built = builder::build(&display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() }, &mut cache, &up);
        let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap();
        let mut cam = noshape_render::Camera { rotation: noshape_render::Camera::look_rotation(glam::DVec3::new(-1.0, 1.0, -1.0).normalize(), glam::DVec3::new(-1.0, 1.0, 2.0).normalize()), ..Default::default() };
        cam.fit_box(lo, hi, w as f64 / h as f64);
        let scene = builder::with_highlights(&built, &display, &[], None, &[], Vec::new(), Default::default(), &up);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
        let at = |p: glam::DVec3| {
            let q = cam.project(p, glam::DVec2::new(w as f64, h as f64), 1.0).unwrap();
            let o = ((q.y as u32 * w + q.x as u32) * 4) as usize;
            hex([f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]])
        };
        let t = THICK as f64;
        println!("alpha {alpha}: top {} front {} side {} background {}", at(glam::DVec3::new(0.15, 0.1, t)), at(glam::DVec3::new(0.15, 0.0, t / 2.0)), at(glam::DVec3::new(0.3, 0.1, t / 2.0)), hex([f.bgra[(10 * w as usize + 10) * 4 + 2], f.bgra[(10 * w as usize + 10) * 4 + 1], f.bgra[(10 * w as usize + 10) * 4]]));
    }
}

/// Prints the profiles (cargo test -p noshape-scene --test all edge_looks:: -- --ignored --nocapture).
#[test]
#[ignore]
fn print_edge_profiles() {
    for dpr in [1.0f32, 2.0] {
        let k = dpr as u32;
        let Some(p) = profiles(dpr, 8 * k, 30 * k) else { return };
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
/// is lit orange with a #FFA90A boundary; a plain edge is a 1 CSS px dark line.
#[test]
fn edge_and_face_highlights_look_as_live() {
    const HOVER: [u8; 3] = [0xFF, 0xC6, 0x85];
    const SELECT: [u8; 3] = [0xFF, 0xA9, 0x0A];
    for dpr in [1.0f32, 2.0] {
        let k = dpr as usize;
        // (Down the front face to the bottom silhouette and the background under it.)
        let before = 8 * k;
        let Some(p) = profiles(dpr, before as u32, 30 * k as u32) else { return };
        let get = |name: &str| p.iter().find(|(n, _)| *n == name).map(|(_, c)| c.clone()).unwrap();
        let rest = get("rest");
        let (top, side) = (rest[0], rest[before + 4 * k]);
        let show = |c: &[[u8; 3]]| c.iter().map(|&x| hex(x)).collect::<Vec<_>>().join(" ");
        // The plain edge: dark at its middle, about 1 CSS px (live look.json: at 1× its darkest pixel ~L 60, it
        // straddles two rows).
        let edge_rows = &rest[..before + 4 * k];
        let dark = edge_rows.iter().filter(|c| c.iter().all(|&v| v < 0x60)).count();
        assert!(edge_rows.iter().any(|c| c.iter().all(|&v| v < 0x48)) && dark <= k + 1, "{dpr}x plain edge: {}", show(edge_rows));
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
        // A whole part hovered (its Parts-list row): no tint and its own edges as at rest; its silhouette outline
        // 2 device px of #FFC685 at any ratio (live's overlay: the ±1 device px neighbours, unscaled).
        let ph = get("part hovered");
        let outline = ph.iter().filter(|&&c| c == HOVER).count();
        assert!(ph[..before + 2 * k] == rest[..before + 2 * k] && outline == 2, "{dpr}x part hovered ({outline} outline px): {}", show(&ph));
        // A whole part selected: lit #FFC04C with no specular, then #FFC04C over it at 50 % (live 10570.glsl /
        // 94943.glsl: top 0.95 × #FFC04C, the shaded front 0.81 × #FFC04C here).
        let ps = get("part selected");
        let lit = |c: [u8; 3]| (c[2] as f32 - c[0] as f32) / 80.0;
        let want = |f: f32| [255.0f32, 192.0, 76.0].map(|v| (v * (0.5 * f + 0.5)).round() as u8);
        let (front_rest, front_sel) = (rest[before + 4 * k], ps[before + 4 * k]);
        assert!(close(ps[0], want(lit(top)), 4) && close(front_sel, want(lit(front_rest)), 4), "{dpr}x part selected: top {} (live {}), front {} (live {})", hex(ps[0]), hex(want(lit(top))), hex(front_sel), hex(want(lit(front_rest))));
        // live2 hl-pixels-1118: its internal top-front edge stays the plain dark edge under the tint (9D772F / 806026),
        // never a face border; its silhouette a #FFC04C outline of 2 device px at any ratio.
        const PART: [u8; 3] = [0xFF, 0xC0, 0x4C];
        let edge = &ps[before - k..before + 2 * k];
        let dark = if k == 1 { [0x9D, 0x77, 0x2F] } else { [0x80, 0x60, 0x26] };
        assert!(edge.iter().any(|&c| close(c, dark, 8)) && !edge.contains(&SELECT), "{dpr}x part selected: internal edge {} (live's darkest {})", show(edge), hex(dark));
        let outline = ps.iter().filter(|&&c| c == PART).count();
        assert_eq!(outline, 2, "{dpr}x part selected: silhouette outline px: {}", show(&ps));
        // A face selected with its part: the face's colour and its #FFA90A border; the part's silhouette as above.
        let fp = get("face + part selected");
        assert!(close(fp[0], [0xE5, 0xA2, 0x22], 4) && fp[before - k..before + 2 * k].contains(&SELECT) && fp.iter().filter(|&&c| c == PART).count() == 2, "{dpr}x face + part selected: {}", show(&fp));
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
    let scene = builder::with_highlights(&built, &display, std::slice::from_ref(&edge), None, std::slice::from_ref(&top), Vec::new(), builder::Highlighting { errors: std::slice::from_ref(&edge), ..Default::default() }, &up);
    let flags = |r: &Reference| built.picks.ids_for(r).iter().map(|&id| scene.states[id as usize]).collect::<Vec<u32>>();
    assert!(flags(&edge).iter().all(|f| f & state::ERROR != 0), "the failing feature's input is red: {:?}", flags(&edge));
    assert!(flags(&top).iter().all(|f| f & state::ACCENT != 0 && f & state::ERROR == 0), "the hovered part's face isn't: {:?}", flags(&top));
}

/// qa SM-TABLE-BEND-LIT-LOOK (live workflows-1123 02 s19): a Sheet metal table row hovered lights its bend's faces as
/// hovered in the graphics (the pale orange fill and outline), not in the accent's amber that darkened the bend's
/// underside to brown.
#[test]
fn a_hovered_table_rows_faces_take_the_hover_look() {
    let r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    use noshape_render::scene::state;
    let display = block();
    let (_, top) = top_front(&display);
    let up = r.uploader();
    let mut cache = GpuCache::default();
    let built = builder::build(&display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() }, &mut cache, &up);
    let flags = |faces_hovered: bool| {
        let scene = builder::with_highlights(&built, &display, &[], None, std::slice::from_ref(&top), Vec::new(), builder::Highlighting { faces_hovered, ..Default::default() }, &up);
        built.picks.ids_for(&top).iter().map(|&id| scene.states[id as usize]).collect::<Vec<u32>>()
    };
    let (row, other) = (flags(true), flags(false));
    assert!(!row.is_empty() && row.iter().all(|f| f & (state::HOVER | state::FILLED) == state::HOVER | state::FILLED && f & state::ACCENT == 0), "a row's face, filled: {row:?}");
    assert!(other.iter().all(|f| f & state::ACCENT != 0 && f & state::HOVER == 0), "other accents keep theirs: {other:?}");
}

/// qa FEEL-APPEAR-FACE-SELECT-FILL (live appearance-1123 q1-face-appearance-dialog): with a face appearance editor open
/// its faces keep their own colour (no selected fill), their boundary the selection's 2 px line; selected otherwise, filled.
#[test]
fn an_appearance_editors_faces_are_outlined_not_filled() {
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
    let flags = |untinted_faces: bool| {
        let scene = builder::with_highlights(&built, &display, std::slice::from_ref(&top), None, &[], Vec::new(), builder::Highlighting { untinted_faces, ..Default::default() }, &up);
        let of = |r: &Reference| built.picks.ids_for(r).iter().map(|&id| scene.states[id as usize]).collect::<Vec<u32>>();
        (of(&top), of(&edge))
    };
    let ((face, boundary), (plain, _)) = (flags(true), flags(false));
    assert!(!face.is_empty() && face.iter().all(|f| f & state::SELECTED == 0), "the editor's face unfilled: {face:?}");
    assert!(boundary.iter().all(|f| f & (state::SELECTED | state::OUTLINE) == state::SELECTED | state::OUTLINE), "its boundary outlined: {boundary:?}");
    assert!(plain.iter().all(|f| f & state::SELECTED != 0), "selected otherwise: {plain:?}");
}

/// A rebuild reuses each unchanged body's pick targets (the cache keyed by its mesh): the same ids for the same
/// references; a body whose names changed over the same mesh (an earlier feature renamed) gets its new names.
#[test]
fn a_rebuild_keeps_the_pick_ids_and_follows_renamed_faces() {
    let r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let display = block();
    let (edge, top) = top_front(&display);
    let up = r.uploader();
    let mut cache = GpuCache::default();
    let opts = SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: true, show_origin: true, show_sketches: true, ..Default::default() };
    let first = builder::build(&display, &opts, &mut cache, &up);
    let again = builder::build(&display, &opts, &mut cache, &up);
    assert_eq!(first.picks.len(), again.picks.len());
    for id in 1..=first.picks.len() as u32 {
        let (a, b) = (first.picks.get(id).unwrap(), again.picks.get(id).unwrap());
        assert!(a.reference == b.reference && a.kind == b.kind && a.anchor == b.anchor, "pick {id}: {a:?} vs {b:?}");
    }
    assert!(first.picks.get(first.picks.len() as u32 + 1).is_none());
    for r in [&edge, &top, &Reference::Origin, &Reference::Plane(DatumPlane::Top)] {
        assert_eq!(first.picks.ids_for(r).len(), 1, "{r:?}");
        assert_eq!(first.picks.ids_for(r), again.picks.ids_for(r), "{r:?}");
    }
    // The same mesh with two faces' names swapped.
    let mut renamed = display.clone();
    let b = &mut renamed.bodies[0];
    let ti = b.face_refs.iter().position(|f| *f == top).unwrap();
    let other = (ti + 1) % b.face_refs.len();
    b.face_refs.swap(ti, other);
    let swapped = builder::build(&renamed, &opts, &mut cache, &up);
    assert_eq!(swapped.picks.ids_for(&top), first.picks.ids_for(&display.bodies[0].face_refs[other]));
    assert_eq!(swapped.picks.get(swapped.picks.ids_for(&top)[0]).map(|t| &t.reference), Some(&top));
}

/// Live sel-feel (P0): a selected sketch region (outside sketch mode) is a pale fill, #CD9D3C at 34 % over what's
/// under it, inside a #FFA90A outline; hovered, only the outline (#FFC685), no fill.
#[test]
fn a_selected_sketch_region_is_a_pale_fill_inside_an_orange_outline() {
    const HOVER: [u8; 3] = [0xFF, 0xC6, 0x85];
    const SELECT: [u8; 3] = [0xFF, 0xA9, 0x0A];
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // A 300 × 200 rectangle on Top, not extruded.
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let pts = [DVec2::new(0.0, 0.0), DVec2::new(0.3, 0.0), DVec2::new(0.3, 0.2), DVec2::new(0.0, 0.2)];
    for i in 0..4 {
        sk.add_line(pts[i], pts[(i + 1) % 4]);
    }
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let display = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default()).display;
    let region = Reference::SketchRegion { sketch: sid, key: display.sketches[0].regions[0].key.clone() };
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let up = r.uploader();
    let (w, h) = (800u32, 600u32);
    let mut cache = GpuCache::default();
    let built = builder::build(&display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, ..Default::default() }, &mut cache, &up);
    let mut cam = noshape_render::Camera::default();
    cam.set_view(noshape_render::StandardView::Top);
    cam.fit_box(glam::DVec3::new(-0.05, -0.05, 0.0), glam::DVec3::new(0.35, 0.25, 0.0), w as f64 / h as f64);
    // A pixel column at the rectangle's middle, across its bottom edge (y = 0).
    let size = glam::DVec2::new(w as f64, h as f64);
    let x = cam.project(glam::DVec3::new(0.15, 0.1, 0.0), size, 1.0).unwrap().x as u32;
    let (centre, edge) = (cam.project(glam::DVec3::new(0.15, 0.1, 0.0), size, 1.0).unwrap().y as u32, cam.project(glam::DVec3::new(0.15, 0.0, 0.0), size, 1.0).unwrap().y as u32);
    let mut column = |sel: &[Reference], hover: Option<&Reference>| {
        let scene = builder::with_highlights(&built, &display, sel, hover, &[], Vec::new(), Default::default(), &up);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
        let px = |y: u32| {
            let o = ((y * w + x) * 4) as usize;
            [f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]]
        };
        (px(centre), (edge - 4..=edge + 4).map(px).collect::<Vec<_>>())
    };
    let (rest, rest_edge) = column(&[], None);
    let (sel, sel_edge) = column(std::slice::from_ref(&region), None);
    let (hov, hov_edge) = column(&[], Some(&region));
    let want = mix([0xCD, 0x9D, 0x3C], rest, 0.34);
    assert!(close(sel, want, 6), "selected fill {} (#CD9D3C at 34 % over {}: {})", hex(sel), hex(rest), hex(want));
    assert!(sel.iter().all(|&c| c >= 0xB0), "a pale fill: {}", hex(sel));
    assert!(sel_edge.contains(&SELECT), "the orange outline: {:?}", sel_edge.iter().map(|&c| hex(c)).collect::<Vec<_>>());
    assert!(hov == rest && hov_edge.contains(&HOVER), "hovered: no fill ({} vs {}), the light outline: {:?} (rest {:?})", hex(hov), hex(rest), hov_edge.iter().map(|&c| hex(c)).collect::<Vec<_>>(), rest_edge.iter().map(|&c| hex(c)).collect::<Vec<_>>());
}

/// Live mc-reach-1122 R5: a hovered circular edge is drawn whole, its back half hidden behind the part included (an
/// Ø20 × 20 cylinder in the isometric view, its bottom rim hovered).
#[test]
fn a_hovered_rim_shows_its_hidden_back_half() {
    const HOVER: [u8; 3] = [0xFF, 0xC6, 0x85];
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.01);
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let regen = |ps: &PartStudio| Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("20 mm")));
    ps.features.push(ex);
    let display = regen(&ps).display;
    let b = &display.bodies[0];
    let rim = b.edge_refs.iter().enumerate().find(|(i, _)| b.mesh.edges[*i].len() > 2 && b.mesh.edges[*i].iter().all(|p| p[2].abs() < 1e-6)).map(|(_, e)| e.clone()).expect("the bottom rim");
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let up = r.uploader();
    let (w, h) = (400u32, 400u32);
    let built = builder::build(&display, &SceneOptions { display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, ..Default::default() }, &mut GpuCache::default(), &up);
    let mut cam = noshape_render::Camera::default();
    cam.set_view(noshape_render::StandardView::Isometric);
    cam.fit_box(glam::DVec3::new(-0.01, -0.01, 0.0), glam::DVec3::new(0.01, 0.01, 0.02), w as f64 / h as f64);
    // The rim's far point, behind the cylinder: the eye looks along (-1, 1, -1).
    let back = cam.project(glam::DVec3::new(-0.01, 0.01, 0.0) * std::f64::consts::FRAC_1_SQRT_2, glam::DVec2::new(w as f64, h as f64), 1.0).unwrap();
    let mut lit_behind = |hover: Option<&Reference>| {
        let scene = builder::with_highlights(&built, &display, &[], hover, &[], Vec::new(), Default::default(), &up);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
        let (x, y) = (back.x.round() as i32, back.y.round() as i32);
        (-2..=2).flat_map(|dy| (-2..=2).map(move |dx| (x + dx, y + dy))).any(|(x, y)| {
            let o = ((y as u32 * w + x as u32) * 4) as usize;
            close([f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]], HOVER, 24)
        })
    };
    assert!(!lit_behind(None), "at rest: the back half hidden");
    assert!(lit_behind(Some(&rim)), "hovered: the back half drawn over the cylinder");
}
