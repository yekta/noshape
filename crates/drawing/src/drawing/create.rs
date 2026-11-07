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
pub(crate) const SCALES: [f64; 30] = [
    100.0, 50.0, 32.0, 20.0, 16.0, 12.0, 10.0, 8.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0,
    1.0 / 2.0, 1.0 / 3.0, 1.0 / 4.0, 1.0 / 5.0, 1.0 / 6.0, 1.0 / 8.0, 1.0 / 10.0, 1.0 / 12.0, 1.0 / 16.0, 1.0 / 20.0, 1.0 / 25.0, 1.0 / 32.0, 1.0 / 40.0, 1.0 / 50.0, 1.0 / 80.0, 1.0 / 100.0,
];

/// The scale of the first view inserted into an empty sheet (live request
/// 1812): the largest list scale at which the source's Front view
/// (`extent`: width and height, mm at 1:1) fits 30 % of the border frame's
/// width and height (ANSI A: 78 × 59 mm; a 70 mm cube 1:2, a 10 × 5 mm
/// face 6:1).
pub fn first_view_scale(size: super::SheetSize, extent: [f64; 2]) -> f64 {
    let f = size.frame_rect();
    let (w, h) = (0.3 * (f[2] - f[0]), 0.3 * (f[3] - f[1]));
    SCALES.iter().copied().find(|s| extent[0] * s <= w + 1e-9 && extent[1] * s <= h + 1e-9).unwrap_or(*SCALES.last().expect("scales"))
}

/// The scale of the first view of one part inserted into an empty sheet (live 2133, Part 1 picked): the largest list
/// scale at which the Front view's width (mm at 1:1) is under 80 mm, 80 itself only at 1:1 and 2:1 (160 wide 1:3,
/// 20 wide 3:1, 40 wide 2:1); its height within 30 % of the frame. On other sheets than ANSI A the 80 mm in
/// proportion to the frame's width.
pub fn first_part_view_scale(size: super::SheetSize, extent: [f64; 2]) -> f64 {
    let f = size.frame_rect();
    let ansi = super::SheetSize::AnsiA.frame_rect();
    let limit = 80.0 * (f[2] - f[0]) / (ansi[2] - ansi[0]);
    let h = 0.3 * (f[3] - f[1]);
    let fits = |s: f64| {
        let w = extent[0] * s;
        (w < limit - 1e-6 || (w <= limit + 1e-6 && (s == 1.0 || s == 2.0))) && extent[1] * s <= h + 1e-6
    };
    SCALES.iter().copied().find(|s| fits(*s)).unwrap_or(*SCALES.last().expect("scales"))
}

/// A sheet's first inserted view gives the sheet its scale (live 1812: the
/// title block shows it, later views default to it); others keep theirs.
pub fn push_inserted_view(sheet: &mut super::Sheet, mut v: DrawingView) {
    if sheet.views.is_empty() && !sheet.scale_shown
        && let Some(s) = v.scale.take()
    {
        sheet.scale = s;
    }
    sheet.views.push(v);
}

/// A base view of an assembly.
fn shows_assembly(doc: &Document, v: &DrawingView) -> bool {
    matches!(&v.kind, ViewKind::Base { source, .. } if matches!(doc.tab(*source).map(|t| &t.content), Some(crate::document::TabContent::Assembly(_))))
}

/// Drawing properties › Views › Auto centermarks / centerlines are the parts'
/// defaults (live: "Auto centermarks (Parts)"): a view of an assembly gets
/// neither until shown from its menu.
pub fn assembly_view_defaults(doc: &Document, v: &mut DrawingView) {
    if shows_assembly(doc, v) {
        v.show_centerlines = false;
        v.auto_centerlines = Some(false);
    }
}

/// [`assembly_view_defaults`] on a new drawing's views: an assembly's base
/// views and the views projected from them.
pub fn assembly_drawing_defaults(doc: &Document, d: &mut Drawing) {
    for sheet in &mut d.sheets {
        let roots: Vec<ViewId> = sheet.views.iter().filter(|v| shows_assembly(doc, v)).map(|v| v.id).collect();
        let of: Vec<Option<ViewId>> = sheet.views.iter().map(|v| super::views::root_view(sheet, v.id)).collect();
        for (v, root) in sheet.views.iter_mut().zip(of) {
            if root.is_some_and(|r| roots.contains(&r)) {
                v.show_centerlines = false;
                v.auto_centerlines = Some(false);
            }
        }
    }
}

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
    // The exact boxes: the fast one is enlarged by the curved faces' poles
    // (pipeflange-steven 66 mm for 50, its views 1:2 where live's are 1:1).
    use rayon::prelude::*;
    let bodies: Vec<_> = model.context.bodies.iter().filter(|b| src.part.as_ref().is_none_or(|p| &b.id == p)).collect();
    bodies.par_iter().filter_map(|b| b.shape.bounding_box(true).ok()).map(|bb| (bb.min, bb.max)).reduce_with(|(lo, hi), (a, b)| (lo.min(a), hi.max(b)))
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
    // The Front view (X by Z) fits 30 % of the frame, as a first inserted
    // view does (live 1898: 3:1, 1:2, 1:3 on ANSI A; 1:1 on A3).
    d.sheets[0].scale = first_view_scale(d.sheets[0].size, [e.x, e.z]);
    // Live 1898 (blocks of 20 to 120 mm on ANSI A, and on ISO A3): the four
    // views' centres don't follow the part: two columns 0.42 of the sheet's
    // width apart about the frame's centre, two rows 0.3696 of its height
    // apart about 0.56 of it.
    let [fx0, _, fx1, _] = d.sheets[0].size.frame_rect();
    let (xc, yc, dx, dy) = ((fx0 + fx1) * 0.5, 0.56 * h, 0.21 * w, 0.1848 * h);
    let third = d.projection == Projection::ThirdAngle;
    // Third angle: front bottom-left, top above, right to the right.
    // First angle: front top-left, top below, the side view to its right.
    let (front_x, other_x) = (xc - dx, xc + dx);
    let (front_y, other_y) = if third { (yc - dy, yc + dy) } else { (yc + dy, yc - dy) };
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
    // In live's order (its Sheets panel: Front, Right, Top, Isometric); the ids stay Top 2, Right 3.
    sheet.views.push(projected(3, [v, 0.0], [other_x, front_y]));
    sheet.views.push(projected(2, [0.0, v], [front_x, other_y]));
    // Third angle: the isometric is projected from Front, up and right (live 2063: its parent is Front; the fold gives
    // the standard isometric frame). First angle keeps a base isometric (its diagonal fold is another view).
    if third {
        sheet.views.push(projected(4, [std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2], [other_x, other_y]));
    } else {
        sheet.views.push(base(4, ViewOrientation::Isometric, [other_x, other_y]));
    }
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

    /// Live 2133 (and 2067 / 2130): a picked part's first view, by its Front view's width alone (block L x W x H: the
    /// view is L x H).
    #[test]
    fn first_part_view_scale_keeps_the_width_under_80_mm() {
        use crate::drawing::SheetSize::AnsiA;
        for (l, h, want) in [(160.0, 10.0, 1.0 / 3.0), (240.0, 10.0, 0.25), (320.0, 10.0, 0.2), (20.0, 10.0, 3.0), (16.0, 5.0, 4.0), (10.0, 5.0, 6.0), (20.0, 5.0, 3.0), (40.0, 10.0, 2.0), (40.0, 20.0, 2.0), (80.0, 10.0, 1.0), (60.0, 10.0, 1.0), (100.0, 10.0, 0.5), (400.0, 10.0, 1.0 / 6.0), (90.0, 10.0, 0.5), (45.0, 10.0, 1.0)] {
            let got = first_part_view_scale(AnsiA, [l, h]);
            assert!((got - want).abs() < 1e-12, "{l} x {h}: {got} (live {want})");
        }
    }

    /// Live 1812: the first inserted view's scale per Front view extent
    /// (block L × W × H: the view is L × H) and template.
    #[test]
    fn first_view_scale_fits_30_percent_of_the_frame() {
        use crate::drawing::SheetSize::{A3, AnsiA};
        for (size, l, h, want) in [(AnsiA, 100.0, 20.0, 0.5), (AnsiA, 400.0, 10.0, 1.0 / 6.0), (AnsiA, 10.0, 5.0, 6.0), (A3, 100.0, 20.0, 1.0), (AnsiA, 75.0, 10.0, 1.0), (AnsiA, 70.0, 10.0, 1.0), (AnsiA, 10.0, 10.0, 5.0), (AnsiA, 70.0, 70.0, 0.5), (A3, 118.0, 5.0, 1.0), (AnsiA, 476.0, 9.75, 1.0 / 8.0)] {
            let got = first_view_scale(size, [l, h]);
            assert!((got - want).abs() < 1e-12, "{size:?} {l} x {h}: {got} (live {want})");
        }
    }

    /// Live 1898: suggested views of blocks (mm) sit at the same spots whatever
    /// the part, at the scale a first view would take: ANSI A (centres in
    /// inches) and ISO A3 (mm, first angle: front top-left, top below it).
    #[test]
    fn suggested_views_where_lives_are() {
        let doc = Document::new();
        let src = DrawingSource { tab: doc.tabs[0].id, part: None, config: ConfigValues::default(), sketch: None, flat: false };
        let at = |name: &str, e: [f64; 3]| {
            let d = create_drawing(&template_by_name(name).unwrap(), Some(&src), Some((DVec3::ZERO, DVec3::from(e) / 1000.0)), true, None);
            (d.sheets[0].scale, d.sheets[0].views.iter().map(|v| v.position).collect::<Vec<_>>())
        };
        let near = |a: [f64; 2], b: [f64; 2], k: f64| (a[0] - b[0] * k).abs() < 1e-3 && (a[1] - b[1] * k).abs() < 1e-3;
        // (front, right, top, isometric: live's order)
        let ansi = [[3.209, 3.1892], [7.829, 3.1892], [3.209, 6.3308], [7.829, 6.3308]];
        for (e, scale) in [([20.0, 10.0, 5.0], 3.0), ([100.0, 50.0, 20.0], 0.5), ([60.0, 60.0, 120.0], 1.0 / 3.0)] {
            let (s, p) = at("ANSI A", e);
            assert!((s - scale).abs() < 1e-12, "{e:?}: {s}");
            assert!(p.iter().zip(ansi).all(|(a, b)| near(*a, b, 25.4)), "{e:?}: {p:?}");
        }
        let (s, p) = at("ISO A3", [100.0, 50.0, 20.0]);
        assert_eq!(s, 1.0);
        let iso = [[122.3, 221.2056], [298.7, 221.2056], [122.3, 111.4344], [298.7, 111.4344]];
        assert!(p.iter().zip(iso).all(|(a, b)| near(*a, b, 1.0)), "{p:?}");
    }

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
            let at = |id: u64| sheet.views.iter().find(|v| v.id.0 == id).map(|v| v.position).expect("view");
            let (front, top) = (at(1), at(2));
            assert_eq!(top[1] > front[1], d.projection == Projection::ThirdAngle, "{name}");
        }
        let empty = create_drawing(&template_by_name("ISO A3").unwrap(), Some(&src), Some(bounds), false, None);
        assert!(empty.sheets[0].views.is_empty());
    }
}
