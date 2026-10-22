//! SK5 through regeneration (qa5, app on 3467ff9): the sketch's fillet
//! dimension is driven by its expression ("3 mm" typed into the box) and the
//! feature regenerates from the R5 geometry. Live (qa5 sketch tools):
//! 30.089 × 22.065, top-right R5 → R3 keeps the top at 25.089 and the height,
//! only the far edge moves (bottom 28.089, right 19.065); R8: 33.089 / 14.065.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_sketch::{ConstraintKind, EntityKind};
use noshape_units::{ExprValue, UnitPrefs};

#[test]
fn fillet_expression_change_moves_like_onshape() {
    for (expr, bottom, right) in [("3 mm", 28.089, 19.065), ("8 mm", 33.089, 14.065)] {
        let units = UnitPrefs::default();
        let mut sk = noshape_sketch::Sketch::new();
        let r = sk.corner_rectangle(DVec2::ZERO, DVec2::new(0.030089, 0.022065));
        let res = sk.fillet_corner(r.points[2], 0.005).unwrap();
        let arc = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Arc)).unwrap();
        assert!(sk.solve().is_solved());
        let cid = sk.constraints().find(|c| matches!(c.kind, ConstraintKind::Radius { curve, .. } if curve == arc)).unwrap().id;
        let mut data = SketchData { sketch: sk, ..Default::default() };
        data.dimension_exprs.insert(cid.0, ExprValue::new(expr));
        let mut ps = PartStudio::new();
        let mut f = ps.new_feature("sketch", &units).unwrap();
        f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        f.params.set("sketch", ParamValue::Sketch(Box::new(data)));
        ps.features.push(f);
        let out = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
        let solved = &out.context.sketches[0].sketch;
        let len = |l| {
            let c = solved.curve(l).unwrap();
            ((c.end_point() - c.start_point()).length() * 1e6).round() / 1e3
        };
        let [b, rt, t, l] = [r.curves[0], r.curves[1], r.curves[2], r.curves[3]];
        assert_eq!([len(t), len(b), len(l), len(rt)], [25.089, bottom, 22.065, right], "{expr}");
    }
}
