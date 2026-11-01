//! Regeneration profile (lead, 10-23): the slowest features of the big test
//! models (tests/perf fixtures) and of the std sheet-metal route, each Part
//! Studio regenerated from scratch as when a document opens. Run in release:
//! `cargo test --release -p noshape-ops --test all regen_profile:: -- --ignored --nocapture`
//! (perf.yml `model` job). Prints every feature's time and the top 5 overall
//! as `profile:` lines.

use glam::DVec3;
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabContent;
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::time::Instant;

/// (model, feature, type, ms)
type Row = (String, String, String, f64);

fn profile(model: &str, ps: &PartStudio, units: &UnitPrefs, rows: &mut Vec<Row>) -> RegenResult {
    profile_with(model, ps, units, rows, &RegenOptions::default())
}

/// [`profile`] with `opts` (a document's Feature Studios for its custom features).
fn profile_with(model: &str, ps: &PartStudio, units: &UnitPrefs, rows: &mut Vec<Row>, opts: &RegenOptions) -> RegenResult {
    let t = Instant::now();
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), units, opts);
    let total = t.elapsed().as_secs_f64() * 1e3;
    let d = &r.display;
    eprintln!("profile: {model}: {total:.0} ms total, features {:.0} ms, display {:.0} ms, {} features", d.regen_ms, total - d.regen_ms as f64, ps.features.len());
    for (f, s) in ps.features.iter().zip(&d.features) {
        rows.push((model.to_string(), f.name.clone(), f.type_id.clone(), s.time_ms as f64));
        if s.status != noshape_model::display::FeatureStatus::Ok {
            eprintln!("profile: {model}: {} {:?} {}", f.name, s.status, s.message.as_deref().unwrap_or(""));
        }
    }
    rows.push((model.to_string(), "(display)".into(), "display".into(), total - d.regen_ms as f64));
    r
}

fn fixtures(rows: &mut Vec<Row>) {
    // NOSHAPE_PROFILE_DIR=<dir>: its documents instead of tests/perf's.
    let dir = std::env::var_os("NOSHAPE_PROFILE_DIR").map_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf"), std::path::PathBuf::from);
    let tmp = std::env::temp_dir().join(format!("noshape-profile-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape")).collect();
    files.sort();
    // NOSHAPE_PROFILE_ONLY=<part of a file name>: those fixtures only.
    if let Ok(only) = std::env::var("NOSHAPE_PROFILE_ONLY") {
        files.retain(|p| p.to_string_lossy().contains(&only));
    }
    for path in files {
        let copy = tmp.join(path.file_name().unwrap());
        std::fs::copy(&path, &copy).unwrap();
        let s = noshape_ops::session::Session::open(&copy).unwrap();
        let units = s.doc.settings.units;
        // Its custom features (the printable knurl) from its own Feature Studios.
        let opts = RegenOptions { custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&s.doc)), ..Default::default() };
        for tab in &s.doc.tabs {
            if let TabContent::PartStudio(ps) = &tab.content {
                let name = format!("{} / {}", path.file_stem().unwrap().to_string_lossy(), tab.name);
                let r = profile_with(&name, ps, &units, rows, &opts);
                // NOSHAPE_PROFILE_BREP=<dir>: the regenerated solids written there (<file> <tab> <n>.brep), for kernel probes.
                if let Some(out) = std::env::var_os("NOSHAPE_PROFILE_BREP") {
                    for (i, b) in r.context.bodies.iter().enumerate() {
                        let _ = noshape_kernel::write_brep(&b.shape, &std::path::Path::new(&out).join(format!("{} {} {i}.brep", path.file_stem().unwrap().to_string_lossy(), tab.name)));
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

fn add(ps: &mut PartStudio, units: &UnitPrefs, ty: &str, set: impl FnOnce(&mut ParamValues)) -> FeatureId {
    let mut f = ps.new_feature(ty, units).unwrap_or_else(|| panic!("unknown feature {ty}"));
    set(&mut f.params);
    let id = f.id;
    ps.features.push(f);
    id
}

fn edge_at(r: &RegenResult, p: DVec3) -> Reference {
    let b = r.context.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("a part");
    for (i, e) in b.shape.edges().iter().enumerate() {
        let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
        if noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9) {
            return Reference::Edge((*b.edges[i]).clone());
        }
    }
    panic!("no edge at {p:?}");
}

/// The std sheet-metal route: a 200 × 150 plate 2 mm (bend R2), 30 mm flanges
/// on its four top edges, a hem on one flange.
fn sheet_metal(rows: &mut Vec<Row>) {
    noshape_model::custom::std_feature::set_std_sheet_metal(true);
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (200.0, 0.0), (200.0, 150.0), (0.0, 150.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(x * 1e-3, y * 1e-3)));
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
    let sketch = add(&mut ps, &units, "sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = r.display.sketches.iter().find(|s| s.feature == sketch).unwrap().regions[0].key.clone();
    add(&mut ps, &units, "sheetMetalStart", |p| {
        p.set("process", ParamValue::Enum("THICKEN".into()));
        p.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("thickness", ParamValue::Expr(ExprValue::new("2 mm")));
        p.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    });
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let edges: Vec<Reference> = [(100.0, 0.0), (200.0, 75.0), (100.0, 150.0), (0.0, 75.0)].iter().map(|&(x, y)| edge_at(&r, DVec3::new(x, y, 2.0))).collect();
    add(&mut ps, &units, "sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", ParamValue::Expr(ExprValue::new("30 mm")));
    });
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    // The front flange's top edge (outer face y = -2..., z = 2 + 30 + radius): the edge nearest its middle top.
    let b = r.context.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("a part");
    let top = b.shape.edges().iter().enumerate().filter_map(|(i, e)| e.edge_info().ok().map(|info| (i, info))).filter(|(_, i)| i.start.y < 1e-3 && i.end.y < 1e-3 && (i.start.x - i.end.x).abs() > 0.1).max_by(|a, b| (a.1.start.z + a.1.end.z).total_cmp(&(b.1.start.z + b.1.end.z))).map(|(i, _)| Reference::Edge((*b.edges[i]).clone()));
    if let Some(top) = top {
        add(&mut ps, &units, "sheetMetalHem", |p| {
            p.set("edges", ParamValue::Query(vec![top]));
        });
    }
    profile("sheet metal (std): plate 200x150 + 4 flanges + hem", &ps, &units, rows);
    // Again, std's Hem module loaded (the first run includes loading it).
    let t = Instant::now();
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let times: Vec<String> = ps.features.iter().zip(&r.display.features).map(|(f, s)| format!("{} {:.0} ms", f.name, s.time_ms)).collect();
    eprintln!("profile: sheet metal (std) warm: {:.0} ms total; {}", t.elapsed().as_secs_f64() * 1e3, times.join(", "));
}

#[test]
#[ignore]
fn regen_profile() {
    let mut rows: Vec<Row> = Vec::new();
    fixtures(&mut rows);
    if std::env::var_os("NOSHAPE_PROFILE_ONLY").is_none() {
        sheet_metal(&mut rows);
    }
    rows.sort_by(|a, b| b.3.total_cmp(&a.3));
    for (i, (model, name, ty, ms)) in rows.iter().take(15).enumerate() {
        eprintln!("profile: #{} {ms:.1} ms  {name} ({ty})  in {model}", i + 1);
    }
}

/// The user's knurl (P0 11-01, v0.4.19): tests/perf/knurl-d25x25-45-45.noshape, the B94.6 Diamond knurl (12 TPI,
/// depth 100 %, tooth 45°, helix 45°) on a Ø25 × 25 cylinder, built through the CLI. Opened and regenerated as the
/// app and the CLI do (its Feature Studio's custom feature, then the display meshes, which the regeneration makes),
/// 3 runs. noshape's Boolean declined its subtract at this height (two grooves crossing 44 µm above the end face)
/// and OCCT's took minutes in the app; live: 16.8 s, 11597.649 mm³, 4479 faces (behaviours request 1743).
#[test]
#[ignore]
fn timed_knurl_d25() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/knurl-d25x25-45-45.noshape");
    let mut runs = Vec::new();
    for i in 0..3 {
        let tmp = std::env::temp_dir().join(format!("noshape-knurl-d25-{}-{i}.noshape", std::process::id()));
        std::fs::copy(&path, &tmp).unwrap();
        let t = Instant::now();
        let s = noshape_ops::session::Session::open(&tmp).unwrap();
        let units = s.doc.settings.units;
        let opts = RegenOptions { custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&s.doc)), ..Default::default() };
        let ps = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
        let secs = t.elapsed().as_secs_f64();
        let _ = std::fs::remove_file(&tmp);
        let bad: Vec<String> = ps.features.iter().zip(&r.display.features).filter(|(_, st)| st.status != noshape_model::display::FeatureStatus::Ok).map(|(f, st)| format!("{} {:?} {:?}", f.name, st.status, st.message)).collect();
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(r.context.bodies.len(), 1);
        let b = &r.context.bodies[0];
        let (v, faces) = (b.shape.volume().unwrap() * 1e9, b.shape.count(noshape_kernel::ShapeType::Face));
        // (The regeneration's time includes the display meshes, made after the features.)
        let features: f64 = r.display.features.iter().map(|st| st.time_ms as f64 / 1e3).sum();
        eprintln!("timed knurl Ø25 × 25 45/45 run {}: {secs:.2} s (features {features:.2} s, the rest (opening, display meshes) {:.2} s), {v:.3} mm³, {faces} faces", i + 1, secs - features);
        // (At a 45° helix our groove tools are within 7e-5 of live's: live's sweep's own error.)
        assert_eq!(faces, 4479);
        assert!((v - 11597.648791535885).abs() < 7e-5 * v, "{v} mm³, live 11597.649");
        runs.push(secs);
    }
    runs.sort_by(|a, b| a.total_cmp(b));
    eprintln!("timed knurl Ø25 × 25 45/45: median {:.2} s, best {:.2} s (live 16.8 s)", runs[1], runs[0]);
    // (OCCT's subtract, where noshape's Boolean declines, takes minutes.)
    assert!(runs[1] < 60.0, "median {:.1} s", runs[1]);
}

/// The sheet metal model alone (cold: the Hem's std module loads; then
/// warm), for `tools/tiers/perf.sh -p noshape-ops --test all
/// regen_profile::sheet_metal_profile -- --ignored --nocapture`.
#[test]
#[ignore]
fn sheet_metal_profile() {
    sheet_metal(&mut Vec::new());
}

/// REGEN-SPEED (core): live's 30-feature part, Extrude 1's depth edited
/// 10 → 12 and committed, as the app does (the regenerator's cache keeps the
/// features before the edit). Prints each re-evaluated feature's time.
/// `cargo test --release -p noshape-ops --test all regen_profile::edit_commit_profile -- --ignored --nocapture`
#[test]
#[ignore]
fn edit_commit_profile() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    let tmp = std::env::temp_dir().join(format!("noshape-edit-profile-{}.noshape", std::process::id()));
    std::fs::copy(dir.join("live-30-features.noshape"), &tmp).unwrap();
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    let ps0 = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
    let opts = RegenOptions::default();
    // Five commits, each on a fresh regenerator that has the part as it was
    // (the features before Extrude 1 cached): medians per feature.
    let mut per: std::collections::BTreeMap<String, Vec<f32>> = Default::default();
    let mut totals = Vec::new();
    for _ in 0..5 {
        let mut reg = Regenerator::new();
        let _ = reg.regenerate(&ps0, &ConfigValues::default(), &units, &opts);
        let mut ps = ps0.clone();
        let ex = ps.features.iter_mut().find(|f| f.name == "Extrude 1").unwrap();
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("12 mm")));
        let t = Instant::now();
        let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &opts);
        totals.push(t.elapsed().as_secs_f64() * 1e3);
        for (f, s) in ps.features.iter().zip(&r.display.features) {
            per.entry(f.name.clone()).or_default().push(s.time_ms);
        }
    }
    let median = |v: &mut Vec<f32>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    totals.sort_by(|a, b| a.total_cmp(b));
    let mut rows: Vec<(String, f32)> = per.into_iter().map(|(n, mut v)| (n, median(&mut v))).collect();
    let sum: f32 = rows.iter().map(|r| r.1).sum();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    eprintln!("edit commit (depth 10 → 12), median of 5: {:.0} ms total (min {:.0}), features {sum:.0} ms", totals[2], totals[0]);
    for (n, ms) in rows.iter().take(15) {
        eprintln!("  {ms:6.1} ms  {n}");
    }
    let _ = std::fs::remove_file(&tmp);
}

/// qa DRAFT-BUDGET-POOL: the CLI-fuzz body whose Draft 7 ran away (its face
/// deletion's defeaturing never ended, memory climbing) when the CPU budget
/// couldn't stop OCCT pool work (patch 0004's parallel pairs; the first patch
/// 0005, which also held the budget in serial pool loops). Its Drafts must stop
/// at their budget (30 s of CPU each): the document opens in well under the
/// 4 minutes it ran before qa stopped it (green 989f756b: 26 s).
#[test]
#[ignore = "T2: a CLI-fuzz document with budgeted drafts (~30-70 s)"]
fn the_draft_pool_hang_document_opens() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/budget/draft-pool-hang.noshape");
    let tmp = std::env::temp_dir().join(format!("noshape-draft-pool-{}.noshape", std::process::id()));
    std::fs::copy(&path, &tmp).unwrap();
    let t = Instant::now();
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    let mut slow = Vec::new();
    for tab in &s.doc.tabs {
        if let TabContent::PartStudio(ps) = &tab.content {
            let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
            for (f, st) in ps.features.iter().zip(&r.display.features) {
                if st.time_ms > 5000.0 {
                    slow.push(format!("{} {:.0} ms {:?} {:?}", f.name, st.time_ms, st.status, st.message));
                }
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    let secs = t.elapsed().as_secs_f64();
    eprintln!("draft pool hang document: opened and regenerated in {secs:.0} s; slow: {slow:?}");
    assert!(secs < 180.0, "{secs:.0} s; slow: {slow:?}");
}

/// BLEND-RUST (core): the big test models regenerated from scratch, OCCT's blends against noshape's own
/// (noshape_kernel::set_rust_blend), and live-30's edit commit both ways: medians of 5, and each model's fillets and
/// chamfers summed. `tools/tiers/perf.sh -p noshape-ops --test all -- regen_profile::blend_rust_profile --ignored --nocapture --test-threads=1`
#[test]
#[ignore]
fn blend_rust_profile() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    for model in ["live-30-features.noshape", "big-plate.noshape", "fins-400.noshape"] {
        let tmp = std::env::temp_dir().join(format!("noshape-blend-profile-{}-{model}", std::process::id()));
        std::fs::copy(dir.join(model), &tmp).unwrap();
        let s = noshape_ops::session::Session::open(&tmp).unwrap();
        let units = s.doc.settings.units;
        let ps = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
        let opts = RegenOptions::default();
        for rust in [false, true] {
            noshape_kernel::set_rust_blend(Some(rust));
            let (mut totals, mut blends) = (Vec::new(), Vec::new());
            let mut volume = 0.0;
            for _ in 0..5 {
                let t = Instant::now();
                let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
                totals.push(t.elapsed().as_secs_f64() * 1e3);
                blends.push(ps.features.iter().zip(&r.display.features).filter(|(f, _)| matches!(f.type_id.as_str(), "fillet" | "chamfer")).map(|(_, s)| s.time_ms as f64).sum::<f64>());
                volume = r.context.bodies.iter().filter_map(|b| b.shape.volume().ok()).sum();
            }
            totals.sort_by(|a, b| a.total_cmp(b));
            blends.sort_by(|a, b| a.total_cmp(b));
            eprintln!("blend profile: {model} {}: regen median {:.1} ms (min {:.1}), fillets + chamfers {:.1} ms, volume {volume:.12e}", if rust { "noshape" } else { "OCCT" }, totals[2], totals[0], blends[2]);
        }
        noshape_kernel::set_rust_blend(None);
        let _ = std::fs::remove_file(&tmp);
    }
    // live-30's edit commit (Extrude 1's depth 10 → 12; the features before it cached).
    let tmp = std::env::temp_dir().join(format!("noshape-blend-edit-{}.noshape", std::process::id()));
    std::fs::copy(dir.join("live-30-features.noshape"), &tmp).unwrap();
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    let ps0 = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
    let opts = RegenOptions::default();
    for rust in [false, true] {
        noshape_kernel::set_rust_blend(Some(rust));
        let mut totals = Vec::new();
        for _ in 0..5 {
            let mut reg = Regenerator::new();
            let _ = reg.regenerate(&ps0, &ConfigValues::default(), &units, &opts);
            let mut ps = ps0.clone();
            let ex = ps.features.iter_mut().find(|f| f.name == "Extrude 1").unwrap();
            ex.params.set("depth", ParamValue::Expr(ExprValue::new("12 mm")));
            let t = Instant::now();
            let _ = reg.regenerate(&ps, &ConfigValues::default(), &units, &opts);
            totals.push(t.elapsed().as_secs_f64() * 1e3);
        }
        totals.sort_by(|a, b| a.total_cmp(b));
        eprintln!("blend profile: live-30 edit commit {}: median {:.1} ms (min {:.1})", if rust { "noshape" } else { "OCCT" }, totals[2], totals[0]);
    }
    noshape_kernel::set_rust_blend(None);
    let _ = std::fs::remove_file(&tmp);
}

/// live-30-features' display meshes, noshape's mesher against BRepMesh: its parts as a regeneration leaves
/// them, meshed on fresh copies (the mesh cache has none, as on open) at the display's deflections, median of 5
/// each. Cold: each round's copies scaled by 1 + k·10⁻⁶, so the CDT's result cache (a face met again in its own
/// frame) has none of them either, as on open; warm: the same copies again (a regeneration that moved faces). For
/// tools/tiers/perf.sh (no environment needed). `cargo test --release -p noshape-ops --test all
/// regen_profile::bench_live30_display_mesh -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore = "bench"]
fn bench_live30_display_mesh() {
    use noshape_kernel::{ShapeType, Transform, set_rust_mesh, tessellate, transform};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    let tmp = std::env::temp_dir().join(format!("noshape-mesh-bench-{}.noshape", std::process::id()));
    std::fs::copy(dir.join("live-30-features.noshape"), &tmp).unwrap();
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    let ps = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let parts: Vec<noshape_kernel::Shape> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| b.shape.clone()).collect();
    let faces: usize = parts.iter().map(|p| p.faces().len()).sum();
    let round = std::cell::Cell::new(0);
    let times = |rust: bool, cold: bool| {
        let mut v: Vec<f64> = (0..5)
            .map(|_| {
                round.set(round.get() + 1);
                let to = if cold { Transform::Scale { center: DVec3::ZERO, factor: 1.0 + round.get() as f64 * 1e-6 } } else { Transform::Translate(DVec3::ZERO) };
                let copies: Vec<_> = parts.iter().map(|p| transform(p, &to, true).unwrap().shape).collect();
                for c in &copies {
                    let _ = c.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
                }
                set_rust_mesh(rust);
                let t = Instant::now();
                for c in &copies {
                    // The display's angle-based deflections (display.rs display_deflection).
                    tessellate(c, c.bounding_box(false).unwrap().diagonal(), 2.0 * std::f64::consts::TAU / 144.0, true).unwrap();
                }
                set_rust_mesh(true);
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[2]
    };
    let (ours, occt) = (times(true, true), times(false, true));
    eprintln!("live-30 ({} parts, {faces} faces) display mesh, cold: ours {ours:.1} ms, BRepMesh {occt:.1} ms: {:.1}×", parts.len(), occt / ours);
    let (ours, occt) = (times(true, false), times(false, false));
    eprintln!("live-30 display mesh, warm: ours {ours:.1} ms, BRepMesh {occt:.1} ms: {:.1}×", occt / ours);
    let _ = std::fs::remove_file(&tmp);
}

/// live-30's display mesh by step (the kernel profile, wall time), ours, cold, three rounds: for
/// tools/tiers/perf.sh. `cargo test --release -p noshape-ops --test all regen_profile::bench_profile_live30 --
/// --ignored --nocapture --test-threads=1`.
#[test]
#[ignore = "bench"]
fn bench_profile_live30_display_mesh() {
    use noshape_kernel::{ShapeType, Transform, kernel_profile_mark, set_kernel_profile, tessellate, transform};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    let tmp = std::env::temp_dir().join(format!("noshape-mesh-profile-{}.noshape", std::process::id()));
    std::fs::copy(dir.join("live-30-features.noshape"), &tmp).unwrap();
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    let ps = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let parts: Vec<noshape_kernel::Shape> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| b.shape.clone()).collect();
    let copies: Vec<_> = (1..=3)
        .flat_map(|k| parts.iter().map(move |p| transform(p, &Transform::Scale { center: DVec3::ZERO, factor: 1.0 + k as f64 * 1e-6 }, true).unwrap().shape))
        .collect();
    for c in &copies {
        let _ = c.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
    }
    set_kernel_profile(true);
    kernel_profile_mark("");
    let t = Instant::now();
    for c in &copies {
        tessellate(c, c.bounding_box(false).unwrap().diagonal(), 2.0 * std::f64::consts::TAU / 144.0, true).unwrap();
    }
    eprintln!("live-30, ours, cold: {:.2} ms each (profiled)", t.elapsed().as_secs_f64() * 1e3 / 3.0);
    kernel_profile_mark("ours, three rounds");
    let _ = std::fs::remove_file(&tmp);
}

/// The profile documents (core 11-01: the perf fixtures and qa's documents, ~/agents/profdocs on the perf hosts, else
/// NOSHAPE_PROFILE_DIR) regenerated once each, one line a document: its total and its slowest feature.
/// `tools/tiers/perf.sh -p noshape-ops --test all -- regen_profile::bench_profdocs --ignored --nocapture --test-threads=1`
#[test]
#[ignore = "T2 bench: needs ~/agents/profdocs"]
fn bench_profdocs() {
    let dir = std::env::var_os("NOSHAPE_PROFILE_DIR").map_or_else(|| std::path::PathBuf::from(format!("{}/agents/profdocs", std::env::var("HOME").unwrap_or_default())), std::path::PathBuf::from);
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("profile documents").filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape")).collect();
    files.sort();
    let tmp = std::env::temp_dir().join(format!("noshape-profdocs-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for path in files {
        let copy = tmp.join(path.file_name().unwrap());
        std::fs::copy(&path, &copy).unwrap();
        let s = noshape_ops::session::Session::open(&copy).unwrap();
        let units = s.doc.settings.units;
        let (mut total, mut slow) = (0.0, (String::new(), 0.0));
        for tab in &s.doc.tabs {
            if let TabContent::PartStudio(ps) = &tab.content {
                let t = Instant::now();
                let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
                total += t.elapsed().as_secs_f64() * 1e3;
                for (f, st) in ps.features.iter().zip(&r.display.features) {
                    if st.time_ms as f64 > slow.1 {
                        slow = (f.name.clone(), st.time_ms as f64);
                    }
                }
            }
        }
        println!("profdoc {}: {total:.0} ms (slowest {} {:.0} ms)", path.file_stem().unwrap().to_string_lossy(), slow.0, slow.1);
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Each document of NOSHAPE_PROFILE_DIR regenerated: a line per part (volume, faces, edges) and per feature not OK,
/// to compare two builds' results (`diff` the outputs).
#[test]
#[ignore = "T2 tool: NOSHAPE_PROFILE_DIR"]
fn regen_summary() {
    let Some(dir) = std::env::var_os("NOSHAPE_PROFILE_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape")).collect();
    files.sort();
    let tmp = std::env::temp_dir().join(format!("noshape-summary-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for path in files {
        let copy = tmp.join(path.file_name().unwrap());
        std::fs::copy(&path, &copy).unwrap();
        let Ok(s) = noshape_ops::session::Session::open(&copy) else {
            println!("{}: does not open", path.display());
            continue;
        };
        let units = s.doc.settings.units;
        for tab in &s.doc.tabs {
            if let TabContent::PartStudio(ps) = &tab.content {
                let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
                let doc = format!("{} / {}", path.file_stem().unwrap().to_string_lossy(), tab.name);
                for (f, st) in ps.features.iter().zip(&r.display.features) {
                    if st.status != noshape_model::display::FeatureStatus::Ok {
                        println!("{doc}: {} {:?} {}", f.name, st.status, st.message.as_deref().unwrap_or(""));
                    }
                }
                for b in &r.context.bodies {
                    println!("{doc}: {} vol {:.6} faces {} edges {}", b.name, b.shape.volume().unwrap_or(0.0) * 1e9, b.shape.faces().len(), b.shape.edges().len());
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
