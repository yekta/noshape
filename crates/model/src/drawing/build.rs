//! Assemble a whole sheet: views scaled and placed, section hatching, cutting
//! lines, detail circles, view labels, associative annotations, border and
//! title block, into one [`SheetGeometry`] plus a pick table for the editor.

use super::font;
use super::sheet::{Anchor, LineStyle, Polyline2, SheetGeometry, Text2, format_scale};
use super::views::{self, Sources, ViewCache, ViewGeometry, dist_to_segment, resolve_point};
use super::{AnnotationKind, Drawing, RenderStyle, Sheet, ViewId, ViewKind};
use noshape_kernel::EdgeClass;
use std::collections::HashMap;
use std::sync::Arc;

/// What a pick id (the `pick` of lines and texts) refers to.
#[derive(Clone, Debug, PartialEq)]
pub enum PickTarget {
    None,
    /// View label, centermarks and other view decorations.
    View(ViewId),
    /// A projected edge of a view (index into `ViewGeometry::edges`).
    Edge { view: ViewId, edge: u32 },
    Annotation(u64),
    /// Cutting line drawn in the parent of section view `0`.
    SectionLine(ViewId),
    /// Detail circle drawn in the parent of detail view `0`.
    DetailCircle(ViewId),
    TitleBlock,
}

impl PickTarget {
    /// The view this target belongs to (for selecting/dragging views).
    pub fn view(&self) -> Option<ViewId> {
        match self {
            PickTarget::View(v) | PickTarget::Edge { view: v, .. } => Some(*v),
            _ => None,
        }
    }
}

/// Mapping between a view's coordinates (meters) and the sheet (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPlacement {
    pub id: ViewId,
    pub origin: [f64; 2],
    pub position: [f64; 2],
    /// Paper length per model length.
    pub scale: f64,
    /// Sheet-space bounds of the view's geometry.
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl ViewPlacement {
    pub fn to_sheet(&self, p: [f64; 2]) -> [f64; 2] {
        let k = self.scale * 1000.0;
        [self.position[0] + (p[0] - self.origin[0]) * k, self.position[1] + (p[1] - self.origin[1]) * k]
    }
    pub fn to_view(&self, s: [f64; 2]) -> [f64; 2] {
        let k = self.scale * 1000.0;
        [self.origin[0] + (s[0] - self.position[0]) / k, self.origin[1] + (s[1] - self.position[1]) / k]
    }
    /// Model length -> sheet length.
    pub fn len_to_sheet(&self, l: f64) -> f64 {
        l * self.scale * 1000.0
    }
    pub fn contains(&self, s: [f64; 2], margin: f64) -> bool {
        s[0] >= self.min[0] - margin && s[0] <= self.max[0] + margin && s[1] >= self.min[1] - margin && s[1] <= self.max[1] + margin
    }
}

pub struct BuiltSheet {
    pub geometry: SheetGeometry,
    /// Indexed by pick id (0 = nothing).
    pub picks: Vec<PickTarget>,
    pub placements: Vec<ViewPlacement>,
    /// The sheet with associative annotation inputs resolved to current
    /// model geometry.
    pub resolved: Sheet,
    /// Annotations whose references could not be resolved.
    pub dangling: Vec<u64>,
}

impl BuiltSheet {
    pub fn placement(&self, id: ViewId) -> Option<&ViewPlacement> {
        self.placements.iter().find(|p| p.id == id)
    }

    /// Closest pick target to a sheet point within `tol` mm. Annotations win
    /// ties against view geometry.
    pub fn hit(&self, p: [f64; 2], tol: f64) -> Option<u32> {
        let mut best: Option<(u32, f64)> = None;
        let mut consider = |pick: u32, d: f64| {
            if pick == 0 || d > tol {
                return;
            }
            let bias = match self.picks.get(pick as usize) {
                Some(PickTarget::Annotation(_)) => -tol * 0.5,
                Some(PickTarget::TitleBlock) => tol * 0.25,
                _ => 0.0,
            };
            let d = d + bias;
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((pick, d));
            }
        };
        for l in &self.geometry.lines {
            if l.pick == 0 {
                continue;
            }
            let n = l.points.len();
            let segs = if l.closed { n } else { n.saturating_sub(1) };
            let mut dmin = f64::INFINITY;
            for i in 0..segs {
                dmin = dmin.min(dist_to_segment(p, l.points[i], l.points[(i + 1) % n]));
            }
            consider(l.pick, dmin);
        }
        for t in &self.geometry.texts {
            if t.pick != 0 && font::text_hit(t, p) {
                consider(t.pick, tol * 0.3);
            }
        }
        best.map(|(k, _)| k)
    }
}

/// Format a model length (meters) in drawing units.
pub fn format_length(drawing: &Drawing, meters: f64) -> String {
    let (v, digits) = if drawing.units_mm { (meters * 1000.0, 2) } else { (meters / 0.0254, 3) };
    let s = format!("{v:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Compute all views of a sheet and assemble it (headless convenience).
pub fn render_sheet(drawing: &Drawing, sheet_index: usize, sources: &Sources, cache: &ViewCache, exact: bool) -> Option<BuiltSheet> {
    let sheet = drawing.sheets.get(sheet_index)?;
    let views = views::compute_sheet_views(drawing, sheet, sources, cache, exact);
    Some(assemble_sheet(drawing, sheet, &views, sources))
}

struct Picks {
    table: Vec<PickTarget>,
    index: HashMap<String, u32>,
}

impl Picks {
    fn id(&mut self, t: PickTarget) -> u32 {
        let key = format!("{t:?}");
        if let Some(i) = self.index.get(&key) {
            return *i;
        }
        self.table.push(t);
        let i = (self.table.len() - 1) as u32;
        self.index.insert(key, i);
        i
    }
}

/// Placement of a view on its sheet.
pub fn placement_of(sheet: &Sheet, g: &ViewGeometry) -> Option<ViewPlacement> {
    let v = sheet.view(g.id)?;
    let scale = views::view_scale(sheet, g.id);
    let mut p = ViewPlacement { id: g.id, origin: g.origin, position: v.position, scale, min: [0.0; 2], max: [0.0; 2] };
    let a = p.to_sheet(g.min);
    let b = p.to_sheet(g.max);
    p.min = [a[0].min(b[0]), a[1].min(b[1])];
    p.max = [a[0].max(b[0]), a[1].max(b[1])];
    // Empty or failed views still get a grab area.
    if p.max[0] - p.min[0] < 10.0 {
        p.min[0] = p.position[0].min(p.min[0]) - 5.0;
        p.max[0] = p.position[0].max(p.max[0]) + 5.0;
    }
    if p.max[1] - p.min[1] < 10.0 {
        p.min[1] = p.position[1].min(p.min[1]) - 5.0;
        p.max[1] = p.position[1].max(p.max[1]) + 5.0;
    }
    Some(p)
}

/// Resolve associative annotation inputs against current model geometry.
pub fn resolve_annotations(drawing: &Drawing, sheet: &Sheet, views: &HashMap<ViewId, Arc<ViewGeometry>>, sources: &Sources) -> (Sheet, Vec<u64>) {
    let _ = drawing;
    let mut out = sheet.clone();
    let mut dangling = Vec::new();
    for a in &mut out.annotations {
        if a.refs.is_empty() {
            continue;
        }
        let Some(view) = a.kind.view() else { continue };
        let frame = views.get(&view).and_then(|g| g.frame);
        let ctx = views::view_source(sheet, view).and_then(|(tab, _)| sources.get(&tab)).map(|s| s.context.clone());
        let (Some(frame), Some(ctx)) = (frame, ctx) else {
            dangling.push(a.id);
            continue;
        };
        let mut ok = true;
        for r in &a.refs {
            let Some((p, radius)) = resolve_point(&ctx, &frame, &r.point) else {
                ok = false;
                continue;
            };
            match (&mut a.kind, r.slot) {
                (AnnotationKind::LinearDimension { a: pa, .. }, 0) => *pa = p,
                (AnnotationKind::LinearDimension { b: pb, .. }, 1) => *pb = p,
                (AnnotationKind::RadialDimension { center, radius: rr, .. }, 0) => {
                    *center = p;
                    if let Some(r) = radius {
                        *rr = r;
                    }
                }
                (AnnotationKind::AngularDimension { vertex, .. }, 0) => *vertex = p,
                (AnnotationKind::AngularDimension { a: pa, .. }, 1) => *pa = p,
                (AnnotationKind::AngularDimension { b: pb, .. }, 2) => *pb = p,
                (AnnotationKind::Centermark { center, size, .. }, 0) => {
                    *center = p;
                    if let Some(r) = radius {
                        *size = r * 1.25;
                    }
                }
                (AnnotationKind::Centerline { a: pa, .. }, 0) => *pa = p,
                (AnnotationKind::Centerline { b: pb, .. }, 1) => *pb = p,
                (AnnotationKind::Balloon { anchor, .. }, 0) => *anchor = p,
                _ => {}
            }
        }
        if !ok {
            dangling.push(a.id);
        }
    }
    (out, dangling)
}

/// 45-degree hatch lines (sheet coordinates) inside the region bounded by
/// `segments` (even-odd rule), `spacing` mm apart.
pub fn hatch(segments: &[[[f64; 2]; 2]], spacing: f64) -> Vec<[[f64; 2]; 2]> {
    if segments.is_empty() || spacing <= 0.0 {
        return vec![];
    }
    let s2 = std::f64::consts::FRAC_1_SQRT_2;
    // w: coordinate across the hatch lines; t: along them.
    let w = |p: [f64; 2]| (-p[0] + p[1]) * s2;
    let t = |p: [f64; 2]| (p[0] + p[1]) * s2;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in segments {
        for p in s {
            lo = lo.min(w(*p));
            hi = hi.max(w(*p));
        }
    }
    let mut out = Vec::new();
    let k0 = (lo / spacing).ceil() as i64;
    let k1 = (hi / spacing).floor() as i64;
    if k1 - k0 > 20000 {
        return out;
    }
    let mut ts: Vec<f64> = Vec::new();
    for k in k0..=k1 {
        let wl = k as f64 * spacing + spacing * 0.5;
        ts.clear();
        for s in segments {
            let (wa, wb) = (w(s[0]), w(s[1]));
            if (wa <= wl) != (wb <= wl) {
                let f = (wl - wa) / (wb - wa);
                ts.push(t(s[0]) + (t(s[1]) - t(s[0])) * f);
            }
        }
        ts.sort_by(f64::total_cmp);
        for pair in ts.chunks_exact(2) {
            let to_xy = |tt: f64| [(tt - wl) * s2, (tt + wl) * s2];
            out.push([to_xy(pair[0]), to_xy(pair[1])]);
        }
    }
    out
}

fn circle_points(c: [f64; 2], r: f64, n: usize) -> Vec<[f64; 2]> {
    (0..n).map(|i| {
        let t = i as f64 / n as f64 * std::f64::consts::TAU;
        [c[0] + r * t.cos(), c[1] + r * t.sin()]
    }).collect()
}

/// Assemble a sheet from computed view geometry.
pub fn assemble_sheet(drawing: &Drawing, sheet: &Sheet, views: &HashMap<ViewId, Arc<ViewGeometry>>, sources: &Sources) -> BuiltSheet {
    let (w, h) = sheet.size.size_mm();
    let mut g = SheetGeometry::new(w, h);
    let mut picks = Picks { table: vec![PickTarget::None], index: HashMap::new() };

    // Border and title block; everything inside the title block is pickable
    // as the title block.
    let mark_l = g.lines.len();
    let mark_t = g.texts.len();
    g.add_border_and_title(sheet);
    let rows = sheet.title_block.fields.len().div_ceil(2) as f64;
    let (tx0, ty1) = (w - 10.0 - 180.0, 10.0 + 8.0 * rows);
    let in_tb = |p: &[f64; 2]| p[0] >= tx0 - 0.01 && p[1] <= ty1 + 0.01 && p[0] <= w - 9.99 && p[1] >= 9.99;
    let tb = picks.id(PickTarget::TitleBlock);
    for l in &mut g.lines[mark_l..] {
        if l.points.iter().all(in_tb) && l.style != LineStyle::Border {
            l.pick = tb;
        }
    }
    for t in &mut g.texts[mark_t..] {
        if in_tb(&t.pos) {
            t.pick = tb;
        }
    }

    let mut placements: Vec<ViewPlacement> = Vec::new();
    for v in &sheet.views {
        let Some(geo) = views.get(&v.id) else { continue };
        let Some(pl) = placement_of(sheet, geo) else { continue };
        placements.push(pl);
    }
    let place = |id: ViewId| placements.iter().find(|p| p.id == id).copied();

    for v in &sheet.views {
        let (Some(geo), Some(pl)) = (views.get(&v.id), place(v.id)) else { continue };
        let view_pick = picks.id(PickTarget::View(v.id));
        // Projected edges.
        for (i, e) in geo.edges.iter().enumerate() {
            let style = match (e.visible, e.class) {
                (_, EdgeClass::Seam) => continue,
                (true, EdgeClass::Smooth) if v.show_tangent_edges => LineStyle::Tangent,
                (true, EdgeClass::Smooth) => continue,
                (false, EdgeClass::Smooth) => continue,
                (true, _) => LineStyle::Visible,
                (false, _) if v.style == RenderStyle::HiddenLinesVisible => LineStyle::Hidden,
                (false, _) => continue,
            };
            let pick = picks.id(PickTarget::Edge { view: v.id, edge: i as u32 });
            g.lines.push(Polyline2 { points: e.points.iter().map(|p| pl.to_sheet(*p)).collect(), style, closed: false, pick });
        }
        // Section hatching.
        if !geo.section_segments.is_empty() {
            let segs: Vec<[[f64; 2]; 2]> = geo.section_segments.iter().map(|s| [pl.to_sheet(s[0]), pl.to_sheet(s[1])]).collect();
            let mut lines = hatch(&segs, 2.5);
            if let Some((c, r)) = geo.clip {
                let (c, r) = (pl.to_sheet(c), pl.len_to_sheet(r));
                lines = lines.into_iter().flat_map(|l| views::clip_polyline_circle(&l, c, r)).filter(|p| p.len() == 2).map(|p| [p[0], p[1]]).collect();
            }
            for l in lines {
                g.lines.push(Polyline2 { points: l.to_vec(), style: LineStyle::Thin, closed: false, pick: view_pick });
            }
        }
        // Detail boundary.
        if let Some((c, r)) = geo.clip {
            g.lines.push(Polyline2 { points: circle_points(pl.to_sheet(c), pl.len_to_sheet(r), 96), style: LineStyle::Thin, closed: true, pick: view_pick });
        }
        // Automatic centermarks on face-on circles.
        if v.show_centerlines {
            for (c, r, _) in geo.circles() {
                let (c, r) = (pl.to_sheet(c), pl.len_to_sheet(r) * 1.15 + 1.0);
                g.lines.push(Polyline2 { points: vec![[c[0] - r, c[1]], [c[0] + r, c[1]]], style: LineStyle::Centerline, closed: false, pick: view_pick });
                g.lines.push(Polyline2 { points: vec![[c[0], c[1] - r], [c[0], c[1] + r]], style: LineStyle::Centerline, closed: false, pick: view_pick });
            }
        }
        if let Some(err) = &geo.error
            && geo.edges.is_empty()
        {
            g.texts.push(Text2 { pos: pl.position, height: 3.0, text: err.clone(), angle: 0.0, anchor: Anchor::Center, pick: view_pick });
            g.rect(pl.min[0], pl.min[1], pl.max[0], pl.max[1], LineStyle::Thin);
            if let Some(l) = g.lines.last_mut() {
                l.pick = view_pick;
            }
        }
        // Labels below the view.
        let mut label_lines: Vec<(String, f64)> = Vec::new();
        match &v.kind {
            ViewKind::Section { label, .. } => label_lines.push((format!("SECTION {label}-{label}"), 5.0)),
            ViewKind::Detail { label, .. } => {
                label_lines.push((format!("DETAIL {label}"), 5.0));
                label_lines.push((format!("SCALE {}", format_scale(pl.scale)), 3.5));
            }
            _ => {}
        }
        if let Some(l) = v.label.as_ref().filter(|l| !l.is_empty())
            && !matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. })
        {
            label_lines.push((l.clone(), 5.0));
            if v.scale.is_some() {
                label_lines.push((format!("SCALE {}", format_scale(pl.scale)), 3.5));
            }
        }
        let mut y = pl.min[1] - 6.0;
        for (text, height) in label_lines {
            y -= height;
            g.texts.push(Text2 { pos: [(pl.min[0] + pl.max[0]) / 2.0, y], height, text, angle: 0.0, anchor: Anchor::BottomCenter, pick: view_pick });
            y -= 2.0;
        }
        // Cutting line / detail circle in the parent.
        match &v.kind {
            ViewKind::Section { parent, label, .. } => {
                if let (Some(ppl), Some(cut)) = (place(*parent), views::section_cut(sheet, v.id)) {
                    let pick = picks.id(PickTarget::SectionLine(v.id));
                    let a = ppl.to_sheet(cut.p0);
                    let b = ppl.to_sheet(cut.p1);
                    g.lines.push(Polyline2 { points: vec![a, b], style: LineStyle::Section, closed: false, pick });
                    let d = [b[0] - a[0], b[1] - a[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                    let u = [d[0] / l, d[1] / l];
                    let sight = [-cut.toward_view[0], -cut.toward_view[1]];
                    for (end, sgn) in [(a, -1.0), (b, 1.0)] {
                        // Short thick end stroke, then the arrow showing the line of sight.
                        let e2 = [end[0] - u[0] * sgn * 6.0, end[1] - u[1] * sgn * 6.0];
                        g.lines.push(Polyline2 { points: vec![e2, end], style: LineStyle::Border, closed: false, pick });
                        let base = [end[0] - u[0] * sgn * 1.5, end[1] - u[1] * sgn * 1.5];
                        let tail = [base[0] - sight[0] * 8.0, base[1] - sight[1] * 8.0];
                        g.lines.push(Polyline2 { points: vec![tail, base], style: LineStyle::Thin, closed: false, pick });
                        g.arrow(base, sight, 3.5);
                        let tp = [tail[0] + u[0] * sgn * 3.0 - sight[0] * 1.0, tail[1] + u[1] * sgn * 3.0 - sight[1] * 1.0];
                        g.texts.push(Text2 { pos: tp, height: 5.0, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    }
                }
            }
            ViewKind::Detail { parent, center, radius, label } => {
                if let Some(ppl) = place(*parent) {
                    let pick = picks.id(PickTarget::DetailCircle(v.id));
                    let c = ppl.to_sheet(*center);
                    let r = ppl.len_to_sheet(*radius);
                    g.lines.push(Polyline2 { points: circle_points(c, r, 96), style: LineStyle::Thin, closed: true, pick });
                    let a = std::f64::consts::FRAC_PI_4;
                    g.texts.push(Text2 { pos: [c[0] + (r + 4.0) * a.cos(), c[1] + (r + 4.0) * a.sin()], height: 5.0, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                }
            }
            _ => {}
        }
    }

    // Annotations with resolved references.
    let (resolved, dangling) = resolve_annotations(drawing, sheet, views, sources);
    let (ml, mt) = (g.lines.len(), g.texts.len());
    let to_sheet = |id: ViewId, p: [f64; 2]| place(id).map(|pl| pl.to_sheet(p)).unwrap_or(p);
    g.add_annotations(&resolved, &to_sheet, &|m| format_length(drawing, m));
    let mut remap: HashMap<u32, u32> = HashMap::new();
    for a in &resolved.annotations {
        remap.insert((a.id & 0xFFFF_FFFF) as u32, picks.id(PickTarget::Annotation(a.id)));
    }
    for l in &mut g.lines[ml..] {
        l.pick = remap.get(&l.pick).copied().unwrap_or(0);
    }
    for t in &mut g.texts[mt..] {
        t.pick = remap.get(&t.pick).copied().unwrap_or(0);
    }

    BuiltSheet { geometry: g, picks: picks.table, placements, resolved, dangling }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatch_square() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let segs: Vec<[[f64; 2]; 2]> = (0..4).map(|i| [sq[i], sq[(i + 1) % 4]]).collect();
        let lines = hatch(&segs, 2.5);
        assert!(lines.len() >= 5, "{}", lines.len());
        for l in &lines {
            for p in l {
                assert!(p[0] > -1e-9 && p[0] < 10.0 + 1e-9 && p[1] > -1e-9 && p[1] < 10.0 + 1e-9);
            }
            // 45 degrees.
            assert!(((l[1][1] - l[0][1]) - (l[1][0] - l[0][0])).abs() < 1e-9);
        }
    }

    #[test]
    fn length_format() {
        let d = Drawing::default();
        assert_eq!(format_length(&d, 0.1), "100");
        assert_eq!(format_length(&d, 0.0125), "12.5");
        assert_eq!(format_length(&d, 0.0024), "2.4");
    }
}
