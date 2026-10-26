//! T0 smoke (docs/TESTS.md): the sample documents open and regenerate with
//! every feature Ok, as when the app opens them. Fast and deterministic; run
//! before every push (tools/tiers/t0.sh).

use noshape_model::display::FeatureStatus;
use noshape_model::document::{Document, TabContent};
use noshape_ops::Ctx;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("noshape-ops-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    let _ = std::fs::remove_file(&p);
    p
}

fn fixture(name: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf").join(name);
    let copy = scratch(name);
    std::fs::copy(&src, &copy).unwrap();
    copy
}

/// Regenerates every Part Studio of the open document; each feature must be
/// Ok (or suppressed / rolled back). Returns (studio name, solid count, volume m³).
fn regenerate_all(ctx: &mut Ctx, path: &Path) -> Vec<(String, usize, f64)> {
    let od = ctx.open(path).unwrap_or_else(|e| panic!("open {}: {}", path.display(), e.to_json()));
    let tabs: Vec<_> = od.doc().tabs.iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| (t.id, t.name.clone())).collect();
    assert!(!tabs.is_empty(), "{}: no Part Studio", path.display());
    let mut out = Vec::new();
    for (tab, name) in tabs {
        let r = od.regenerate(tab, None).unwrap_or_else(|e| panic!("{name}: {}", e.to_json()));
        assert!(!r.cancelled, "{name}: cancelled");
        let bad: Vec<_> = r.display.features.iter().filter(|f| matches!(f.status, FeatureStatus::Error | FeatureStatus::Warning)).map(|f| format!("{:?} {:?}", f.id, f.message)).collect();
        assert!(bad.is_empty(), "{}: {name}: features not Ok: {bad:?}", path.display());
        let solids = r.context.bodies.iter().filter(|b| b.shape.volume().is_ok_and(|v| v > 0.0)).count();
        let volume = r.context.bodies.iter().filter_map(|b| b.shape.volume().ok()).filter(|v| *v > 0.0).sum();
        out.push((name, solids, volume));
    }
    out
}

fn create(ctx: &mut Ctx, name: &str, doc: Document) -> PathBuf {
    let path = scratch(name);
    ctx.create(&path, doc).unwrap_or_else(|e| panic!("create {name}: {}", e.to_json()));
    path
}

#[test]
fn demo_document_regenerates() {
    let mut ctx = Ctx::new();
    let path = create(&mut ctx, "demo.noshape", noshape_model::demo::demo_document());
    let studios = regenerate_all(&mut ctx, &path);
    assert!(studios.iter().any(|(_, n, _)| *n > 0), "{studios:?}");
}

#[test]
fn demo_document_with_drawing_regenerates() {
    let mut ctx = Ctx::new();
    let path = create(&mut ctx, "demo-drawing.noshape", noshape_model::demo::demo_document_with_drawing());
    let studios = regenerate_all(&mut ctx, &path);
    assert!(studios.iter().any(|(_, n, _)| *n > 0), "{studios:?}");
    assert!(ctx.doc(None).unwrap().doc().tabs.iter().any(|t| matches!(t.content, TabContent::Drawing(_))));
}

/// Live's 30-feature part (tests/perf/README.md): 60779.484 mm³ in live.
#[test]
fn live_30_features_opens_and_regenerates() {
    let mut ctx = Ctx::new();
    let path = fixture("live-30-features.noshape");
    let studios = regenerate_all(&mut ctx, &path);
    let (_, n, v) = &studios[0];
    assert_eq!(*n, 1, "{studios:?}");
    let mm3 = v * 1e9;
    assert!((mm3 - 60779.484).abs() < 0.5, "volume {mm3} mm³ vs live 60779.484");
}
