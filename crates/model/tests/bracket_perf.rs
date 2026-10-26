//! qa5 PERF2 / live "qa5 bracket": an 80 × 50 × 8 base, a wall, two holes,
//! fillets, a chamfer, a patterned wall hole, a gusset and its mirror
//! (docs/parity/workflow-bracket.md). Onshape regenerates the whole Part
//! Studio in 77 ms and the depth edit 8 → 10 in 70 ms; the geometry is
//! Onshape's (67191.3275328646 mm³, 20 faces; 73262.5554193746 at 10 mm).

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
    reg: Regenerator,
}

impl Studio {
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap();
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }
    fn regen(&mut self) -> (RegenResult, f64) {
        let t0 = std::time::Instant::now();
        let r = self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default());
        (r, t0.elapsed().as_secs_f64())
    }
    /// A sketch of closed polylines and circles (mm, sketch coordinates); its regions, smallest first.
    fn sketch(&mut self, plane: DatumPlane, polys: &[&[(f64, f64)]], circles: &[((f64, f64), f64)]) -> Vec<Reference> {
        let mut sk = noshape_sketch::Sketch::new();
        for poly in polys {
            let pts: Vec<_> = poly.iter().map(|(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
            for i in 0..pts.len() {
                sk.add_line_between(pts[i], pts[(i + 1) % pts.len()]).unwrap();
            }
        }
        for ((x, y), r) in circles {
            sk.add_circle(DVec2::new(x * 1e-3, y * 1e-3), r * 1e-3);
        }
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(plane)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen().0;
        let mut regions: Vec<_> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| (g.area, Reference::SketchRegion { sketch: s, key: g.key.clone() })).collect();
        regions.sort_by(|a, b| a.0.total_cmp(&b.0));
        regions.into_iter().map(|x| x.1).collect()
    }
    /// The part's edges whose midpoint is `p` (mm).
    fn edges_at(&mut self, points: &[DVec3]) -> Vec<Reference> {
        let r = self.regen().0;
        let b = r.context.bodies.iter().find(|b| b.kind == BodyKind::Solid).unwrap();
        let mut out = Vec::new();
        for (i, x) in b.shape.edges().iter().enumerate() {
            let Ok(info) = x.edge_info() else { continue };
            let Ok(m) = x.point_at(0.5 * (info.first_param + info.last_param)) else { continue };
            if points.iter().any(|p| (m * 1e3 - *p).length() < 1e-3) {
                out.push(Reference::Edge((*b.edges[i]).clone()));
            }
        }
        out
    }
}

fn part(r: &RegenResult) -> (f64, usize) {
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
    assert_eq!(solids.len(), 1);
    (solids[0].shape.volume().unwrap() * 1e9, solids[0].shape.count(noshape_kernel::ShapeType::Face))
}

#[test]
#[ignore = "T2: big model / perf"]
fn bracket_regenerates_like_onshape() {
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new() };
    let base = s.sketch(DatumPlane::Top, &[&[(0.0, 0.0), (80.0, 0.0), (80.0, 50.0), (0.0, 50.0)]], &[]);
    let x1 = s.add("extrude", &[("entities", q(base)), ("depth", e("8 mm"))]);
    let wall = s.sketch(DatumPlane::Top, &[&[(0.0, 0.0), (80.0, 0.0), (80.0, 8.0), (0.0, 8.0)]], &[]);
    s.add("extrude", &[("entities", q(wall)), ("depth", e("60 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let holes = s.sketch(DatumPlane::Top, &[], &[((20.0, 30.0), 4.0), ((60.0, 30.0), 4.0)]);
    s.add("extrude", &[("entities", q(holes[..2].to_vec())), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(true)), ("endBound", en("THROUGH_ALL"))]);
    let f = s.edges_at(&[DVec3::new(0.0, 50.0, 4.0), DVec3::new(80.0, 50.0, 4.0)]);
    assert_eq!(f.len(), 2);
    s.add("fillet", &[("entities", q(f)), ("radius", e("5 mm"))]);
    let c = s.edges_at(&[DVec3::new(40.0, 0.0, 60.0)]);
    assert_eq!(c.len(), 1);
    s.add("chamfer", &[("entities", q(c)), ("width", e("1 mm"))]);
    let wh = s.sketch(DatumPlane::Front, &[], &[((15.0, 45.0), 3.0)]);
    let hole = s.add("extrude", &[("entities", q(wh[..1].to_vec())), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(true)), ("endBound", en("THROUGH_ALL")), ("oppositeDirection", ParamValue::Bool(true))]);
    // The base's back edge runs −x (its sketch line's way): live's pattern
    // failed until Opposite direction (behaviours.md "qa5 bracket" †).
    let dir = s.edges_at(&[DVec3::new(40.0, 50.0, 0.0)]);
    assert_eq!(dir.len(), 1);
    s.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(hole)])), ("directionOne", q(dir)), ("distance", e("25 mm")), ("instanceCount", e("3")), ("oppositeDirection", ParamValue::Bool(true))]);
    let g = s.sketch(DatumPlane::Right, &[&[(8.0, 8.0), (30.0, 8.0), (8.0, 40.0)]], &[]);
    let gusset = s.add("extrude", &[("entities", q(g[..1].to_vec())), ("depth", e("5 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let pl = s.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Right)])), ("offset", e("40 mm"))]);
    s.add("mirror", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(gusset)])), ("mirrorPlane", q(vec![Reference::FeaturePlane(pl)])), ("fullFeaturePattern", ParamValue::Bool(true))]);
    // The whole Part Studio from scratch, as on opening the document.
    s.reg = Regenerator::new();
    let (r, t_open) = s.regen();
    for (i, st) in r.display.features.iter().enumerate() {
        assert!(matches!(st.status, FeatureStatus::Ok), "feature {i} {:?}: {:?}", st.status, st.message);
    }
    let (v, faces) = part(&r);
    assert!((v - 67191.3275328646).abs() < 1e-3, "{v}");
    assert_eq!(faces, 20);
    let times: Vec<String> = r.display.features.iter().zip(&s.ps.features).map(|(st, f)| format!("{} {:.1}", f.type_id, st.time_ms)).collect();
    s.ps.feature_mut(x1).unwrap().params.set("depth", e("10 mm"));
    let (r, t_edit) = s.regen();
    let (v, _) = part(&r);
    assert!((v - 73262.5554193746).abs() < 1e-3, "{v}");
    eprintln!("bracket: open {:.1} ms, depth edit {:.1} ms (Onshape 77 / 70 ms); {}", t_open * 1e3, t_edit * 1e3, times.join(", "));
}
