//! Creating a drawing (Onshape's Create drawing dialog): a template sets the
//! sheet, title block, units and projection angle; "Suggested views" adds
//! the front, top, right and isometric views of the source, scaled to fit
//! the sheet and laid out for the projection angle.

use super::templates::{DrawingTemplate, apply_template};
use super::views::{SourceKey, regenerate_source};
use super::{Drawing, DrawingView, Projection, RenderStyle, ViewId, ViewKind, ViewOrientation};
use crate::assembly::eval::AssemblyEvaluator;
use crate::configuration::ConfigValues;
use crate::document::{Document, TabId};
use crate::reference::BodyId;
use crate::regen::Regenerator;
use glam::DVec3;

/// What the drawing shows: a Part Studio (all parts, or one) or an assembly.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingSource {
    pub tab: TabId,
    pub part: Option<BodyId>,
    pub config: ConfigValues,
    /// A sketch of the Part Studio ("Create Drawing of Sketch N…").
    pub sketch: Option<crate::FeatureId>,
    /// The part's flat pattern (the sheet metal flat view's "Create Drawing
    /// of Flat pattern of Part N…").
    pub flat: bool,
}

/// The scales Suggested views picks from, largest first (view scale =
/// paper / model): live's list, 100:1 … 2:1, 1:1, 1:2 … 1:100 (no 3:4,
/// 1.5, 2.5 or 0.4; qa DRAWING-SCALE).
const SCALES: [f64; 30] = [
    100.0, 50.0, 32.0, 20.0, 16.0, 12.0, 10.0, 8.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0,
    1.0 / 2.0, 1.0 / 3.0, 1.0 / 4.0, 1.0 / 5.0, 1.0 / 6.0, 1.0 / 8.0, 1.0 / 10.0, 1.0 / 12.0, 1.0 / 16.0, 1.0 / 20.0, 1.0 / 25.0, 1.0 / 32.0, 1.0 / 40.0, 1.0 / 50.0, 1.0 / 80.0, 1.0 / 100.0,
];

/// Model bounds (meters) of a drawing source, regenerated from the document.
pub fn source_bounds(doc: &Document, src: &DrawingSource) -> Option<(DVec3, DVec3)> {
    let key = SourceKey { tab: src.tab, config: src.config.clone(), explode: None, version: None, position: None, display: None };
    let model = regenerate_source(doc, &key, &mut Regenerator::new(), &mut AssemblyEvaluator::new())?;
    let mut acc: Option<(DVec3, DVec3)> = None;
    // A sketch source: the bounds of its curves.
    if let Some(f) = &src.sketch {
        let s = model.context.sketches.iter().find(|s| &s.feature == f)?;
        for e in s.sketch.entities().filter(|e| !e.construction) {
            for p in s.sketch.curve(e.id).map(|c| c.tessellate(1e-4)).unwrap_or_default() {
                let w = s.frame.to_world(p);
                acc = Some(match acc {
                    None => (w, w),
                    Some((lo, hi)) => (lo.min(w), hi.max(w)),
                });
            }
        }
        return acc;
    }
    // A flat pattern: its extent in its own plane (x, y), the thickness in z.
    if src.flat {
        let flat = crate::sheetmetal::flat_pattern_for(&model.context, src.part.as_ref()?).ok()?;
        let pts = flat.loops.iter().flatten();
        let lo = pts.clone().fold(glam::DVec2::splat(f64::INFINITY), |a, p| a.min(*p));
        let hi = pts.fold(glam::DVec2::splat(f64::NEG_INFINITY), |a, p| a.max(*p));
        return Some((lo.extend(0.0), hi.extend(flat.thickness)));
    }
    for b in model.context.bodies.iter().filter(|b| src.part.as_ref().is_none_or(|p| &b.id == p)) {
        let Ok(bb) = b.shape.bounding_box(false) else { continue };
        acc = Some(match acc {
            None => (bb.min, bb.max),
            Some((lo, hi)) => (lo.min(bb.min), hi.max(bb.max)),
        });
    }
    acc
}

/// The largest standard scale at which the suggested views of a model of
/// `extent` (meters, X Y Z) fit a sheet of `w` x `h` millimeters.
pub fn suggested_scale(extent: DVec3, w: f64, h: f64) -> f64 {
    let e = extent.max(DVec3::splat(1e-4)) * 1000.0;
    // Front (X by Z) and top (X by Y) stacked, right (Y by Z) and the
    // isometric view (about the diagonal) beside them; 60% of the sheet
    // leaves room for the title block, gaps and dimensions.
    let iso = (e.x + e.y) * 0.87;
    let need_w = e.x + e.y.max(iso);
    let need_h = e.z + e.y.max(iso * 0.6);
    SCALES.iter().copied().find(|s| need_w * s <= w * 0.6 && need_h * s <= h * 0.6).unwrap_or(*SCALES.last().expect("scales"))
}

/// The view frame of a sketch source (looking at the sketch plane).
pub fn source_frame(doc: &Document, src: &DrawingSource) -> Option<[[f64; 3]; 2]> {
    let f = src.sketch.as_ref()?;
    let key = SourceKey { tab: src.tab, config: src.config.clone(), explode: None, version: None, position: None, display: None };
    let model = regenerate_source(doc, &key, &mut Regenerator::new(), &mut AssemblyEvaluator::new())?;
    super::views::sketch_frame(&model.context, f)
}

/// A new drawing from a template; with `suggested` and a source, the
/// front, top, right and isometric views (labels and scales off); a sketch
/// source gets one view of the sketch, facing its plane (`frame`).
pub fn create_drawing(template: &DrawingTemplate, source: Option<&DrawingSource>, bounds: Option<(DVec3, DVec3)>, suggested: bool, frame: Option<[[f64; 3]; 2]>) -> Drawing {
    let mut d = Drawing::default();
    // Formats › Lock: Unlocked on new drawings (live's ANSI and ISO dumps,
    // dom/drawing-properties-formats-*).
    apply_template(&mut d, 0, template);
    super::templates::template_precisions(&mut d.props, template.dim_units().imperial());
    super::templates::template_sizes(&mut d.props, template.style == super::TitleBlockStyle::Ansi);
    let (Some(src), true) = (source, suggested) else { return d };
    if src.sketch.is_some() {
        let (lo, hi) = bounds.unwrap_or((DVec3::splat(-0.05), DVec3::splat(0.05)));
        let (w, h) = d.sheets[0].size.size_mm();
        let e = (hi - lo).max(DVec3::splat(1e-4)) * 1000.0;
        let ext = e.x.max(e.y).max(e.z);
        d.sheets[0].scale = SCALES.iter().copied().find(|s| ext * s <= w.min(h) * 0.6).unwrap_or(*SCALES.last().expect("scales"));
        d.sheets[0].views.push(DrawingView {
            id: ViewId(1),
            kind: ViewKind::Base { source: src.tab, config: src.config.clone(), orientation: ViewOrientation::Front, part: None },
            position: [w * 0.45, 15.0 + (h - 15.0) * 0.55],
            sketch: src.sketch,
            custom_frame: frame,
            ..Default::default()
        });
        return d;
    }
    let (lo, hi) = bounds.unwrap_or((DVec3::splat(-0.05), DVec3::splat(0.05)));
    let e = (hi - lo) * 1000.0;
    let (w, h) = d.sheets[0].size.size_mm();
    if src.flat {
        return flat_drawing(d, src, e, w, h);
    }
    let s = suggested_scale(hi - lo, w, h);
    d.sheets[0].scale = s;
    let iso = (e.x + e.y) * 0.87 * s;
    // Cell sizes (paper mm): column of the front view, the other column;
    // row of the front view, the other row.
    let (fw, ow) = (e.x * s, (e.y * s).max(iso));
    let (fh, oh) = (e.z * s, (e.y * s).max(iso * 0.6));
    let gap = 25.0;
    let (tw, th) = (fw + gap + ow, fh + gap + oh);
    // Centered in the area above the title block band.
    let (x0, y0) = ((w - tw) * 0.5, 15.0 + ((h - 15.0) - th).max(0.0) * 0.6);
    let third = d.projection == Projection::ThirdAngle;
    // Third angle: front bottom-left, top above, right to the right.
    // First angle: front top-right, top below, right view to the left.
    let (front_x, other_x) = if third { (x0 + fw * 0.5, x0 + fw + gap + ow * 0.5) } else { (x0 + ow + gap + fw * 0.5, x0 + ow * 0.5) };
    let (front_y, other_y) = if third { (y0 + fh * 0.5, y0 + fh + gap + oh * 0.5) } else { (y0 + oh + gap + fh * 0.5, y0 + oh * 0.5) };
    // Automatic centermarks as Drawing properties › Views (live asbdt:
    // the suggested Front view marks its hole); projected views copy the
    // front view's, like the Projected view tool.
    let vd = d.props.view_defaults.clone();
    let base = |id: u64, orientation: ViewOrientation, position: [f64; 2]| DrawingView {
        id: ViewId(id),
        kind: ViewKind::Base { source: src.tab, config: src.config.clone(), orientation, part: src.part.clone() },
        position,
        style: RenderStyle::HiddenLinesRemoved,
        show_centerlines: vd.auto_centermarks,
        auto_centerlines: Some(vd.auto_centerlines),
        ..Default::default()
    };
    let projected = |id: u64, dir: [f64; 2], position: [f64; 2]| DrawingView {
        id: ViewId(id),
        kind: ViewKind::Projected { parent: ViewId(1), direction: Some(dir) },
        position,
        show_centerlines: vd.auto_centermarks,
        auto_centerlines: Some(vd.auto_centerlines),
        ..Default::default()
    };
    let sheet = &mut d.sheets[0];
    sheet.views.push(base(1, ViewOrientation::Front, [front_x, front_y]));
    let v = if third { 1.0 } else { -1.0 };
    sheet.views.push(projected(2, [0.0, v], [front_x, other_y]));
    sheet.views.push(projected(3, [v, 0.0], [other_x, front_y]));
    sheet.views.push(base(4, ViewOrientation::Isometric, [other_x, other_y]));
    d
}

/// Suggested views of a flat pattern (live dx 10-27: the flat view, and the
/// part's isometric view beside it; live adds two edge views of the flat,
/// which projected views of a flat view can't show yet). `e`: the flat's
/// extent (mm, x by y).
fn flat_drawing(mut d: Drawing, src: &DrawingSource, e: DVec3, w: f64, h: f64) -> Drawing {
    let iso = (e.x + e.y) * 0.6;
    let (need_w, need_h) = (e.x + iso, e.y.max(iso * 0.6));
    let s = SCALES.iter().copied().find(|s| need_w * s <= w * 0.6 && need_h * s <= h * 0.6).unwrap_or(*SCALES.last().expect("scales"));
    d.sheets[0].scale = s;
    let gap = 25.0;
    let x0 = (w - (e.x + iso) * s - gap) * 0.5;
    let y = 15.0 + (h - 15.0) * 0.5;
    let vd = d.props.view_defaults.clone();
    let view = |id: u64, orientation: ViewOrientation, position: [f64; 2], flat: bool| DrawingView {
        id: ViewId(id),
        kind: ViewKind::Base { source: src.tab, config: src.config.clone(), orientation, part: src.part.clone() },
        position,
        style: RenderStyle::HiddenLinesRemoved,
        show_centerlines: vd.auto_centermarks,
        auto_centerlines: Some(vd.auto_centerlines),
        flat,
        ..Default::default()
    };
    d.sheets[0].views.push(view(1, ViewOrientation::Top, [x0 + e.x * s * 0.5, y], true));
    d.sheets[0].views.push(view(2, ViewOrientation::Isometric, [x0 + e.x * s + gap + iso * s * 0.5, y], false));
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::TabContent;
    use crate::drawing::templates::template_by_name;

    #[test]
    fn suggested_views_fit_the_sheet() {
        let mut doc = Document::new();
        if let TabContent::PartStudio(ps) = &mut doc.tabs[0].content {
            *ps = crate::demo::plate_studio();
        }
        let src = DrawingSource { tab: doc.tabs[0].id, part: None, config: ConfigValues::default(), sketch: None, flat: false };
        let bounds = source_bounds(&doc, &src).expect("bounds");
        // The plate: 100 x 60 x 12 mm.
        assert!(((bounds.1 - bounds.0) * 1000.0 - DVec3::new(100.0, 60.0, 12.0)).length() < 1e-3, "{bounds:?}");
        for name in ["ISO A3", "ANSI B", "ISO A4"] {
            let t = template_by_name(name).unwrap();
            let d = create_drawing(&t, Some(&src), Some(bounds), true, None);
            let sheet = &d.sheets[0];
            assert_eq!(sheet.views.len(), 4);
            assert_eq!(d.projection, t.projection);
            let (w, h) = sheet.size.size_mm();
            for v in &sheet.views {
                assert!(v.position[0] > 0.0 && v.position[0] < w && v.position[1] > 0.0 && v.position[1] < h, "{name}: {:?}", v.position);
                assert!(v.label.is_none());
                // Automatic centermarks (Drawing properties › Views default).
                assert!(v.show_centerlines, "{name}: {:?}", v.id);
            }
            // A3 fits the plate at 1:1, A4 needs 1:2... at most 1:1 for this small part.
            assert!(sheet.scale <= 1.0 && sheet.scale >= 0.5, "{name}: {}", sheet.scale);
            // The top view is above the front view in third angle, below in first angle.
            let (front, top) = (sheet.views[0].position, sheet.views[1].position);
            assert_eq!(top[1] > front[1], d.projection == Projection::ThirdAngle, "{name}");
        }
        let empty = create_drawing(&template_by_name("ISO A3").unwrap(), Some(&src), Some(bounds), false, None);
        assert!(empty.sheets[0].views.is_empty());
    }
}
