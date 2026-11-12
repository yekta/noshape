//! Bodies made by splitting one (Split, Boolean subtract into pieces, patterns, mirror, ...) name their faces,
//! edges and vertices apart, as Parasolid does: a reference picked on one piece resolves on that piece only
//! (user P0 11-02: a Chamfer of one Split piece's top face chamfered all three pieces).

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::collections::BTreeMap;

struct T {
    ps: PartStudio,
    units: UnitPrefs,
}

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

impl T {
    fn new() -> Self {
        T { ps: PartStudio::new(), units: UnitPrefs::default() }
    }
    fn regen(&self) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap();
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }
    fn ok(&self) -> RegenResult {
        let r = self.regen();
        for (i, s) in r.display.features.iter().enumerate() {
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Info), "feature {i} ({}) {:?}: {:?}", self.ps.features[i].type_id, s.status, s.message);
        }
        r
    }
    fn plane(&mut self, from: DatumPlane, d: &str) -> Reference {
        Reference::FeaturePlane(self.add("cPlane", &[("entities", q(vec![Reference::Plane(from)])), ("cplaneType", en("OFFSET")), ("offset", e(d))]))
    }
    /// A 60 × 40 × 20 block (mm) on Top.
    fn block(&mut self) {
        let mut sk = noshape_sketch::Sketch::new();
        let p: Vec<_> = [(0.0, 0.0), (0.06, 0.0), (0.06, 0.04), (0.0, 0.04)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
        for i in 0..4 {
            sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
        }
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen();
        let reg = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone();
        self.add("extrude", &[("entities", q(vec![Reference::SketchRegion { sketch: s, key: reg }])), ("depth", e("20 mm"))]);
    }
    /// The block split by two planes `from` offset by a and b: three parts.
    fn split3(&mut self, from: DatumPlane, a: &str, b: &str) {
        self.block();
        let (pa, pb) = (self.plane(from, a), self.plane(from, b));
        for p in [pa, pb] {
            let r = self.regen();
            let bodies = r.context.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
            self.add("splitPart", &[("targets", q(bodies)), ("tool", q(vec![p]))]);
        }
    }
}

/// Face, edge and vertex names that more than one body carries.
fn shared_names(r: &RegenResult) -> Vec<String> {
    let mut owners: BTreeMap<String, std::collections::BTreeSet<usize>> = BTreeMap::new();
    for (bi, b) in r.context.bodies.iter().enumerate() {
        for n in b.faces.iter().chain(b.edges.iter()).chain(b.vertices.iter()) {
            owners.entry(format!("{n:?}")).or_default().insert(bi);
        }
    }
    owners.into_iter().filter(|(_, s)| s.len() > 1).map(|(n, s)| format!("{s:?} {}", n.chars().take(160).collect::<String>())).collect()
}

#[test]
fn split_pieces_name_apart() {
    for (from, a, b) in [(DatumPlane::Right, "20 mm", "40 mm"), (DatumPlane::Top, "5 mm", "12 mm")] {
        let mut t = T::new();
        t.split3(from, a, b);
        let r = t.ok();
        assert_eq!(r.context.bodies.len(), 3);
        let shared = shared_names(&r);
        assert!(shared.is_empty(), "{from:?}: names on more than one part:\n{}", shared.join("\n"));
    }
}

/// The stacked split (cuts at z 5 and 12): the middle part's top face (the cut at z 12, which the top part's bottom
/// shares) as a reference, and each part's volume.
fn middle_top() -> (T, Reference, Vec<f64>) {
    let mut t = T::new();
    t.split3(DatumPlane::Top, "5 mm", "12 mm");
    let r = t.ok();
    let mut found = None;
    for b in &r.context.bodies {
        for (i, f) in b.shape.faces().iter().enumerate() {
            let c = f.centroid().unwrap();
            let n = f.normal_at_point(c).unwrap();
            if (c.z - 0.012).abs() < 1e-9 && n.z > 0.5 {
                found = Some(Reference::Face((*b.faces[i]).clone()));
            }
        }
    }
    let vols = r.context.bodies.iter().map(|b| b.shape.volume().unwrap()).collect();
    (t, found.expect("the middle part's top face"), vols)
}

fn changed(before: &[f64], r: &RegenResult) -> Vec<usize> {
    r.context.bodies.iter().enumerate().filter(|(i, b)| (b.shape.volume().unwrap() - before[*i]).abs() > 1e-12).map(|(i, _)| i).collect()
}

/// User P0 11-02: Chamfer "Face of Split 1" on one piece changed every piece.
#[test]
fn a_chamfer_of_one_split_piece_s_face_changes_that_piece_only() {
    let (mut t, face, before) = middle_top();
    t.add("chamfer", &[("entities", q(vec![face])), ("chamferType", en("EQUAL_OFFSETS")), ("width", e("0.8 mm")), ("tangentPropagation", ParamValue::Bool(true))]);
    let r = t.ok();
    assert_eq!(changed(&before, &r).len(), 1, "parts changed: {:?}", changed(&before, &r));
}

#[test]
fn a_fillet_of_one_split_piece_s_face_changes_that_piece_only() {
    let (mut t, face, before) = middle_top();
    t.add("fillet", &[("entities", q(vec![face])), ("radius", e("0.8 mm")), ("tangentPropagation", ParamValue::Bool(true))]);
    let r = t.ok();
    assert_eq!(changed(&before, &r).len(), 1, "parts changed: {:?}", changed(&before, &r));
}

/// A sketch on one piece's cut face sits on that face, facing out of that piece (+z), not on its twin (-z).
#[test]
fn a_sketch_on_one_split_piece_s_face_takes_that_face() {
    let (mut t, face, _) = middle_top();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(0.02, 0.02), 0.005);
    let s = t.add("sketch", &[("sketchPlane", q(vec![face])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.ok();
    let d = r.display.sketches.iter().find(|x| x.feature == s).unwrap();
    assert!((d.frame.origin.z - 0.012).abs() < 1e-9 && d.frame.normal.z > 0.999, "{:?}", d.frame);
}

/// A reference saved before the pieces were named apart (the name the pieces shared) still finds both faces, as it
/// did: old documents regenerate as before.
#[test]
fn a_reference_to_the_shared_name_still_finds_both_faces() {
    let (t, face, _) = middle_top();
    let Reference::Face(n) = face else { unreachable!() };
    let noshape_model::reference::TopoName::Split { parent, .. } = &n else { panic!("not named apart: {n:?}") };
    let r = t.ok();
    let hits: usize = r.context.bodies.iter().map(|b| b.find_faces(parent).len()).sum();
    assert_eq!(hits, 2);
}

/// Every op that makes several bodies of one names them apart: a part pattern, a part mirror, a subtraction that cuts a
/// block in two, a split by another part.
#[test]
fn pieces_of_other_ops_name_apart() {
    let body0 = |t: &T| Reference::Body(t.regen().context.bodies[0].id.clone());
    let mut cases: Vec<(&str, T)> = Vec::new();
    // Part pattern ×3 along x.
    let mut t = T::new();
    t.block();
    let b = body0(&t);
    let r = t.regen();
    let edge = r.context.bodies[0].edges.iter().enumerate().find(|(i, _)| {
        let inf = r.context.bodies[0].shape.edges()[*i].edge_info().unwrap();
        (inf.end - inf.start).normalize().x.abs() > 0.999
    }).map(|(_, n)| Reference::Edge((**n).clone())).unwrap();
    t.add("linearPattern", &[("patternType", en("PART")), ("entities", q(vec![b])), ("directionOne", q(vec![edge])), ("distance", e("80 mm")), ("instanceCount", e("3"))]);
    cases.push(("part pattern", t));
    // Part mirror (New).
    let mut t = T::new();
    t.block();
    let b = body0(&t);
    t.add("mirror", &[("patternType", en("PART")), ("entities", q(vec![b])), ("mirrorPlane", q(vec![Reference::Plane(DatumPlane::Right)])), ("operationType", en("NEW"))]);
    cases.push(("part mirror", t));
    // A slab through the block subtracted: two pieces.
    let mut t = T::new();
    t.block();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.028, -0.01), (0.032, -0.01), (0.032, 0.05), (0.028, 0.05)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let reg = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone();
    let target = Reference::Body(r.context.bodies[0].id.clone());
    t.add("extrude", &[("entities", q(vec![Reference::SketchRegion { sketch: s, key: reg }])), ("depth", e("30 mm")), ("operationType", en("REMOVE")), ("booleanScope", q(vec![target]))]);
    cases.push(("a subtraction in two", t));
    // Split by part: the block split by a thin box through it (kept).
    let mut t = T::new();
    t.split3(DatumPlane::Right, "15 mm", "45 mm");
    cases.push(("split twice by planes", t));
    let mut report = String::new();
    for (name, t) in cases {
        let r = t.ok();
        let shared = shared_names(&r);
        if r.context.bodies.len() < 2 {
            report += &format!("{name}: {} bodies\n", r.context.bodies.len());
        }
        if !shared.is_empty() {
            report += &format!("{name}:\n{}\n", shared.join("\n"));
        }
    }
    assert!(report.is_empty(), "{report}");
}

/// The other features that take a face, on one piece's cut face: Shell (removing it), Extrude up to it, a mate
/// connector on it, Draft with it as the neutral plane.
#[test]
fn other_features_take_one_split_piece_s_face() {
    // Shell 1 mm removing the middle part's top face: only that part changes.
    let (mut t, face, before) = middle_top();
    t.add("shell", &[("entities", q(vec![face.clone()])), ("thickness", e("1 mm"))]);
    let r = t.ok();
    assert_eq!(changed(&before, &r).len(), 1, "shell: parts changed {:?}", changed(&before, &r));
    // A mate connector at the face's centroid: z 12, its z up.
    let (mut t, face, _) = middle_top();
    t.add("mateConnector", &[("originQuery", q(vec![face.clone()])), ("entityInferenceType", en("CENTROID"))]);
    let r = t.ok();
    let c = &r.context.connectors[0];
    assert!((c.origin.z - 0.012).abs() < 1e-9 && c.z.z > 0.999, "connector {:?} z {:?}", c.origin, c.z);
    // Draft the middle part's four sides 5° from that face: only that part changes.
    let (mut t, face, before) = middle_top();
    let r = t.regen();
    let mut sides = Vec::new();
    for b in &r.context.bodies {
        for (i, f) in b.shape.faces().iter().enumerate() {
            let c = f.centroid().unwrap();
            let n = f.normal_at_point(c).unwrap();
            if n.z.abs() < 1e-9 && c.z > 0.005 && c.z < 0.012 {
                sides.push(Reference::Face((*b.faces[i]).clone()));
            }
        }
    }
    assert_eq!(sides.len(), 4);
    t.add("draft", &[("neutralPlane", q(vec![face.clone()])), ("draftFaces", q(sides)), ("angle", e("5 deg"))]);
    let r = t.ok();
    assert_eq!(changed(&before, &r).len(), 1, "draft: parts changed {:?}", changed(&before, &r));
    // Extrude a circle on Top up to that face: the new part ends at z 12 (its twin, the top part's bottom, is there
    // too; the reference must find one face, not fail as ambiguous).
    let (mut t, face, _) = middle_top();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(0.02, 0.02), 0.005);
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let reg = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone();
    t.add("extrude", &[("entities", q(vec![Reference::SketchRegion { sketch: s, key: reg }])), ("endBound", en("UP_TO_SURFACE")), ("endBoundEntityFace", q(vec![face]))]);
    let r = t.ok();
    let new = r.context.bodies.last().unwrap();
    let bb = new.shape.bounding_box(true).unwrap();
    assert!((bb.max.z - 0.012).abs() < 1e-7, "{bb:?}");
}

/// SPLIT-FACE-SAME-ID (the user's Gyro, lead P0 11-04): three concentric rings (one Revolve of three rectangles) split
/// by one plane: each ring's cut face is its own, so a Chamfer of the inner ring's chamfers that ring only (it loses
/// π d² (r1 + r2), its two rims' triangles revolved) and the other two keep their volumes; also with both sides
/// kept (six parts).
#[test]
fn one_split_through_several_bodies_names_each_cut_face_apart() {
    use std::f64::consts::PI;
    for both in [false, true] {
        let mut t = T::new();
        let mut sk = noshape_sketch::Sketch::new();
        for (a, b) in [(3.0, 5.0), (7.0, 9.0), (11.0, 13.0)] {
            let p: Vec<_> = [(a, -5.0), (b, -5.0), (b, 5.0), (a, 5.0)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
            for i in 0..4 {
                sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
            }
        }
        let (a0, a1) = (sk.add_point(DVec2::new(0.0, -0.006)), sk.add_point(DVec2::new(0.0, 0.006)));
        let axis = sk.add_line_between(a0, a1).unwrap();
        sk.set_construction(axis, true).unwrap();
        let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Front)])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = t.regen();
        let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
        assert_eq!(regions.len(), 3);
        t.add("revolve", &[("entities", q(regions)), ("axis", q(vec![Reference::SketchEntity { sketch: s, entity: axis.0 }])), ("revolveType", en("FULL"))]);
        let r = t.ok();
        assert_eq!(r.context.bodies.len(), 3);
        let bodies = r.context.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
        t.add("splitPart", &[("targets", q(bodies)), ("tool", q(vec![Reference::Plane(DatumPlane::Top)])), ("keepBothSides", ParamValue::Bool(both))]);
        let r = t.ok();
        assert_eq!(r.context.bodies.len(), if both { 6 } else { 3 });
        let volumes = |r: &RegenResult| r.context.bodies.iter().map(|b| (b.id.clone(), b.shape.volume().unwrap() * 1e9)).collect::<BTreeMap<_, _>>();
        let before = volumes(&r);
        // The cut faces (z = 0): each part's own name; the inner ring's is the smallest.
        let mut cut: Vec<(f64, noshape_model::reference::BodyId, Reference)> = Vec::new();
        for b in r.context.bodies.iter() {
            for (i, n) in b.faces.iter().enumerate() {
                let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
                if f.is_planar() && f.centroid().unwrap().z.abs() < 1e-9 {
                    cut.push((f.area().unwrap() * 1e6, b.id.clone(), Reference::Face((**n).clone())));
                }
            }
        }
        assert_eq!(cut.len(), if both { 6 } else { 3 });
        let names: std::collections::HashSet<String> = cut.iter().map(|c| format!("{:?}", c.2)).collect();
        assert_eq!(names.len(), cut.len(), "every part's cut face named apart");
        cut.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (_, inner, face) = cut[0].clone();
        t.add("chamfer", &[("entities", q(vec![face])), ("width", e("0.5 mm")), ("tangentPropagation", ParamValue::Bool(true))]);
        let r = t.ok();
        let after = volumes(&r);
        let lost = PI * 0.25 * (3.0 + 5.0);
        for (id, v) in &after {
            let want = if *id == inner { before[id] - lost } else { before[id] };
            assert!((v - want).abs() < 1e-6 * want, "both sides {both}: {id:?} {v} mm³, want {want} (the chamfered one {inner:?})");
        }
    }
}

/// The same with one Remove extrude through three separate blocks: each block's hole wall is its own face.
#[test]
fn one_remove_through_several_bodies_names_each_wall_apart() {
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    for x0 in [0.0, 0.03, 0.06] {
        let p: Vec<_> = [(x0, 0.0), (x0 + 0.02, 0.0), (x0 + 0.02, 0.02), (x0, 0.02)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
        for i in 0..4 {
            sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
        }
    }
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
    t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm"))]);
    // A slot across all three, cut through.
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(-0.005, 0.008), (0.085, 0.008), (0.085, 0.012), (-0.005, 0.012)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
    let s2 = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.ok();
    assert_eq!(r.context.bodies.len(), 3);
    let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s2).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s2, key: g.key.clone() }).collect();
    let bodies = r.context.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
    t.add("extrude", &[("entities", q(regions)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(false)), ("booleanScope", q(bodies))]);
    let r = t.ok();
    let mut names = std::collections::HashMap::<String, usize>::new();
    for b in r.context.bodies.iter() {
        for n in b.faces.iter() {
            *names.entry(format!("{n:?}")).or_default() += 1;
        }
    }
    let shared: Vec<_> = names.iter().filter(|(_, c)| **c > 1).collect();
    assert!(shared.is_empty(), "{} parts, names on several: {shared:?}", r.context.bodies.len());
}

/// Live ui-parity split-new-edit-reedit: with one side kept, a part the tool doesn't cut stays or goes by the side it
/// lies on (Part 1 wholly behind Part 2's top face was deleted once Keep both sides was off); with both sides kept it
/// stays, with the info that nothing was split.
#[test]
fn a_one_sided_split_keeps_or_drops_a_part_it_doesnt_cut_by_its_side() {
    for (both, front, parts, info) in [(true, true, 1, true), (false, true, 0, false), (false, false, 1, false)] {
        let mut t = T::new();
        t.block();
        let above = t.plane(DatumPlane::Top, "30 mm");
        let r = t.ok();
        let body = Reference::Body(r.context.bodies[0].id.clone());
        t.add("splitPart", &[("targets", q(vec![body])), ("tool", q(vec![above])), ("keepBothSides", ParamValue::Bool(both)), ("keepFront", ParamValue::Bool(front))]);
        let r = t.ok();
        assert_eq!(r.context.bodies.len(), parts, "both sides {both}, front {front}");
        let split = r.display.features.last().unwrap();
        let nothing = split.message.as_deref().is_some_and(|m| m.contains("does not split"));
        assert_eq!(nothing, info, "both sides {both}, front {front}: {:?}", split.message);
    }
}
