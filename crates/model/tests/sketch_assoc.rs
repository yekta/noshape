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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
    s2.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(data)));
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
        let d = std::sync::Arc::make_mut(d);
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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(data)));
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
        let d = std::sync::Arc::make_mut(d);
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
        let d = std::sync::Arc::make_mut(d);
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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(data)));
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
        let d = std::sync::Arc::make_mut(d);
        d.update_text(tid, |t| t.bold = true).unwrap();
        d.sketch.solve();
    }
    let r2 = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let w2 = width(&r2);
    assert!(w2 > w1 * 1.02, "bold glyphs are wider after regen: {w1} -> {w2}");
}

/// An empty sketch on a frame's face (a 60 x 40 mm plate with a 40 x 20 mm opening, 10 mm): the face and the
/// opening's inside are both sketch faces, as live's (asm-robotic-arm's Sketch 3: JPG the frame's face, JPC inside
/// it, which its Extrude 4 picks), neither of them qSketchRegion's.
#[test]
fn a_sketch_on_a_face_with_a_hole_has_the_holes_inside_too() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.corner_rectangle(DVec2::ZERO, DVec2::new(0.06, 0.04));
    sk.corner_rectangle(DVec2::new(0.01, 0.01), DVec2::new(0.05, 0.03));
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    let sketch_id = sketch.id;
    ps.features.push(sketch);
    let regions = regen(&ps, &units).display.sketches[0].regions.clone();
    let ring = regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("the frame's region").key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sketch_id, key: ring }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("10 mm")));
    ps.features.push(ex);
    let r = regen(&ps, &units);
    let top = r.context.bodies[0].faces.iter().find(|n| format!("{n:?}").contains("CapEnd")).expect("top cap face").clone();
    let mut f = ps.new_feature("sketch", &units).unwrap();
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Face((*top).clone())]));
    f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: noshape_sketch::Sketch::new(), ..Default::default() })));
    let id = f.id;
    ps.features.push(f);
    let r = regen(&ps, &units);
    let s = r.context.sketches.iter().find(|s| s.feature == id).unwrap();
    let mut areas: Vec<f64> = s.regions.iter().map(|g| g.area() * 1e6).collect();
    areas.sort_by(f64::total_cmp);
    assert!(areas.len() == 2 && (areas[0] - 800.0).abs() < 1e-6 && (areas[1] - 1600.0).abs() < 1e-6, "{areas:?}");
    assert!(s.imprinted.iter().all(|i| *i), "{:?}", s.imprinted);
}

/// A sketch on a face imprints the face's edges, as live 11-03 (core's probe in "live 1812 drawing scale", a 60 x 40 x
/// 10 mm block from (0, 0) on Top, sketches on its top face): the face's pieces are sketch faces, the sketch's own
/// regions are split by the face's edges, and only those are qSketchRegion's; off the face, what the sketch's curves
/// close with its edges is a sketch face too; Disable imprinting leaves the sketch's own. A region picked before it was split still extrudes whole.
#[test]
fn a_sketch_on_a_face_imprints_the_faces_edges() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.corner_rectangle(DVec2::ZERO, DVec2::new(0.06, 0.04));
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    let sketch_id = sketch.id;
    ps.features.push(sketch);
    let key = regen(&ps, &units).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sketch_id, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("10 mm")));
    ps.features.push(ex);
    let r = regen(&ps, &units);
    let top = r.context.bodies[0].faces.iter().find(|n| format!("{n:?}").contains("CapEnd")).expect("top cap face").clone();
    let mm = |x: f64, y: f64| DVec2::new(x, y) * 1e-3;
    // (live: sketch faces' areas, mm², and how many are qSketchRegion's)
    type Case = (&'static str, fn(&mut noshape_sketch::Sketch, &dyn Fn(f64, f64) -> DVec2), bool, &'static [f64], usize);
    let cases: [Case; 8] = [
        ("circle inside", |s, mm| { s.add_circle(mm(20.0, 20.0), 0.005); }, false, &[78.5398, 2321.4602], 1),
        // Off the face, two lines closing a triangle with its edge (live: c2-duct-multipart's Sketch 5, JVC / JVG).
        ("lines closed by an edge", |s, mm| { s.add_line(mm(0.0, 40.0), mm(0.0, 60.0)); s.add_line(mm(0.0, 60.0), mm(20.0, 40.0)); }, false, &[200.0, 2400.0], 0),
        ("line across", |s, mm| { s.add_line(mm(30.0, -10.0), mm(30.0, 50.0)); }, false, &[1200.0, 1200.0], 0),
        ("line from edge to edge", |s, mm| { s.add_line(mm(30.0, 0.0), mm(30.0, 40.0)); }, false, &[1200.0, 1200.0], 0),
        ("circle over an edge", |s, mm| { s.add_circle(mm(0.0, 20.0), 0.008); }, false, &[100.531, 100.531, 2299.469], 2),
        ("line inside", |s, mm| { s.add_line(mm(10.0, 10.0), mm(50.0, 30.0)); }, false, &[2400.0], 0),
        ("corner cut", |s, mm| { s.add_line(mm(0.0, 30.0), mm(10.0, 40.0)); }, false, &[50.0, 2350.0], 0),
        ("circle inside, Disable imprinting", |s, mm| { s.add_circle(mm(20.0, 20.0), 0.005); }, true, &[78.5398], 1),
    ];
    for (what, draw, disable, areas, regions) in cases {
        let mut s = noshape_sketch::Sketch::new();
        draw(&mut s, &mm);
        let mut f = ps.new_feature("sketch", &units).unwrap();
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Face((*top).clone())]));
        f.params.set("disableImprinting", ParamValue::Bool(disable));
        f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: s, ..Default::default() })));
        let id = f.id;
        let mut p = ps.clone();
        p.features.push(f);
        let r = regen(&p, &units);
        let s = r.context.sketches.iter().find(|s| s.feature == id).unwrap();
        let mut ours: Vec<f64> = s.regions.iter().map(|g| g.area() * 1e6).collect();
        ours.sort_by(f64::total_cmp);
        assert_eq!(ours.len(), areas.len(), "{what}: {ours:?}");
        for (a, b) in ours.iter().zip(areas) {
            assert!((a - b).abs() < 1e-3, "{what}: {ours:?}, live {areas:?}");
        }
        let own = (0..s.regions.len()).filter(|&i| !s.imprinted.get(i).copied().unwrap_or(false)).count();
        assert_eq!(own, regions, "{what}: qSketchRegion's");
    }
    // On another sketch's region (live 1747, "Face of Sketch 1"): its boundary is imprinted the same way.
    let mut s = noshape_sketch::Sketch::new();
    s.add_line(mm(30.0, -10.0), mm(30.0, 50.0));
    let mut f = ps.new_feature("sketch", &units).unwrap();
    let block_region = match ps.features[1].params.0.get("entities") {
        Some(ParamValue::Query(q)) => q[0].clone(),
        _ => unreachable!(),
    };
    f.params.set("sketchPlane", ParamValue::Query(vec![block_region]));
    f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: s, ..Default::default() })));
    let id = f.id;
    let mut p = ps.clone();
    p.features.push(f);
    let r = regen(&p, &units);
    let s = r.context.sketches.iter().find(|s| s.feature == id).unwrap();
    assert_eq!(s.regions.iter().map(|g| (g.area() * 1e6).round()).collect::<Vec<_>>(), [1200.0, 1200.0], "on a sketch region");
    // The circle over the edge, its whole region picked before imprinting: 8 mm of it, half of it over the edge.
    let mut s = noshape_sketch::Sketch::new();
    let c = s.add_circle(mm(0.0, 20.0), 0.008);
    let whole = noshape_model::regen::eval::encode_region_key(&s.regions()[0].id);
    assert!(s.regions()[0].entities() == vec![c]);
    let mut f = ps.new_feature("sketch", &units).unwrap();
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Face((*top).clone())]));
    f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: s, ..Default::default() })));
    let id = f.id;
    ps.features.push(f);
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: id, key: whole }]));
    ex.params.set("operationType", ParamValue::Enum("NEW".into()));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("5 mm")));
    ps.features.push(ex);
    let r = regen(&ps, &units);
    let v = r.context.bodies.iter().map(|b| b.shape.volume().unwrap() * 1e9).find(|v| (v - 24000.0).abs() > 1.0).expect("the new part");
    assert!((v - std::f64::consts::PI * 64.0 * 5.0).abs() < 1e-3, "the whole circle: {v} mm³");
}

