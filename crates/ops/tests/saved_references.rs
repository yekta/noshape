//! Documents saved by older releases whose references must keep resolving.

use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::document::TabContent;
use noshape_model::regen::{RegenOptions, Regenerator};

/// qa's P0 (11-12, cli-chamfer-hang-pre, saved by an older release): a cut there leaves one solid touching itself
/// along an edge, which our Boolean once paired as two lumps' faces; the result looked the same but moved the faces a
/// later Linear pattern's stored direction names ("Select a pattern direction.", 5 of 11 parts lost). Its direction
/// resolves and all 11 parts are there, as on v0.4.44.
#[test]
fn an_older_documents_pattern_direction_still_resolves() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli-chamfer-hang-pre.noshape");
    let tmp = std::env::temp_dir().join(format!("noshape-chamfer-hang-pre-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let copy = tmp.join("cli-chamfer-hang-pre.noshape");
    std::fs::copy(&path, &copy).unwrap();
    let s = noshape_ops::session::Session::open(&copy).unwrap();
    let ps = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps) } else { None }).expect("a Part Studio");
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &s.doc.settings.units, &RegenOptions::default());
    let _ = std::fs::remove_dir_all(&tmp);
    let lp2 = ps.features.iter().position(|f| f.name == "Linear pattern 2").expect("Linear pattern 2");
    let st = &r.display.features[lp2];
    assert_eq!(st.status, FeatureStatus::Ok, "{:?}", st.message);
    assert_eq!(r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).count(), 11);
}
