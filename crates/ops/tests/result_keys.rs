//! The persistent regeneration cache must never show a stale part: a Part Studio's whole-result key changes with
//! every input outside its own features (lead 11-11): a derived source tab, a source in another document or version,
//! a Variable Studio value, the Feature Studio code a custom feature uses, the configuration, the document's units,
//! a File tab's bytes (a Decal's image).

use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::{Feature, PartStudio};
use noshape_model::regen::RegenKey;
use noshape_ops::doc::OpenDoc;
use noshape_ops::session::Session;
use noshape_units::{ExprValue, UnitPrefs};

fn block(depth: &str) -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("depth", ParamValue::Expr(ExprValue::new(depth)));
    ps.features.push(ex);
    ps
}

fn feature(ps: &PartStudio, type_id: &str, params: &[(&str, ParamValue)]) -> Feature {
    let mut f = ps.new_feature("extrude", &UnitPrefs::default()).unwrap();
    f.type_id = type_id.into();
    f.name = type_id.into();
    f.params = Default::default();
    for (k, v) in params {
        f.params.set(k, v.clone());
    }
    f
}

struct Doc {
    open: OpenDoc,
    _dir: std::path::PathBuf,
}

impl Doc {
    fn new(doc: Document, name: &str) -> Doc {
        let dir = std::env::temp_dir().join(format!("noshape-result-keys-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let session = Session::create(dir.join("doc.noshape"), doc).unwrap();
        Doc { open: OpenDoc::new(session), _dir: dir }
    }
    fn key(&mut self, tab: TabId) -> RegenKey {
        self.open.regenerate(tab, None).unwrap().result_key.expect("a full run has a key")
    }
    fn edit(&mut self, f: impl FnOnce(&mut Document)) {
        f(&mut self.open.session.doc);
    }
}

fn studio_mut(doc: &mut Document, tab: TabId) -> &mut PartStudio {
    match &mut doc.tabs.iter_mut().find(|t| t.id == tab).unwrap().content {
        TabContent::PartStudio(ps) => ps,
        _ => unreachable!(),
    }
}

#[test]
fn a_derived_source_tab_changes_the_key() {
    let mut doc = Document::new();
    let a = doc.add_tab("Source", TabContent::PartStudio(block("10 mm")));
    let mut b = PartStudio::new();
    b.features.push(feature(&b, "importDerived", &[("partStudio", ParamValue::Text(a.to_string()))]));
    let b = doc.add_tab("Derived", TabContent::PartStudio(b));
    let mut d = Doc::new(doc, "derived");
    let k0 = d.key(b);
    assert_eq!(k0, d.key(b), "the same document, the same key");
    d.edit(|doc| studio_mut(doc, a).features[0].params.set("depth", ParamValue::Expr(ExprValue::new("12 mm"))));
    assert_ne!(k0, d.key(b), "the source's depth");
}

#[test]
fn a_source_in_another_document_or_version_changes_the_key() {
    let mut doc = Document::new();
    let mut b = PartStudio::new();
    b.features.push(feature(&b, "importDerived", &[("partStudio", ParamValue::Text("ext:d1/v1/e1".into()))]));
    let b = doc.add_tab("Derived", TabContent::PartStudio(b));
    let mut d = Doc::new(doc, "ext");
    let k0 = d.key(b);
    d.edit(|doc| studio_mut(doc, b).features[0].params.set("partStudio", ParamValue::Text("ext:d1/v2/e1".into())));
    assert_ne!(k0, d.key(b), "another version");
}

#[test]
fn a_variable_studio_value_changes_the_key() {
    let mut doc = Document::new();
    let var = |v: &str| serde_json::from_value::<noshape_model::variables::VariableDef>(serde_json::json!({ "name": "w", "expr": serde_json::to_value(ExprValue::new(v)).unwrap() })).unwrap();
    let vs = doc.add_tab("Variables", TabContent::VariableStudio(noshape_model::variables::VariableStudio { variables: vec![var("10 mm")], ..Default::default() }));
    let ps = doc.add_tab("Part Studio", TabContent::PartStudio(block("#w")));
    let mut d = Doc::new(doc, "variables");
    let k0 = d.key(ps);
    d.edit(|doc| {
        if let TabContent::VariableStudio(v) = &mut doc.tabs.iter_mut().find(|t| t.id == vs).unwrap().content {
            v.variables[0] = var("11 mm");
        }
    });
    assert_ne!(k0, d.key(ps), "the variable's value");
}

#[test]
fn feature_studio_code_a_custom_feature_uses_changes_the_key() {
    let mut doc = Document::new();
    let src = |n: &str| format!("FeatureScript 2892;\nimport(path : \"onshape/std/geometry.fs\", version : \"2892.0\");\n\nannotation {{ \"Feature Type Name\" : \"Thing\" }}\nexport const thing = defineFeature(function(context is Context, id is Id, definition is map)\n    precondition {{}}\n    {{\n        fCuboid(context, id + \"c\", {{ \"corner1\" : vector(0, 0, 0) * millimeter, \"corner2\" : vector({n}, 1, 1) * millimeter }});\n    }});\n");
    let fs = doc.add_tab("Feature Studio", TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(&src("1"))));
    let mut ps = PartStudio::new();
    ps.features.push(feature(&ps, &noshape_model::custom::type_id(fs, "thing"), &[]));
    let ps = doc.add_tab("Part Studio", TabContent::PartStudio(ps));
    let mut d = Doc::new(doc, "custom");
    let k0 = d.key(ps);
    d.edit(|doc| {
        if let TabContent::FeatureStudio(f) = &mut doc.tabs.iter_mut().find(|t| t.id == fs).unwrap().content {
            *f = noshape_model::custom::FeatureStudio::new(&src("2"));
        }
    });
    assert_ne!(k0, d.key(ps), "the Feature Studio's code");
}

#[test]
fn the_configuration_and_the_units_change_the_key() {
    let mut doc = Document::new();
    let mut studio = block("10 mm");
    studio.configuration.inputs.push(serde_json::from_value(serde_json::json!({ "id": "List", "name": "List", "kind": { "List": { "options": [{ "id": "Default", "name": "Default" }, { "id": "Other", "name": "Other" }], "default": "Default" } } })).unwrap());
    let ps = doc.add_tab("Part Studio", TabContent::PartStudio(studio));
    let mut d = Doc::new(doc, "config");
    let k0 = d.key(ps);
    let other = noshape_model::configuration::ConfigValues([("List".to_string(), "Other".to_string())].into_iter().collect());
    let k1 = d.open.regenerate(ps, Some(&other)).unwrap().result_key.unwrap();
    assert_ne!(k0, k1, "another configuration");
    d.edit(|doc| doc.settings.units.length = noshape_units::LengthUnit::Inch);
    assert_ne!(k0, d.key(ps), "the document's units");
}

#[test]
fn a_file_tabs_bytes_change_the_key() {
    let mut doc = Document::new();
    let file = |b: &[u8]| noshape_model::file_tab::FileTab::new("image.png", b, 1);
    let img = doc.add_tab("image.png", TabContent::File(file(b"first image")));
    let mut ps = block("10 mm");
    ps.features.push(feature(&ps, "decal", &[("image", ParamValue::Text(img.to_string()))]));
    let ps = doc.add_tab("Part Studio", TabContent::PartStudio(ps));
    let mut d = Doc::new(doc, "file");
    let k0 = d.key(ps);
    // The same name, size and upload time, other bytes.
    d.edit(|doc| doc.tabs.iter_mut().find(|t| t.id == img).unwrap().content = TabContent::File(file(b"other image")));
    assert_ne!(k0, d.key(ps), "the File tab's bytes");
}

/// Probe: a document's parts' shown faces in memory, after OCCT's binary round trip, and after the result store's
/// encode / decode (NOSHAPE_PROBE_DOC: a document copied to a scratch dir first).
#[test]
#[ignore = "T2: probe"]
fn shown_faces_through_the_store_probe() {
    let Ok(path) = std::env::var("NOSHAPE_PROBE_DOC") else { return };
    let dir = std::env::temp_dir().join(format!("noshape-store-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(&path, dir.join("d.noshape")).unwrap();
    let mut open = OpenDoc::new(Session::open(dir.join("d.noshape")).unwrap());
    let tabs: Vec<TabId> = open.doc().tabs.iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id).collect();
    let shown = |s: &noshape_kernel::Shape| s.shown_topology().map(|t| t.face_count()).unwrap_or(0);
    for tab in tabs {
        let r = open.regenerate(tab, None).unwrap();
        let bytes = noshape_model::regen::persist::encode_result(&r.context, &r.display.features).unwrap();
        let (back, _) = noshape_model::regen::persist::decode_result(&bytes).unwrap();
        for (b, d) in r.context.bodies.untracked().iter().zip(back.bodies.untracked()) {
            let bin = noshape_kernel::Shape::from_bin_bytes(&b.shape.to_bin_bytes(false).unwrap()).unwrap();
            println!("{}: {} faces, shown {} in memory, {} after binary, {} after the store ({} faces)", b.name, b.shape.faces().len(), shown(&b.shape), shown(&bin), shown(&d.shape), d.shape.faces().len());
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
