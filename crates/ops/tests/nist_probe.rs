//! Local probe (not committed): the NIST fixture's display meshing.
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabContent;
use noshape_model::regen::{RegenOptions, Regenerator};
use std::time::Instant;

#[test]
#[ignore]
fn nist_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    let tmp = std::env::temp_dir().join(format!("noshape-nist-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let copy = tmp.join("nist.noshape");
    std::fs::copy(dir.join("nist-mtc-crada-assembly.noshape"), &copy).unwrap();
    let s = noshape_ops::session::Session::open(&copy).unwrap();
    let units = s.doc.settings.units;
    for tab in &s.doc.tabs {
        if let TabContent::PartStudio(ps) = &tab.content {
            for _ in 0..2 {
                let t = Instant::now();
                let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
                let tris: usize = r.display.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
                eprintln!("nist: {} ms total, {} triangles, delabella {}", t.elapsed().as_millis(), tris, std::env::var("NOSHAPE_MESH_DELABELLA").is_ok());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
