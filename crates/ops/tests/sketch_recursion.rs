//! qa SKETCH-CONSTRUCTIVE-RECURSION (P0, v0.4.21): opening a sketch-heavy fuzz
//! document (CLI fuzz seed 211, step 89) overflowed the stack in the sketch's
//! constructive solve. It opens and regenerates again (v0.4.20: "no solid
//! parts").

use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabContent;
use noshape_model::regen::{RegenOptions, Regenerator};

#[test]
fn the_sk211_fuzz_document_opens_and_regenerates() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sk211-step89.noshape");
    let tmp = std::env::temp_dir().join(format!("noshape-sk211-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let copy = tmp.join("sk211-step89.noshape");
    std::fs::copy(&path, &copy).unwrap();
    let s = noshape_ops::session::Session::open(&copy).unwrap();
    let mut studios = 0;
    for t in &s.doc.tabs {
        if let TabContent::PartStudio(ps) = &t.content {
            studios += 1;
            let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &s.doc.settings.units, &RegenOptions::default());
            assert!(!r.cancelled);
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(studios > 0);
}
