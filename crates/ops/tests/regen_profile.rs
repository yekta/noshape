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
/// 4 minutes it ran before qa stopped it (green 989f756b: 26 s). Its 26 s were
/// Shell 20's OCCT offset evaluation that never ended, stopped there by Linux's
/// CPU timer alone: on macOS the document never opened (bool, 11-06). With OCCT
/// patch 0023 it ends at once, the document in 4 s; release.yml's "Budgeted
/// documents" opens it on every platform.
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

/// The first feature of each NOSHAPE_PROFILE_DIR document (NOSHAPE_PROFILE_ONLY: one) after which a body has an edge
/// of more than two faces (Parasolid makes no such body: live 1760) or a plane using an edge on both sides (a slit),
/// regenerating each prefix of the feature list.
#[test]
#[ignore = "T2 tool: NOSHAPE_PROFILE_DIR"]
fn non_manifold_origin() {
    let Some(dir) = std::env::var_os("NOSHAPE_PROFILE_DIR") else { return };
    let only = std::env::var("NOSHAPE_PROFILE_ONLY").ok();
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape") && only.as_ref().is_none_or(|o| p.file_name().is_some_and(|n| n.to_string_lossy() == *o))).collect();
    files.sort();
    let tmp = std::env::temp_dir().join(format!("noshape-nm-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for path in files {
        let copy = tmp.join(path.file_name().unwrap());
        std::fs::copy(&path, &copy).unwrap();
        let Ok(s) = noshape_ops::session::Session::open(&copy) else { continue };
        let units = s.doc.settings.units;
        for tab in &s.doc.tabs {
            if let TabContent::PartStudio(ps) = &tab.content {
                for k in 1..=ps.features.len() {
                    let mut p = ps.clone();
                    p.features.truncate(k);
                    let r = Regenerator::new().regenerate(&p, &ConfigValues::default(), &units, &RegenOptions::default());
                    // (Or an edge only a plane uses: a slit, as no plane has a seam.)
                    let bad: Vec<String> = r.context.bodies.iter().filter(|b| {
                        let faces = b.shape.faces();
                        b.shape.edge_faces().is_ok_and(|ef| ef.iter().any(|f| {
                            let mut f = f.clone();
                            f.sort_unstable();
                            f.dedup();
                            f.len() > 2 || (f.len() == 1 && faces.get(f[0] as usize).is_some_and(|x| matches!(x.face_surface(), Ok(noshape_kernel::Surface::Plane(_)))))
                        }))
                    }).map(|b| b.name.clone()).collect();
                    if !bad.is_empty() {
                        let f = &ps.features[k - 1];
                        println!("{} / {}: after feature {k} {} ({}): {bad:?} non-manifold", path.file_stem().unwrap().to_string_lossy(), tab.name, f.name, f.type_id);
                        break;
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// T2 bench: every feature with std's (or a custom feature's) editing logic in the perf documents and NOSHAPE_PROFILE_DIR's
/// (else ~/agents/profdocs), its logic run as a new dialog runs it (on the model just before it), best of 3: the ten
/// slowest, one line each.
#[test]
#[ignore = "T2 bench: tools/tiers/perf.sh"]
fn bench_editing_logic() {
    let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf")];
    dirs.push(std::env::var_os("NOSHAPE_PROFILE_DIR").map_or_else(|| std::path::PathBuf::from(format!("{}/agents/profdocs", std::env::var("HOME").unwrap_or_default())), std::path::PathBuf::from));
    let tmp = std::env::temp_dir().join(format!("noshape-elogic-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut rows: Vec<(f64, String)> = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape")).collect();
        files.sort();
        for path in files {
            let copy = tmp.join(path.file_name().unwrap());
            std::fs::copy(&path, &copy).unwrap();
            let Ok(s) = noshape_ops::session::Session::open(&copy) else { continue };
            let units = s.doc.settings.units;
            let opts = RegenOptions { custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&s.doc)), ..Default::default() };
            for tab in &s.doc.tabs {
                let TabContent::PartStudio(ps) = &tab.content else { continue };
                for (i, f) in ps.features.iter().enumerate() {
                    if f.suppressed || !noshape_model::custom::std_feature::has_editing_logic(&f.type_id) {
                        continue;
                    }
                    let before = RegenOptions { upto: Some(i), ..opts.clone() };
                    let ctx = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &before).context;
                    let specified: Vec<String> = f.params.0.keys().cloned().collect();
                    let best = (0..3)
                        .map(|_| {
                            let t = Instant::now();
                            let _ = noshape_model::custom::std_feature::editing_logic_in((*ctx).clone(), &f.type_id, f.id, &f.name, &units, &Default::default(), &f.params, &specified, true);
                            t.elapsed().as_secs_f64() * 1e3
                        })
                        .fold(f64::INFINITY, f64::min);
                    rows.push((best, format!("{} / {} / {} ({})", path.file_stem().unwrap().to_string_lossy(), tab.name, f.name, f.type_id)));
                }
            }
        }
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("editing logic: {} features, {:.1} ms in all", rows.len(), rows.iter().map(|r| r.0).sum::<f64>());
    for (ms, what) in rows.iter().take(10) {
        println!("timing editing logic {what}: {ms:.1} ms");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// T2 bench: edit latency. Each document of tests/perf, NOSHAPE_PROFILE_DIR (else ~/agents/profdocs) and
/// NOSHAPE_EDIT_DOCS (more directories, `:`-separated) regenerated once (warm, as an open document is), then typical
/// edits each committed and regenerated as the app does (the same Regenerator, preview meshes: what the graphics
/// wait for): a dimension of the first sketch with one, the first extrude's depth, the first fillet's radius, the
/// middle feature suppressed and unsuppressed. One line per edit: the time, how much of it features / display, and
/// how many features ran again of how many.
#[test]
#[ignore = "T2 bench: tools/tiers/perf.sh"]
fn bench_edit_latency() {
    noshape_model::regen::set_reuse(Some(false));
    edit_latency(false);
}

/// [`bench_edit_latency`] with incremental regeneration, each edit's result checked against a full regeneration
/// (no cache, no reuse) of the edited Part Studio: every body (id, name, kind, colour, names, volume, area, faces)
/// and every feature's status and message the same.
#[test]
#[ignore = "T2 bench: tools/tiers/perf.sh"]
fn bench_edit_latency_reuse() {
    noshape_model::regen::set_reuse(Some(true));
    edit_latency(true);
    noshape_model::regen::set_reuse(None);
}

fn edit_latency(check: bool) {
    use noshape_model::params::ParamValue;
    let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf")];
    dirs.push(std::env::var_os("NOSHAPE_PROFILE_DIR").map_or_else(|| std::path::PathBuf::from(format!("{}/agents/profdocs", std::env::var("HOME").unwrap_or_default())), std::path::PathBuf::from));
    if let Ok(more) = std::env::var("NOSHAPE_EDIT_DOCS") {
        dirs.extend(more.split(':').map(std::path::PathBuf::from));
    }
    let only = std::env::var("NOSHAPE_PROFILE_ONLY").ok();
    let tmp = std::env::temp_dir().join(format!("noshape-edits-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // Round edits, as typed: a length one 1 mm more, anything else 1 % more.
    let scaled = |e: &noshape_units::ExprValue| noshape_units::ExprValue::new(if e.text.trim_end().ends_with("mm") { format!("({}) + 1 mm", e.text) } else { format!("({}) * 1.01", e.text) });
    // Each feature's own effect: the bodies it adds or changes (fingerprinted), from the context after each feature
    // (the cached runs up to it).
    type Print = (String, usize, usize, i64, i64);
    let effects = |reg: &mut Regenerator, ps: &noshape_model::partstudio::PartStudio, units: &noshape_units::UnitPrefs, opts: &RegenOptions| -> Vec<Vec<Print>> {
        let print = |b: &noshape_model::regen::context::Body| -> Print {
            let v = b.shape.volume().unwrap_or(0.0) * 1e9;
            let a = noshape_kernel::surface_properties(&b.shape).map_or(0.0, |p| p.mass) * 1e6;
            (b.id.to_string(), b.shape.count(noshape_kernel::ShapeType::Face), b.shape.count(noshape_kernel::ShapeType::Edge), (v * 1e6).round() as i64, (a * 1e6).round() as i64)
        };
        let mut before: Vec<Print> = Vec::new();
        let mut out = Vec::new();
        for k in 0..ps.features.len() {
            let o = RegenOptions { upto: Some(k + 1), preview_meshes: true, ..opts.clone() };
            let ctx = reg.regenerate(ps, &ConfigValues::default(), units, &o).context;
            let after: Vec<Print> = ctx.bodies.iter().map(|b| print(b)).collect();
            out.push(after.iter().filter(|p| !before.contains(p)).cloned().collect());
            before = after;
        }
        out
    };
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape") && only.as_ref().is_none_or(|o| p.file_name().is_some_and(|n| n.to_string_lossy() == *o))).collect();
        files.sort();
        for path in files {
            let copy = tmp.join(path.file_name().unwrap());
            std::fs::copy(&path, &copy).unwrap();
            let Ok(s) = noshape_ops::session::Session::open(&copy) else { continue };
            let units = s.doc.settings.units;
            let opts = RegenOptions { custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&s.doc)), preview_meshes: true, ..Default::default() };
            let doc = path.file_stem().unwrap().to_string_lossy().to_string();
            for tab in &s.doc.tabs {
                let TabContent::PartStudio(ps) = &tab.content else { continue };
                if ps.features.is_empty() {
                    continue;
                }
                let mut reg = Regenerator::new();
                let t = Instant::now();
                let _ = reg.regenerate(ps, &ConfigValues::default(), &units, &opts);
                let open_ms = t.elapsed().as_secs_f64() * 1e3;
                let n = ps.features.len();
                println!("edit {doc} / {}: {n} features, first regeneration {open_ms:.0} ms", tab.name);
                // The edits: (label, the studio after it).
                let mut edits: Vec<(String, noshape_model::partstudio::PartStudio)> = Vec::new();
                if let Some((i, f)) = ps.features.iter().enumerate().find(|(_, f)| matches!(f.params.get("sketch"), Some(ParamValue::Sketch(d)) if !d.dimension_exprs.is_empty())) {
                    let mut e = ps.clone();
                    if let Some(ParamValue::Sketch(d)) = e.features[i].params.0.get_mut("sketch") {
                        let (k, v) = d.dimension_exprs.iter().next().map(|(k, v)| (*k, scaled(v))).unwrap();
                        d.dimension_exprs.insert(k, v);
                    }
                    edits.push((format!("dimension in {} (#{})", f.name, i + 1), e));
                }
                for (ty, key, what) in [("extrude", "depth", "depth"), ("fillet", "radius", "radius")] {
                    if let Some((i, f)) = ps.features.iter().enumerate().find(|(_, f)| f.type_id == ty && !f.suppressed && matches!(f.params.get(key), Some(ParamValue::Expr(_)))) {
                        let mut e = ps.clone();
                        if let Some(ParamValue::Expr(x)) = e.features[i].params.0.get_mut(key) {
                            *x = scaled(x);
                        }
                        edits.push((format!("{what} of {} (#{})", f.name, i + 1), e));
                    }
                }
                let mid = n / 2;
                let mut e = ps.clone();
                e.features[mid].suppressed = !e.features[mid].suppressed;
                edits.push((format!("suppress {} (#{})", ps.features[mid].name, mid + 1), e));
                let old_effects = effects(&mut reg, ps, &units, &opts);
                for (label, edited) in edits {
                    for (step, studio) in [(label.clone(), &edited), (format!("undo ({label})"), ps)] {
                        let t = Instant::now();
                        let r = reg.regenerate(studio, &ConfigValues::default(), &units, &opts);
                        let ms = t.elapsed().as_secs_f64() * 1e3;
                        let ran: Vec<usize> = (0..studio.features.len()).filter(|&k| r.evaluated.contains(&studio.features[k].id)).collect();
                        let time = |k: usize| r.display.features.get(k).map_or(0.0, |st| st.time_ms as f64);
                        let features: f64 = ran.iter().map(|&k| time(k)).sum();
                        // Features that ran again with their own effect as before (and the same position: no reorder).
                        let new_effects = if std::ptr::eq(studio, ps) { old_effects.clone() } else { effects(&mut reg, studio, &units, &opts) };
                        let same: Vec<usize> = ran.iter().copied().filter(|&k| k > 0 && !studio.features[k].suppressed && new_effects.get(k) == old_effects.get(k) && studio.features[k].id == ps.features[k].id).collect();
                        let same_ms: f64 = same.iter().map(|&k| time(k)).sum();
                        let names: Vec<&str> = same.iter().filter(|&&k| time(k) >= 1.0).map(|&k| studio.features[k].name.as_str()).collect();
                        println!("timing edit {doc} / {} / {step}: {ms:.1} ms (features {features:.1}, display and the rest {:.1}), {} of {n} features ran, {} of them ({same_ms:.1} ms) with their effect unchanged {names:?}", tab.name, ms - features, ran.len(), same.len());
                        if check {
                            noshape_model::regen::set_reuse(Some(false));
                            let full = Regenerator::new().regenerate(studio, &ConfigValues::default(), &units, &opts);
                            noshape_model::regen::set_reuse(Some(true));
                            let sig = |r: &RegenResult| -> Vec<String> {
                                let mut v: Vec<String> = r.context.bodies.iter().map(|b| format!("{} {} {:?} {} {:?} {:?} {:?} {:.9} {:.9} {}", b.id, b.name, b.kind, b.color, b.faces, b.edges, b.vertices, b.shape.volume().unwrap_or(0.0) * 1e9, noshape_kernel::surface_properties(&b.shape).map_or(0.0, |p| p.mass) * 1e6, b.shape.count(noshape_kernel::ShapeType::Face))).collect();
                                v.extend(r.display.features.iter().map(|st| format!("{:?} {:?} {:?}", st.id, st.status, st.message)));
                                v
                            };
                            let (a, b) = (sig(&r), sig(&full));
                            let diff: Vec<(&String, &String)> = a.iter().zip(&b).filter(|(x, y)| x != y).take(3).collect();
                            assert!(a.len() == b.len() && diff.is_empty(), "{doc} / {} / {step}: incremental differs from full: {} vs {} lines; {diff:#?}", tab.name, a.len(), b.len());
                        }
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The whole-result cache's key costs a serialization of the studio a run: its time on the perf documents, and a
/// repeated run's (a cache hit) against the first.
#[test]
#[ignore = "T2 probe"]
fn result_key_cost_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    let tmp = std::env::temp_dir().join(format!("noshape-keycost-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for name in ["live-30-features", "big-plate", "fins-400", "knurl-d25x25-45-45", "parts-1000"] {
        let copy = tmp.join(format!("{name}.noshape"));
        std::fs::copy(dir.join(format!("{name}.noshape")), &copy).unwrap();
        let s = noshape_ops::session::Session::open(&copy).unwrap();
        let units = s.doc.settings.units;
        for tab in &s.doc.tabs {
            let TabContent::PartStudio(ps) = &tab.content else { continue };
            let t = Instant::now();
            let mut bytes = 0;
            for _ in 0..10 {
                bytes = serde_json::to_vec(ps).unwrap().len();
            }
            let ser = t.elapsed().as_secs_f64() * 1e3 / 10.0;
            let mut reg = Regenerator::new();
            let t = Instant::now();
            reg.regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
            let first = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let again = reg.regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
            println!("{name}: {} features, studio {} kB serialized in {ser:.2} ms; first run {first:.0} ms, again {:.2} ms ({} evaluated)", ps.features.len(), bytes / 1024, t.elapsed().as_secs_f64() * 1e3, again.evaluated.len());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Which feature of a document first leaves a body with a vertex tolerance at or over NOSHAPE_TOL_OVER (default 1e-5
/// m): each feature's run (rollback after it), the largest vertex tolerance of its bodies (their B-rep text).
/// `NOSHAPE_TOL_DOC=<file>`.
#[test]
#[ignore = "T2 tool: NOSHAPE_TOL_DOC"]
fn tolerance_by_feature() {
    let Ok(path) = std::env::var("NOSHAPE_TOL_DOC") else { return };
    let over: f64 = std::env::var("NOSHAPE_TOL_OVER").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-5);
    let copy = std::env::temp_dir().join(format!("noshape-tol-{}.noshape", std::process::id()));
    std::fs::copy(&path, &copy).unwrap();
    let s = noshape_ops::session::Session::open(&copy).unwrap();
    let units = s.doc.settings.units;
    let mut reg = Regenerator::new();
    for tab in &s.doc.tabs {
        let TabContent::PartStudio(ps) = &tab.content else { continue };
        for k in 0..ps.features.len() {
            let r = reg.regenerate(ps, &ConfigValues::default(), &units, &RegenOptions { upto: Some(k + 1), ..Default::default() });
            let mut worst = (0.0f64, String::new());
            for b in &r.context.bodies {
                let Ok(bytes) = b.shape.to_brep_bytes() else { continue };
                let text = String::from_utf8_lossy(&bytes);
                let mut lines = text.lines();
                while let Some(l) = lines.next() {
                    if l.trim() == "Ve"
                        && let Some(t) = lines.next().and_then(|t| t.split_whitespace().next()).and_then(|t| t.parse::<f64>().ok())
                        && t > worst.0
                    {
                        worst = (t, b.id.to_string());
                    }
                }
            }
            let st = &r.display.features[k];
            println!("{} {} ({:?}): largest vertex tolerance {:.3e} ({}){}", k + 1, ps.features[k].name, st.status, worst.0, worst.1, if worst.0 >= over { "  <-- over" } else { "" });
        }
    }
    let _ = std::fs::remove_file(&copy);
}

/// A document's bodies after one feature written as BREP (NOSHAPE_BODY_DOC, NOSHAPE_BODY_UPTO = the feature's name):
/// `<doc>.<feature>.<i>.brep`, for kernel probes.
#[test]
#[ignore = "T2 tool: NOSHAPE_BODY_DOC"]
fn bodies_after_feature() {
    let (Ok(path), Ok(upto)) = (std::env::var("NOSHAPE_BODY_DOC"), std::env::var("NOSHAPE_BODY_UPTO")) else { return };
    let tmp = std::env::temp_dir().join(format!("noshape-bodies-{}.noshape", std::process::id()));
    std::fs::copy(&path, &tmp).unwrap();
    // (A document a test left open: its write-ahead log too.)
    for ext in ["-wal", "-shm"] {
        let _ = std::fs::copy(format!("{path}{ext}"), format!("{}{ext}", tmp.display()));
    }
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    for tab in &s.doc.tabs {
        let TabContent::PartStudio(ps) = &tab.content else { continue };
        let Some(k) = ps.features.iter().position(|f| f.name == upto) else { continue };
        let mut p = ps.clone();
        p.features.truncate(k + 1);
        // (The document's Feature Studios: its custom features build.)
        let opts = RegenOptions { custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&s.doc)), ..Default::default() };
        let r = Regenerator::new().regenerate(&p, &ConfigValues::default(), &units, &opts);
        for (i, b) in r.context.bodies.iter().enumerate() {
            let out = format!("{path}.{upto}.{i}.brep");
            noshape_kernel::write_brep(&b.shape, std::path::Path::new(&out)).unwrap();
            let bb = b.shape.bounding_box(false).ok();
            println!("wrote {out} ({}): {} faces, volume {:?}, box {:?}", b.name, b.shape.faces().len(), b.shape.volume().ok(), bb.map(|x| (x.min * 1e3, x.max * 1e3)));
        }
    }
    let _ = std::fs::remove_file(&tmp);
}

/// A document's Part Studios (NOSHAPE_DUMP_DOC): each feature with its type, status and its query parameters'
/// references, then each part's volume and face count after the last feature and after each one named in
/// NOSHAPE_DUMP_AFTER (comma-separated).
#[test]
#[ignore = "T2 tool: NOSHAPE_DUMP_DOC"]
fn dump_document() {
    let Ok(path) = std::env::var("NOSHAPE_DUMP_DOC") else { return };
    let tmp = std::env::temp_dir().join(format!("noshape-dump-{}.noshape", std::process::id()));
    std::fs::copy(&path, &tmp).unwrap();
    for ext in ["-wal", "-shm"] {
        let _ = std::fs::copy(format!("{path}{ext}"), format!("{}{ext}", tmp.display()));
    }
    let s = noshape_ops::session::Session::open(&tmp).unwrap();
    let units = s.doc.settings.units;
    let after: Vec<String> = std::env::var("NOSHAPE_DUMP_AFTER").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default();
    for tab in &s.doc.tabs {
        let TabContent::PartStudio(ps) = &tab.content else { continue };
        println!("tab {} (scope rule {:?})", tab.name, ps.scope_rule);
        let opts = RegenOptions { custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&s.doc)), ..Default::default() };
        let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &opts);
        for (f, st) in ps.features.iter().zip(&r.display.features) {
            println!("  {} [{}] {:?} {:?}", f.name, f.type_id, st.status, st.message);
            for (k, v) in f.params.0.iter() {
                if let ParamValue::Query(q) = v
                    && !q.is_empty()
                {
                    println!("    {k}: {q:?}");
                }
            }
        }
        let parts = |r: &RegenResult| r.context.bodies.iter().map(|b| format!("{} {:.4} mm³ {} faces", b.name, b.shape.volume().unwrap_or(0.0) * 1e9, b.shape.faces().len())).collect::<Vec<_>>();
        println!("  parts at the end: {:?}", parts(&r));
        for name in &after {
            let Some(k) = ps.features.iter().position(|f| &f.name == name) else { continue };
            let mut p = ps.clone();
            p.features.truncate(k + 1);
            let r = Regenerator::new().regenerate(&p, &ConfigValues::default(), &units, &opts);
            println!("  parts after {name}: {:?}", parts(&r));
            for b in r.context.bodies.iter() {
                for (i, n) in b.faces.iter().enumerate() {
                    let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok();
                    let c = face.as_ref().and_then(|f| f.centroid().ok()).unwrap_or_default() * 1e3;
                    let a = face.as_ref().and_then(|f| f.area().ok()).unwrap_or_default() * 1e6;
                    println!("    {} face {i} at ({:.2}, {:.2}, {:.2}) area {a:.6}: {n:?}", b.name, c.x, c.y, c.z);
                }
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
}

/// The user's Gyro (NOSHAPE_GYRO_DOC): what Split 1 made, then a Chamfer of the inner ring's top face (its smallest
/// upward planar face) of NOSHAPE_GYRO_WIDTH (default 0.5 mm) and the parts after it.
#[test]
#[ignore = "T2 probe: NOSHAPE_GYRO_DOC"]
fn gyro_chamfer_probe() {
    use noshape_ops::{Ctx, registry};
    use serde_json::json;
    let Ok(path) = std::env::var("NOSHAPE_GYRO_DOC") else { return };
    let tmp = std::env::temp_dir().join(format!("noshape-gyro-{}.noshape", std::process::id()));
    std::fs::copy(&path, &tmp).unwrap();
    let mut ctx = Ctx::new();
    let mut call = |tool: &str, args: serde_json::Value| {
        let op = registry::by_tool_name(tool).unwrap();
        registry::run(&mut ctx, op, &args).map(|o| o.value).unwrap_or_else(|e| e.to_json())
    };
    call("document_open", json!({ "path": tmp.to_str().unwrap() }));
    let made = call("query_entities", json!({ "kind": "faces", "created_by": "Split 1", "limit": 100 }));
    println!("GYRO created_by Split 1: {}", made.to_string().chars().take(600).collect::<String>());
    let tops = call("query_entities", json!({ "kind": "faces", "surface": "plane", "normal": [0, 0, 1], "limit": 100 }));
    println!("GYRO upward planar faces: {}", tops.to_string().chars().take(1500).collect::<String>());
    let before = call("query_parts", json!({}));
    println!("GYRO parts before: {}", before.to_string().chars().take(1500).collect::<String>());
    let faces = tops.get("faces").or_else(|| tops.get("entities")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let area = |f: &serde_json::Value| f.get("area").and_then(|a| a.as_f64()).unwrap_or(f64::MAX);
    let Some(inner) = faces.iter().min_by(|a, b| area(a).total_cmp(&area(b))) else { return };
    let id = inner["id"].clone();
    let width = std::env::var("NOSHAPE_GYRO_WIDTH").unwrap_or_else(|_| "0.5 mm".into());
    let added = call("feature_add", json!({ "type": "chamfer", "params": { "entities": [id], "width": width, "tangentPropagation": true } }));
    println!("GYRO chamfer of {}: {}", inner, added.to_string().chars().take(400).collect::<String>());
    let after = call("query_parts", json!({}));
    println!("GYRO parts after: {}", after.to_string().chars().take(1500).collect::<String>());
    let _ = std::fs::remove_file(&tmp);
}
