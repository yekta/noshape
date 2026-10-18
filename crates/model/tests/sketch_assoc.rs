//! Associative sketch geometry: intersection curves follow the model, and
//! sketch text regenerates its glyph outlines from the text box.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{FeatureStatus, PlaneFrame};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::features::sketch::{add_section_curves, section_face};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::{IntersectionLink, SketchData};
use noshape_sketch::EntityId;
use noshape_units::{ExprValue, UnitPrefs};

/// Centered 100 x 50 mm block, 25 mm tall (extruded up from Top).
fn block() -> (PartStudio, UnitPrefs) {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.corner_rectangle(DVec2::new(-0.05, -0.025), DVec2::new(0.05, 0.025));
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sketch_id = sketch.id;
    ps.features.push(sketch);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = r.display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sketch_id, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("25 mm")));
    ps.features.push(ex);
    (ps, units)
}

fn regen(ps: &PartStudio, units: &UnitPrefs) -> noshape_model::regen::RegenResult {
    let mut reg = Regenerator::new();
    let r = reg.regenerate(ps, &ConfigValues::default(), units, &RegenOptions::default());
    for f in &r.display.features {
        assert_eq!(f.status, FeatureStatus::Ok, "{:?}: {:?}", f.id, f.message);
    }
    r
}

#[test]
fn intersection_curves_follow_the_model() {
    let (mut ps, units) = block();
    let r = regen(&ps, &units);
    // The top cap of the block crosses the Front plane (y = 0) along a line.
    let body = &r.context.bodies[0];
    let cap = body.faces.iter().find(|n| format!("{n:?}").contains("CapEnd")).expect("top cap face");
    let face_ref = Reference::Face((**cap).clone());
    let faces = noshape_model::regen::measure::resolve_shapes(&r.context, &face_ref);
    let frame = PlaneFrame::FRONT;
    let curves = section_face(&faces[0], &frame).unwrap();
    assert_eq!(curves.len(), 1, "{curves:?}");
    let mut sk = noshape_sketch::Sketch::new();
    let ids = add_section_curves(&mut sk, &curves);
    assert_eq!(ids.len(), 1);
    let data = SketchData { sketch: sk, intersections: vec![IntersectionLink { face: face_ref, entities: ids.iter().map(|e| e.0).collect() }], ..Default::default() };
    let mut s2 = ps.new_feature("sketch", &units).unwrap();
    s2.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
    s2.params.set("sketch", ParamValue::Sketch(Box::new(data)));
    let s2_id = s2.id;
    ps.features.push(s2);

    let line_height = |r: &noshape_model::regen::RegenResult| -> (f64, f64) {
        let s = r.context.sketches.iter().find(|s| s.feature == s2_id).unwrap();
        let c = s.sketch.curve(ids[0]).unwrap();
        let (a, b) = (s.frame.to_world(c.start_point()), s.frame.to_world(c.end_point()));
        ((a.z + b.z) * 0.5, (a - b).length())
    };
    let r = regen(&ps, &units);
    let (z, len) = line_height(&r);
    assert!((z - 0.025).abs() < 1e-9 && (len - 0.1).abs() < 1e-9, "z {z} len {len}");

    // Make the block taller and wider: the intersection line follows.
    if let Some(ParamValue::Expr(e)) = ps.features[1].params.0.get_mut("depth") {
        *e = ExprValue::new("40 mm");
    }
    if let Some(ParamValue::Sketch(d)) = ps.features[0].params.0.get_mut("sketch") {
        let pts: Vec<EntityId> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).collect();
        for p in &pts[1..3] {
            let q = d.sketch.point(*p).unwrap();
            d.sketch.set_point(*p, q + DVec2::new(0.02, 0.0)).unwrap();
        }
    }
    let r = regen(&ps, &units);
    let (z, len) = line_height(&r);
    assert!((z - 0.04).abs() < 1e-9, "line moved up with the cap: z {z}");
    assert!((len - 0.12).abs() < 1e-9, "line follows the wider block: {len}");
    let warn = r.display.features.iter().find(|f| f.id == s2_id).and_then(|f| f.message.clone());
    assert!(warn.as_ref().is_none_or(|m| !m.contains("intersection")), "{warn:?}");
}

#[test]
fn sketch_text_regenerates_glyph_regions() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut data = SketchData::default();
    let tid = data.add_text("IO", "Liberation Sans", false, false, DVec2::new(0.0, 0.0), 0.01).unwrap();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(data)));
    ps.features.push(sketch);
    let r = regen(&ps, &units);
    // "I" is one region, "O" a ring plus its counter: three regions;
    // outlines are shown but no glyph points.
    let d = &r.display.sketches[0];
    assert_eq!(d.regions.len(), 3, "regions of IO");
    assert!(d.points.iter().all(|p| noshape_sketch::text::glyph_text_id(EntityId(p.entity)).is_none()));
    let glyph_curves = d.curves.iter().filter(|c| noshape_sketch::text::glyph_text_id(EntityId(c.entity)).is_some()).count();
    assert!(glyph_curves > 3);
    let keys: Vec<Vec<u32>> = d.regions.iter().map(|r| r.key.clone()).collect();

    // Regenerating again yields the same region keys (stable references).
    let r2 = regen(&ps, &units);
    let keys2: Vec<Vec<u32>> = r2.display.sketches[0].regions.iter().map(|r| r.key.clone()).collect();
    assert_eq!(keys, keys2);

    // Editing the text regenerates the outlines; bold widens the box.
    let width_before = {
        let s = &r.context.sketches[0];
        let t = &s.sketch;
        let data = match ps.features[0].params.0.get("sketch") {
            Some(ParamValue::Sketch(d)) => d.clone(),
            _ => unreachable!(),
        };
        t.curve(EntityId(data.texts[0].box_lines[0])).unwrap().length()
    };
    if let Some(ParamValue::Sketch(d)) = ps.features[0].params.0.get_mut("sketch") {
        d.update_text(tid, |t| {
            t.text = "IOI".into();
            t.bold = true;
        })
        .unwrap();
    }
    let r = regen(&ps, &units);
    assert_eq!(r.display.sketches[0].regions.len(), 4, "regions of IOI");
    let data = match ps.features[0].params.0.get("sketch") {
        Some(ParamValue::Sketch(d)) => d.clone(),
        _ => unreachable!(),
    };
    let w = r.context.sketches[0].sketch.curve(EntityId(data.texts[0].box_lines[0])).unwrap().length();
    assert!(w > width_before * 1.2, "box width follows the text: {w} vs {width_before}");

    // Deleting the box removes the text.
    if let Some(ParamValue::Sketch(d)) = ps.features[0].params.0.get_mut("sketch") {
        let line = EntityId(d.texts[0].box_lines[0]);
        d.sketch.remove_entities(&[line]);
        d.prune_texts();
        assert!(d.texts.is_empty());
        assert_eq!(d.sketch.entity_count(), 0, "rest of the box removed");
    }
    let r = regen(&ps, &units);
    assert!(r.display.sketches[0].regions.is_empty());
}

#[test]
fn sketch_on_a_face_lies_in_the_face_plane() {
    let (mut ps, units) = block();
    let r = regen(&ps, &units);
    let cap = r.context.bodies[0].faces.iter().find(|n| format!("{n:?}").contains("CapEnd")).expect("top cap face").clone();
    let mut s2 = ps.new_feature("sketch", &units).unwrap();
    s2.params.set("sketchPlane", ParamValue::Query(vec![Reference::Face((*cap).clone())]));
    let id = s2.id;
    ps.features.push(s2);
    let r = regen(&ps, &units);
    let f = r.context.sketches.iter().find(|s| s.feature == id).unwrap().frame;
    assert!((f.origin.z - 0.025).abs() < 1e-12, "sketch origin on the top face: {:?}", f.origin);
    assert!(f.origin.x.abs() < 1e-12 && f.origin.y.abs() < 1e-12, "world origin projected onto the face: {:?}", f.origin);
}

#[test]
fn text_style_edit_regenerates_with_a_cached_regenerator() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut data = SketchData::default();
    let tid = data.add_text("Hi", "Liberation Sans", false, false, DVec2::new(0.0, 0.0), 0.01).unwrap();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(data)));
    ps.features.push(sketch);
    let mut reg = Regenerator::new();
    let width = |r: &noshape_model::regen::RegenResult| -> f64 {
        let s = &r.context.sketches[0].sketch;
        let ids: Vec<EntityId> = s.entities().filter(|e| noshape_sketch::text::glyph_text_id(e.id).is_some() && e.kind() != noshape_sketch::EntityKind::Point).map(|e| e.id).collect();
        s.entities_bbox(&ids).size().x
    };
    let r1 = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let w1 = width(&r1);
    if let Some(ParamValue::Sketch(d)) = ps.features[0].params.0.get_mut("sketch") {
        d.update_text(tid, |t| t.bold = true).unwrap();
        d.sketch.solve();
    }
    let r2 = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let w2 = width(&r2);
    assert!(w2 > w1 * 1.02, "bold glyphs are wider after regen: {w1} -> {w2}");
}
