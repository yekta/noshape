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
    let t = Instant::now();
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), units, &RegenOptions::default());
    let total = t.elapsed().as_secs_f64() * 1e3;
    let d = &r.display;
    eprintln!("profile: {model}: {total:.0} ms total, features {:.0} ms, display {:.0} ms, {} features", d.regen_ms, total - d.regen_ms as f64, ps.features.len());
    for (f, s) in ps.features.iter().zip(&d.features) {
        rows.push((model.to_string(), f.name.clone(), f.type_id.clone(), s.time_ms as f64));
    }
    rows.push((model.to_string(), "(display)".into(), "display".into(), total - d.regen_ms as f64));
    r
}

fn fixtures(rows: &mut Vec<Row>) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
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
        for tab in &s.doc.tabs {
            if let TabContent::PartStudio(ps) = &tab.content {
                let name = format!("{} / {}", path.file_stem().unwrap().to_string_lossy(), tab.name);
                profile(&name, ps, &units, rows);
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
