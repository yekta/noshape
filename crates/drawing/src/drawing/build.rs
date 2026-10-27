//! Assemble a whole sheet: views scaled, rotated, broken and placed,
//! section hatching, cutting lines, detail circles, break lines, view
//! labels, associative annotations, tables, border and title block, into
//! one [`SheetGeometry`] plus a pick table for the editor.

use super::CadTables;
use super::cad_layers::{CadLayer, CadTextStyle};
use super::font;
use super::format::{self, DimText};
use super::sheet::{Anchor, AnnotationEnv, Fill2, LineStyle, Polyline2, SheetGeometry, Text2, format_scale};
use super::tables::{self, TableLayout};
use super::views::{self, Sources, ViewCache, ViewGeometry, dist_to_segment, resolve_in_view, rotate2};
use super::sheet::LayerSpan;
use super::{Annotation, AnnotationKind, BreakStyle, Drawing, RenderStyle, Sheet, SheetLayer, TableKind, ViewId, ViewKind};
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
    /// The inspection item bubble of an annotation.
    Inspection(u64),
    /// Cutting line drawn in the parent of section view `0`.
    SectionLine(ViewId),
    /// Detail circle drawn in the parent of detail view `0`.
    DetailCircle(ViewId),
    /// The profile of broken-out section `index` of a view (live: its menu
    /// has Edit section / Remove section).
    BrokenOut { view: ViewId, index: usize },
    TitleBlock,
    /// A title block cell's field (its key): a note, editable while the
    /// formats are locked (live qa4 round 5 s11 (4)).
    TitleField(String),
    Table(u64),
    /// Break lines of break `index` of a view.
    BreakLine { view: ViewId, index: usize },
    /// A drawing sketch entity (sheet sketch id).
    SketchEntity(u32),
    /// A hatch region.
    Hatch(u64),
    /// An inserted image.
    Image(u64),
    /// The reference indicator of auxiliary view `0` in its parent.
    AuxIndicator(ViewId),
    /// The label (and scale label) under view `0`.
    ViewLabel(ViewId),
    /// An automatic centermark of a view, keyed by its circle's center
    /// ([`centermark_key`]).
    AutoCentermark { view: ViewId, center: [i64; 2] },
}

/// Key of an automatic centermark: its center in view units (meters), to
/// the nanometer.
pub fn centermark_key(c: [f64; 2]) -> [i64; 2] {
    c.map(|x| (x * 1e9).round() as i64)
}

impl PickTarget {
    /// The view this target belongs to (for selecting/dragging views).
    pub fn view(&self) -> Option<ViewId> {
        match self {
            PickTarget::View(v) | PickTarget::Edge { view: v, .. } | PickTarget::ViewLabel(v) | PickTarget::BrokenOut { view: v, .. } => Some(*v),
            _ => None,
        }
    }
}

/// One break of a placed view, in view coordinates: the band `a..b` of
/// axis `axis` (0 = x) shrinks to `band` (view units).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BreakMap {
    pub axis: u8,
    pub a: f64,
    pub b: f64,
    pub band: f64,
    /// Index in the view's `breaks`.
    pub index: u8,
}

pub const MAX_BREAKS: usize = 4;

/// Mapping between a view's coordinates (meters) and the sheet (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPlacement {
    pub id: ViewId,
    pub origin: [f64; 2],
    pub position: [f64; 2],
    /// Paper length per model length.
    pub scale: f64,
    /// Rotation on the sheet (radians, counter-clockwise).
    pub rotation: f64,
    /// Sheet-space bounds of the view's geometry.
    pub min: [f64; 2],
    pub max: [f64; 2],
    pub breaks: [BreakMap; MAX_BREAKS],
    pub nbreaks: u8,
}

impl ViewPlacement {
    pub fn new(id: ViewId, origin: [f64; 2], position: [f64; 2], scale: f64) -> Self {
        Self { id, origin, position, scale, rotation: 0.0, min: position, max: position, breaks: [BreakMap::default(); MAX_BREAKS], nbreaks: 0 }
    }

    pub fn breaks(&self) -> &[BreakMap] {
        &self.breaks[..self.nbreaks as usize]
    }

    /// The same placement without breaks.
    pub fn unbroken(&self) -> ViewPlacement {
        ViewPlacement { nbreaks: 0, ..*self }
    }

    /// View coordinates with the break bands collapsed.
    pub fn collapse(&self, p: [f64; 2]) -> [f64; 2] {
        let mut q = p;
        for br in self.breaks() {
            let ax = br.axis as usize;
            let x = p[ax];
            if x >= br.b {
                q[ax] -= br.b - br.a - br.band;
            } else if x > br.a {
                q[ax] -= (x - br.a) * (1.0 - br.band / (br.b - br.a));
            }
        }
        q
    }

    /// Inverse of [`Self::collapse`].
    pub fn expand(&self, q: [f64; 2]) -> [f64; 2] {
        let mut p = q;
        for ax in 0..2 {
            let mut brs: Vec<&BreakMap> = self.breaks().iter().filter(|b| b.axis as usize == ax).collect();
            brs.sort_by(|a, b| a.a.total_cmp(&b.a));
            let xc = q[ax];
            let mut acc = 0.0;
            let mut out = None;
            for br in brs {
                let ac = br.a - acc;
                if xc <= ac {
                    out = Some(xc + acc);
                    break;
                }
                if xc <= ac + br.band {
                    out = Some(br.a + (xc - ac) / br.band.max(1e-300) * (br.b - br.a));
                    break;
                }
                acc += br.b - br.a - br.band;
            }
            p[ax] = out.unwrap_or(xc + acc);
        }
        p
    }

    pub fn to_sheet(&self, p: [f64; 2]) -> [f64; 2] {
        let k = self.scale * 1000.0;
        let (q, o) = (self.collapse(p), self.collapse(self.origin));
        let d = rotate2([(q[0] - o[0]) * k, (q[1] - o[1]) * k], self.rotation);
        [self.position[0] + d[0], self.position[1] + d[1]]
    }
    pub fn to_view(&self, s: [f64; 2]) -> [f64; 2] {
        let k = self.scale * 1000.0;
        let o = self.collapse(self.origin);
        let d = rotate2([s[0] - self.position[0], s[1] - self.position[1]], -self.rotation);
        self.expand([o[0] + d[0] / k, o[1] + d[1] / k])
    }
    /// Model length -> sheet length.
    pub fn len_to_sheet(&self, l: f64) -> f64 {
        l * self.scale * 1000.0
    }
    /// View-plane direction -> sheet direction.
    pub fn dir_to_sheet(&self, v: [f64; 2]) -> [f64; 2] {
        rotate2(v, self.rotation)
    }
    pub fn contains(&self, s: [f64; 2], margin: f64) -> bool {
        s[0] >= self.min[0] - margin && s[0] <= self.max[0] + margin && s[1] >= self.min[1] - margin && s[1] <= self.max[1] + margin
    }
    /// Distance from a sheet point to the view box (0 inside).
    pub fn dist_to(&self, s: [f64; 2]) -> f64 {
        let dx = (self.min[0] - s[0]).max(s[0] - self.max[0]).max(0.0);
        let dy = (self.min[1] - s[1]).max(s[1] - self.max[1]).max(0.0);
        (dx * dx + dy * dy).sqrt()
    }
}

/// Document-level data for property fields.
#[derive(Clone, Debug, Default)]
pub struct SheetInfo {
    pub doc_name: String,
    pub drawing_name: String,
    /// The user (title block DRAWN, `{user.name}`).
    pub user_name: String,
    /// The drawing's revisions, oldest first (revision tables; from
    /// [`tables::revision_rows`]).
    pub revisions: Vec<tables::RevisionRow>,
    /// Evaluated custom tables (from [`tables::evaluate_custom_tables`]).
    pub custom_tables: tables::CustomTables,
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
    /// Table grids (table id, layout).
    pub tables: Vec<(u64, TableLayout)>,
    /// Title block rectangle [x0, y0, x1, y1] (mm).
    pub title_rect: Option<[f64; 4]>,
    /// The title block's field cells (pick, rectangle).
    pub title_cells: Vec<(u32, [f64; 4])>,
}

impl BuiltSheet {
    pub fn placement(&self, id: ViewId) -> Option<&ViewPlacement> {
        self.placements.iter().find(|p| p.id == id)
    }

    pub fn table_layout(&self, id: u64) -> Option<&TableLayout> {
        self.tables.iter().find(|(t, _)| *t == id).map(|(_, l)| l)
    }

    /// The closest line pick target to a sheet point within `tol` mm among
    /// those `accept` takes (a tool that takes only view edges sees the
    /// edge under an annotation or a centerline; qa DWG-DIM-TWO-EDGES).
    pub fn hit_where(&self, p: [f64; 2], tol: f64, accept: &dyn Fn(&PickTarget) -> bool) -> Option<u32> {
        let mut best: Option<(u32, f64)> = None;
        for l in self.geometry.lines.iter().filter(|l| l.pick != 0 && self.picks.get(l.pick as usize).is_some_and(accept)) {
            let n = l.points.len();
            let segs = if l.closed { n } else { n.saturating_sub(1) };
            let d = (0..segs).map(|i| dist_to_segment(p, l.points[i], l.points[(i + 1) % n])).fold(f64::INFINITY, f64::min);
            if d <= tol && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((l.pick, d));
            }
        }
        best.map(|(k, _)| k)
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
                Some(PickTarget::Annotation(_) | PickTarget::Inspection(_)) => -tol * 0.5,
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
        // Inside a table counts as on it (tables sit on top of everything
        // but annotations).
        let on_annotation = best.is_some_and(|(k, _)| matches!(self.picks.get(k as usize), Some(PickTarget::Annotation(_))));
        if !on_annotation {
            for (id, l) in self.tables.iter().rev() {
                if l.contains(p, 0.0)
                    && let Some(i) = self.picks.iter().position(|t| *t == PickTarget::Table(*id))
                {
                    return Some(i as u32);
                }
            }
        }
        // A field cell's note over the block's lines; anywhere else inside
        // the title block selects it.
        let inside = |r: &[f64; 4]| p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3];
        if best.is_none_or(|(k, _)| matches!(self.picks.get(k as usize), Some(PickTarget::TitleBlock)))
            && let Some((pick, _)) = self.title_cells.iter().find(|(_, r)| inside(r))
        {
            return Some(*pick);
        }
        if best.is_none()
            && let Some(r) = self.title_rect
            && p[0] >= r[0]
            && p[0] <= r[2]
            && p[1] >= r[1]
            && p[1] <= r[3]
            && let Some(i) = self.picks.iter().position(|t| *t == PickTarget::TitleBlock)
        {
            return Some(i as u32);
        }
        best.map(|(k, _)| k)
    }
}

/// How far a section cutting line runs past its view (live: 0.25 in).
const CUT_OVERSHOOT: f64 = 6.35;

/// Format a model length (meters) in drawing units.
pub fn format_length(drawing: &Drawing, meters: f64) -> String {
    format::format_len(drawing, &Default::default(), meters)
}

/// Compute all views of a sheet and assemble it (headless convenience).
pub fn render_sheet(drawing: &Drawing, sheet_index: usize, sources: &Sources, cache: &ViewCache, exact: bool) -> Option<BuiltSheet> {
    render_sheet_with(drawing, sheet_index, sources, cache, exact, &SheetInfo::default())
}

pub fn render_sheet_with(drawing: &Drawing, sheet_index: usize, sources: &Sources, cache: &ViewCache, exact: bool, info: &SheetInfo) -> Option<BuiltSheet> {
    let sheet = drawing.sheets.get(sheet_index)?;
    let views = views::compute_sheet_views(drawing, sheet, sources, cache, exact);
    Some(assemble_sheet(drawing, sheet_index, &views, sources, info))
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

/// Placement of view `id` showing geometry `g` (geometry is shared through
/// the cache between views with identical projections, so `g.id` may differ).
/// An auxiliary view's reference indicator on the sheet (live behaviours
/// "Auxiliary view reference indicator", asb 10-20 u): a phantom viewing-plane
/// line parallel to the edge, on the view's side, centred on the edge and
/// 0.105 in longer at each end; at each end an arrow (shaft + head, 0.36 in)
/// pointing at the edge, and the letter beyond its tip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuxIndicator {
    /// The anchor on the edge (sheet).
    pub anchor: [f64; 2],
    /// Along the edge and toward the view (sheet, unit).
    pub es: [f64; 2],
    pub ns: [f64; 2],
    pub offset: f64,
    /// The line's half lengths either side of its middle.
    pub ext: [f64; 2],
    /// The line's ends (where the arrows start).
    pub elbows: [[f64; 2]; 2],
    pub tips: [[f64; 2]; 2],
    /// The middle snap point (on the line).
    pub mid: [f64; 2],
    /// The letters' places (each one's snap point).
    pub letters: [[f64; 2]; 2],
}

/// Arrow style: where the letter sits, beside the arrow (live u-933: 0.13 in
/// from the tail along it, 0.17 in to its upper / left side), plus its
/// own move (`letter_at[0]`).
pub fn aux_arrow_letter(ind: &AuxIndicator, a: &super::AuxRef) -> [f64; 2] {
    let sight = [-ind.ns[0], -ind.ns[1]];
    let mut side = [-sight[1], sight[0]];
    if side[1] < -1e-9 || (side[1].abs() <= 1e-9 && side[0] > 0.0) {
        side = [-side[0], -side[1]];
    }
    let moved = a.letter_at.map(|m| m[0]).unwrap_or_default();
    [ind.mid[0] + sight[0] * 3.35 + side[0] * 4.4 + moved[0], ind.mid[1] + sight[1] * 3.35 + side[1] * 4.4 + moved[1]]
}

/// Live (sheet inches → mm): line 0.879 in from the edge, 0.105 in past
/// each end of it; arrow shaft and head 0.18 in each, head 0.06 in wide;
/// letters 0.12 in.
pub const AUX_OFFSET: f64 = 22.33;
pub const AUX_PAST_EDGE: f64 = 2.667;
pub const AUX_SHAFT: f64 = 4.572;
pub const AUX_HEAD: f64 = 4.572;
pub const AUX_LETTER: f64 = 3.048;
/// The line's half length when the edge can't be measured.
pub const AUX_EXT: f64 = 10.0;

/// The reference indicator of auxiliary view `id`, `ppl` being its parent's
/// placement and `parent` its geometry (for the edge's extent).
pub fn aux_indicator(sheet: &Sheet, id: ViewId, ppl: &ViewPlacement, parent: Option<&ViewGeometry>) -> Option<AuxIndicator> {
    let v = sheet.view(id)?;
    let (ViewKind::Auxiliary { edge_dir, .. }, Some(a)) = (&v.kind, v.aux_ref.as_ref()) else { return None };
    let l = (edge_dir[0].powi(2) + edge_dir[1].powi(2)).sqrt().max(1e-12);
    let e = [edge_dir[0] / l, edge_dir[1] / l];
    let n = views::auxiliary_toward(sheet, id).unwrap_or([-e[1], e[0]]);
    let (es, ns) = (ppl.dir_to_sheet(e), ppl.dir_to_sheet(n));
    let c = ppl.to_sheet(a.anchor);
    // The edge's extent along itself (view coordinates → sheet mm), from the
    // parent's edges through the anchor parallel to it.
    let k = ppl.scale * 1000.0;
    let mut span: Option<(f64, f64)> = None;
    for edge in parent.map(|g| g.edges.as_slice()).unwrap_or(&[]) {
        for w in edge.points.windows(2) {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let dl = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if dl < 1e-12 || (d[0] * e[1] - d[1] * e[0]).abs() / dl > 1e-3 || views::dist_to_segment(a.anchor, w[0], w[1]) > 1e-6 {
                continue;
            }
            for p in [w[0], w[1]] {
                let t = (p[0] - a.anchor[0]) * e[0] + (p[1] - a.anchor[1]) * e[1];
                span = Some(match span {
                    None => (t, t),
                    Some((lo, hi)) => (lo.min(t), hi.max(t)),
                });
            }
        }
    }
    let (centre, half) = match span {
        Some((lo, hi)) => ((lo + hi) / 2.0 * k, (hi - lo) / 2.0 * k + AUX_PAST_EDGE),
        None => (0.0, AUX_EXT),
    };
    let offset = a.offset.unwrap_or(AUX_OFFSET);
    let ext = a.ext.unwrap_or([half; 2]);
    let base = [c[0] + ns[0] * offset + es[0] * centre, c[1] + ns[1] * offset + es[1] * centre];
    let at = |t: f64| [base[0] + es[0] * t, base[1] + es[1] * t];
    let elbows = [at(-ext[0]), at(ext[1])];
    let tips = elbows.map(|p| [p[0] - ns[0] * (AUX_SHAFT + AUX_HEAD), p[1] - ns[1] * (AUX_SHAFT + AUX_HEAD)]);
    let moved = a.letter_at.unwrap_or([[0.0; 2]; 2]);
    let letters = [0, 1].map(|i| [tips[i][0] - ns[0] * (AUX_LETTER * 0.8) + moved[i][0], tips[i][1] - ns[1] * (AUX_LETTER * 0.8) + moved[i][1]]);
    Some(AuxIndicator { anchor: c, es, ns, offset, ext, elbows, tips, mid: at((ext[1] - ext[0]) / 2.0), letters })
}

pub fn placement_of(sheet: &Sheet, id: ViewId, g: &ViewGeometry) -> Option<ViewPlacement> {
    let v = sheet.view(id)?;
    let scale = views::view_scale(sheet, id);
    let mut p = ViewPlacement::new(id, g.origin, v.position, scale);
    p.rotation = views::view_rotation(sheet, id);
    let k = scale * 1000.0;
    let mut brs: Vec<(usize, &super::ViewBreak)> = v.breaks.iter().enumerate().filter(|(_, b)| b.b > b.a).collect();
    brs.sort_by(|x, y| x.1.a.total_cmp(&y.1.a));
    let mut n = 0usize;
    for (i, b) in brs {
        let axis = if b.vertical { 0u8 } else { 1u8 };
        let overlaps = p.breaks[..n].iter().any(|o| o.axis == axis && b.a < o.b && b.b > o.a);
        if overlaps || n >= MAX_BREAKS {
            continue;
        }
        let band = (b.gap_mm.max(0.5) / k).min(b.b - b.a);
        p.breaks[n] = BreakMap { axis, a: b.a, b: b.b, band, index: i as u8 };
        n += 1;
    }
    p.nbreaks = n as u8;
    let corners = [g.min, [g.max[0], g.min[1]], g.max, [g.min[0], g.max[1]]];
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for c in corners {
        let s = p.to_sheet(c);
        for k in 0..2 {
            lo[k] = lo[k].min(s[k]);
            hi[k] = hi[k].max(s[k]);
        }
    }
    p.min = lo;
    p.max = hi;
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
        let geo = views.get(&view).filter(|g| g.frame.is_some());
        let ctx = views::view_source(sheet, view).and_then(|(key, _)| sources.get(&key)).map(|s| s.context.clone());
        let (Some(geo), Some(ctx)) = (geo, ctx) else {
            dangling.push(a.id);
            continue;
        };
        let mut ok = true;
        for r in &a.refs {
            let Some((p, radius)) = resolve_in_view(&ctx, geo, &r.point) else {
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
                (AnnotationKind::HoleCallout { center, radius: rr, .. }, 0) => {
                    *center = p;
                    if let Some(r) = radius {
                        *rr = r;
                    }
                }
                (AnnotationKind::CircleCenterline { points, .. }, k) => {
                    if let Some(x) = points.get_mut(k as usize) {
                        *x = p;
                    }
                }
                (AnnotationKind::VirtualSharp { a1: x, .. }, 0)
                | (AnnotationKind::VirtualSharp { b1: x, .. }, 1)
                | (AnnotationKind::VirtualSharp { a2: x, .. }, 2)
                | (AnnotationKind::VirtualSharp { b2: x, .. }, 3) => *x = p,
                (AnnotationKind::Balloon { anchor, .. }, 0) => *anchor = p,
                (AnnotationKind::OrdinateDimension { origin, .. }, 0) => *origin = p,
                (AnnotationKind::OrdinateDimension { point, .. }, 1) => *point = p,
                (AnnotationKind::ChamferDimension { a: x, .. }, 0)
                | (AnnotationKind::ChamferDimension { b: x, .. }, 1)
                | (AnnotationKind::ChamferDimension { c: x, .. }, 2)
                | (AnnotationKind::ChamferDimension { d: x, .. }, 3) => *x = p,
                (AnnotationKind::ArcLengthDimension { center, radius: rr, .. }, 0) => {
                    *center = p;
                    if let Some(r) = radius {
                        *rr = r;
                    }
                }
                (AnnotationKind::ArcLengthDimension { a: pa, .. }, 1) => *pa = p,
                (AnnotationKind::ArcLengthDimension { b: pb, .. }, 2) => *pb = p,
                (AnnotationKind::OrdinateGroup { origin, .. }, 0) => *origin = p,
                (AnnotationKind::OrdinateGroup { points, .. }, k) => {
                    if let Some(x) = points.get_mut(k as usize - 1) {
                        *x = p;
                    }
                }
                (AnnotationKind::MaxMinDimension { a: pa, ra, .. }, 0) => {
                    *pa = p;
                    if let (Some(r), true) = (radius, *ra > 0.0) {
                        *ra = r;
                    }
                }
                (AnnotationKind::MaxMinDimension { b: pb, rb, .. }, 1) => {
                    *pb = p;
                    if let (Some(r), true) = (radius, *rb > 0.0) {
                        *rb = r;
                    }
                }
                (AnnotationKind::ForeshortenedRadial { center, radius: rr, .. }, 0) => {
                    *center = p;
                    if let Some(r) = radius {
                        *rr = r;
                    }
                }
                (AnnotationKind::ForeshortenedDiameter { axis, .. }, k @ (0 | 1)) => axis[k as usize] = p,
                (AnnotationKind::ForeshortenedDiameter { edge, .. }, 2) => *edge = p,
                (AnnotationKind::DatumTarget { anchor, .. }, 0) => *anchor = p,
                // The target's origin (its position dimensions measure from it).
                (AnnotationKind::DatumTarget { .. }, 1) => {
                    if let Some(o) = a.target.as_mut() {
                        o.origin = Some(p);
                    }
                }
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
        for pair in ts.as_chunks::<2>().0 {
            let to_xy = |tt: f64| [(tt - wl) * s2, (tt + wl) * s2];
            out.push([to_xy(pair[0]), to_xy(pair[1])]);
        }
    }
    out
}

fn circle_points(c: [f64; 2], r: f64, n: usize) -> Vec<[f64; 2]> {
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            [c[0] + r * t.cos(), c[1] + r * t.sin()]
        })
        .collect()
}

/// One section hatch line `l` (unbroken sheet coordinates): clipped to a
/// detail's profile, the crop, and mapped through the view's breaks.
/// Drawing properties › Views pen `pen` on `lines`, each run with its own
/// line type's dashes.
fn pen_lines(g: &mut SheetGeometry, lines: std::ops::Range<usize>, pen: &super::PenProps) {
    let mut start = lines.start;
    while start < lines.end {
        let st = g.lines[start].style;
        let end = (start..lines.end).find(|&k| g.lines[k].style != st).unwrap_or(lines.end);
        g.pens.push(super::sheet::PenSpan { centered: false, lines: start..end, width_mm: pen.thickness, dashes: st.dashes().to_vec() });
        start = end;
    }
}

/// Colour `rgb` on the lines and fills added since `m` (not the texts).
fn color_lines_since(g: &mut SheetGeometry, m: super::sheet::GeomMark, rgb: Option<[u8; 3]>) {
    let Some(rgb) = rgb else { return };
    let span = super::sheet::ColorSpan { lines: m.0..g.lines.len(), texts: 0..0, fills: m.2..g.fills.len(), rgb };
    if !(span.lines.is_empty() && span.fills.is_empty()) {
        g.colors.push(span);
    }
}

/// A label's font and colour on the texts from `t0`.
fn label_since(g: &mut SheetGeometry, t0: usize, label: &super::LabelProps) {
    if t0 >= g.texts.len() {
        return;
    }
    if let Some(font) = label.font.clone() {
        g.fonts.push(super::sheet::FontSpan { texts: t0..g.texts.len(), font });
    }
    if let Some(rgb) = label.color {
        g.colors.push(super::sheet::ColorSpan { lines: 0..0, texts: t0..g.texts.len(), fills: 0..0, rgb });
    }
}

fn push_hatch_line(g: &mut SheetGeometry, pl: &ViewPlacement, geo: &ViewGeometry, l: [[f64; 2]; 2], pick: u32) {
    let ub = pl.unbroken();
    let mut pieces = vec![vec![ub.to_view(l[0]), ub.to_view(l[1])]];
    if let Some(poly) = &geo.clip_poly {
        pieces = pieces.into_iter().flat_map(|p| views::clip_polyline_polygon(&p, poly)).collect();
    } else if let Some((c, r)) = geo.clip {
        pieces = pieces.into_iter().flat_map(|p| views::clip_polyline_circle(&p, c, r)).collect();
    }
    if let Some(crop) = &geo.crop {
        pieces = pieces.into_iter().flat_map(|p| views::clip_to_crop(&p, crop)).collect();
    }
    for p in pieces {
        for q in map_polyline(pl, &p) {
            g.lines.push(Polyline2 { points: q, style: LineStyle::Thin, closed: false, pick });
        }
    }
}

/// Pieces of a view-coordinate polyline outside the view's break bands,
/// mapped to the sheet.
fn map_polyline(pl: &ViewPlacement, pts: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    let mut pieces = vec![pts.to_vec()];
    for br in pl.breaks() {
        pieces = pieces.into_iter().flat_map(|p| views::clip_polyline_band(&p, br.axis as usize, br.a, br.b)).collect();
    }
    pieces.into_iter().map(|p| p.into_iter().map(|q| pl.to_sheet(q)).collect()).collect()
}

/// Mass (kg) of the part the sheet's first base view shows: its override,
/// else from its material's density.
fn part_mass(drawing: &Drawing, sheet: &Sheet, sources: &Sources) -> Option<f64> {
    let base = sheet.views.iter().chain(drawing.sheets.iter().flat_map(|s| s.views.iter())).find(|v| matches!(v.kind, ViewKind::Base { .. }))?;
    let (k, body) = views::SourceKey::of_view(base)?;
    let src = sources.get(&k)?;
    // An assembly: its override, else the sum of its parts.
    if src.assembly && body.is_none() {
        if let Some(m) = src.own.as_ref().and_then(|o| o.mass_override) {
            return Some(m);
        }
        let masses: Vec<f64> = src
            .parts
            .iter()
            .filter_map(|p| p.mass_override.or_else(|| crate::regen::measure::mass_properties(&src.context, std::slice::from_ref(p.body.as_ref()?), &|_| p.density).and_then(|m| m.mass)))
            .collect();
        return (!masses.is_empty()).then(|| masses.iter().sum());
    }
    let info = src.part(body.as_ref())?;
    if let Some(m) = info.mass_override {
        return Some(m);
    }
    let id = info.body.clone()?;
    let rho = info.density;
    crate::regen::measure::mass_properties(&src.context, std::slice::from_ref(&id), &|_| rho).and_then(|m| m.mass)
}

/// Export layers (lines, texts) of an annotation's elements.
fn annotation_cad_layers(k: &AnnotationKind) -> (CadLayer, CadLayer) {
    use AnnotationKind as A;
    match k {
        A::Centermark { .. } => (CadLayer::Centermarks, CadLayer::Centermarks),
        A::Centerline { .. } | A::CircleCenterline { .. } => (CadLayer::Centerlines, CadLayer::Centerlines),
        A::VirtualSharp { .. } => (CadLayer::VirtualSharps, CadLayer::VirtualSharps),
        // Live drw2-k-dxf2018: a note's leader goes with its text; hole
        // callout, GD&T, datum, surface finish and weld leaders and symbol
        // lines on ANNOTATION_LINES.
        A::Balloon { .. } | A::HoleCallout { .. } | A::RevisionCallout { .. } | A::Gdt { .. } | A::Datum { .. } | A::SurfaceFinish { .. } | A::Weld { .. } => (CadLayer::AnnotationLines, CadLayer::AnnotationText),
        _ => (CadLayer::AnnotationText, CadLayer::AnnotationText),
    }
}

/// Lookup of property fields for a sheet.
pub fn field_value(drawing: &Drawing, si: usize, info: &SheetInfo, sources: &Sources, key: &str) -> Option<String> {
    let sheet = drawing.sheets.get(si)?;
    // Part shown by the first base view of this sheet (else of the drawing).
    let part = || {
        let base = sheet.views.iter().chain(drawing.sheets.iter().flat_map(|s| s.views.iter())).find(|v| matches!(v.kind, ViewKind::Base { .. }))?;
        let (k, part) = views::SourceKey::of_view(base)?;
        let src = sources.get(&k)?;
        // An assembly view shows the assembly's own Properties (its part
        // number, not its first part's; qa5 D4).
        let p = if src.assembly && part.is_none() { src.own.clone() } else { src.part(part.as_ref()).cloned() };
        Some((src.name.clone(), p, src.assembly))
    };
    Some(match key {
        "part.name" => part().map(|(n, p, asm)| if asm { n } else { p.map(|p| p.name).unwrap_or_default() }).unwrap_or_default(),
        "part.number" => part().and_then(|(_, p, _)| p).map(|p| p.number).unwrap_or_default(),
        "part.material" => part().and_then(|(_, p, _)| p).map(|p| p.material).unwrap_or_default(),
        "part.description" => part().and_then(|(_, p, _)| p).map(|p| p.description).unwrap_or_default(),
        "model.name" => part().map(|(n, _, _)| n).unwrap_or_default(),
        "doc.name" => info.doc_name.clone(),
        "user.name" => drawing.drawn_by.as_deref().unwrap_or(&info.user_name).to_uppercase(),
        // Release approver / date and the drawing's three title lines: empty
        // until set (the title block shows its "----" placeholders, live
        // drawing/156, 164).
        "release.approver" | "release.date" | "drawing.title1" | "drawing.title2" | "drawing.title3" => String::new(),
        "drawing.name" => info.drawing_name.clone(),
        // Who / when (the local user, today), as Onshape's drawn / changed by.
        "drawing.drawn_by" => drawing.drawn_by.as_deref().unwrap_or(&info.user_name).to_uppercase(),
        "drawing.changed_by" => info.user_name.to_uppercase(),
        // Live: M/D/YYYY in ANSI drawings ("10/19/2026"), ISO dates otherwise.
        "drawing.date_drawn" | "drawing.changed_date" => if sheet.title_style == super::TitleBlockStyle::Ansi { format::field_format(&format::today(), "m/d/yyyy") } else { format::today() },
        // Release / metadata properties with no value yet: "----".
        // An unreleased drawing is "In progress" (as parts, `ItemMeta::state`).
        "drawing.state" => crate::partstudio::RELEASE_STATES[0].to_string(),
        "drawing.category" => "Onshape Drawing".into(),
        "drawing.not_revision_managed" => "False".into(),
        "drawing.description" | "drawing.number" | "drawing.revision" | "drawing.revision_description" => String::new(),
        "drawing.inspection_count" => drawing.sheets.iter().flat_map(|s| s.annotations.iter()).filter(|a| a.inspection.is_some()).count().to_string(),
        // Sheet reference properties from the part's Properties.
        // Live (drw2 10-21 h): an unset revision reads "-".
        "part.revision" => part().and_then(|(_, p, _)| p).map(|p| if p.revision.is_empty() { "-".into() } else { p.revision }).unwrap_or_default(),
        "part.state" => part().and_then(|(_, p, _)| p).map(|p| p.meta.state().to_string()).unwrap_or_default(),
        "part.vendor" => part().and_then(|(_, p, _)| p).and_then(|p| p.meta.vendor).unwrap_or_default(),
        "part.project" => part().and_then(|(_, p, _)| p).and_then(|p| p.meta.project).unwrap_or_default(),
        "part.product_line" => part().and_then(|(_, p, _)| p).and_then(|p| p.meta.product_line).unwrap_or_default(),
        "part.title1" => part().and_then(|(_, p, _)| p).and_then(|p| p.meta.title1).unwrap_or_default(),
        "part.title2" => part().and_then(|(_, p, _)| p).and_then(|p| p.meta.title2).unwrap_or_default(),
        "part.title3" => part().and_then(|(_, p, _)| p).and_then(|p| p.meta.title3).unwrap_or_default(),
        "part.not_revision_managed" => part().and_then(|(_, p, _)| p).map(|p| if p.meta.not_revision_managed { "True" } else { "False" }.to_string()).unwrap_or_default(),
        "part.exclude_from_bom" => part().and_then(|(_, p, _)| p).map(|p| if p.meta.exclude_from_bom { "True" } else { "False" }.to_string()).unwrap_or_default(),
        "part.unit_of_measure" => part().and_then(|(_, p, _)| p).map(|p| p.meta.unit_of_measure.unwrap_or_else(|| "Each".into())).unwrap_or_default(),
        "part.tessellation" => part().and_then(|(_, p, _)| p).map(|p| p.tessellation.unwrap_or(crate::partstudio::TessQuality::Auto).label().to_string()).unwrap_or_default(),
        // Live: 3 decimals in the drawing's mass unit ("0.052 LB"); "----"
        // without a material.
        "part.mass" => part_mass(drawing, sheet, sources).map(|m| if drawing.units_mm { format!("{m:.3} kg") } else { format!("{:.3} lb", m / 0.453_592_37) }).unwrap_or_default(),
        "part.category" => part().map(|(_, _, asm)| if asm { "Onshape Assembly" } else { "Onshape Part" }.to_string()).unwrap_or_default(),
        "part.subassembly_bom" => String::new(),
        "sheet.name" => sheet.name.clone(),
        "sheet.number" => (si + 1).to_string(),
        "sheet.count" => drawing.sheets.len().to_string(),
        "sheet.scale" => if sheet.views.is_empty() && !sheet.scale_shown { String::new() } else { format_scale(sheet.scale) },
        "sheet.size" => sheet.size.label().to_string(),
        "date" => format::today(),
        "units" => if drawing.units_mm { "mm".into() } else { "in".into() },
        _ => return None,
    })
}

/// Onshape-style hole / thread callout text for the hole a callout points
/// at: "2X ⌀6.6 THRU", "⌴ ⌀11 ↧ 6.5", "⌵ ⌀12.4 X 90°", tapped "M6x1.0 ↧ 10"
/// over "⌀5 ↧ 12".
pub fn hole_callout_lines(drawing: &Drawing, sheet: &Sheet, sources: &Sources, a: &Annotation) -> Option<Vec<String>> {
    hole_callout_marked(drawing, sheet, sources, a).map(|lines| lines.iter().map(|l| format::flatten_stacked(l)).collect())
}

/// Hole callout lines with the fit deviations of a diameter marked as
/// stacked text (see [`format::stacked`]).
fn hole_callout_marked(drawing: &Drawing, sheet: &Sheet, sources: &Sources, a: &Annotation) -> Option<Vec<String>> {
    let view = a.kind.view()?;
    let (key, _) = views::view_source(sheet, view)?;
    let ctx = &sources.get(&key)?.context;
    let r = a.refs.iter().find(|r| r.slot == 0)?;
    let (c, radius) = views::resolve_point3(ctx, &r.point)?;
    let radius = radius?;
    let tol = radius * 1e-3 + 1e-7;
    let hole = ctx.holes.iter().find(|h| h.matches_circle(c, radius, tol))?;
    let opts = a.hole.as_deref().cloned().unwrap_or_default();
    let n = hole_count(ctx, hole, opts.count);
    // Without "Override precision/tolerance" the hole feature's precision
    // applies (the callout's units always do).
    let format = if opts.override_tol { a.format.clone() } else { super::DimFormat { precision: None, tolerance: super::Tolerance::None, ..a.format.clone() } };
    let a = &Annotation { format, ..a.clone() };
    // The callout's lines with lengths written by `len`; `dual`: the dual
    // block (fit classes and deviations stay on the primary, as live's
    // dimensions).
    let build = |len: &dyn Fn(f64) -> String, dual: bool| -> Vec<String> {
        let count = if n > 1 { format!("{n}{} ", drawing.props.hole_indicator) } else { String::new() };
        let depth = |d: Option<f64>| match (hole.through, d) {
            (true, _) => " THRU".to_string(),
            (false, Some(d)) => format!(" ↧ {}", len(d)),
            _ => String::new(),
        };
        let mut out = Vec::new();
        match &hole.thread {
            Some(t) => {
                let class = hole.thread_class.as_ref().map(|c| format!(" - {c}")).unwrap_or_default();
                out.push(format!("{count}{t}{class}{}", depth(hole.tapped_depth.or(hole.depth))));
                if let Some(td) = hole.tap_drill {
                    out.push(format!("⌀{}{}", len(td), depth(hole.depth)));
                }
            }
            None => match &hole.diameter_fit {
                // Fit tolerances (live, dialogs/hole-35): "⌀6.350 H7 THRU",
                // "⌀6.350 H7 +0.015/−0.000 THRU" and "⌀6.350 +0.015/−0.000 THRU"
                // with the deviations stacked; a precision override of the
                // tolerance applies to the size and the deviations.
                Some(fit) => {
                    let f = super::DimFormat { precision: fit.precision.or(a.format.precision).or(drawing.props.fit_tolerance_precision), ..a.format.clone() };
                    let class = fit.class.as_ref().map(|c| format!(" {c}")).unwrap_or_default();
                    let dev = fit.deviations.map(|(u, l)| format!(" {}", format::stacked(&format::format_deviation(drawing, &f, u, false), &format::format_deviation(drawing, &f, l, true)))).unwrap_or_default();
                    if dual {
                        out.push(format!("{count}⌀{}{}", len(hole.diameter), depth(hole.depth)));
                    } else {
                        out.push(format!("{count}⌀{}{class}{dev}{}", format::format_len(drawing, &f, hole.diameter), depth(hole.depth)));
                    }
                }
                None => {
                    // An overridden tolerance on the diameter (live drawing/465).
                    let dev = |v: f64, lower: bool| format::format_deviation(drawing, &a.format, v, lower);
                    let tol = match &a.format.tolerance {
                        super::Tolerance::Symmetric { value } => format!(" ±{}", len(*value)),
                        super::Tolerance::Deviation { upper, lower } => format!(" {}", format::stacked(&dev(*upper, false), &dev(*lower, true))),
                        super::Tolerance::Min => " MIN".into(),
                        super::Tolerance::Max => " MAX".into(),
                        super::Tolerance::Fit { hole, .. } if !hole.is_empty() => format!(" {hole}"),
                        _ => String::new(),
                    };
                    let size = match &a.format.tolerance {
                        super::Tolerance::Limits { upper, lower } => format::stacked(&len(hole.diameter + upper), &len(hole.diameter + lower)),
                        _ => len(hole.diameter),
                    };
                    out.push(format!("{count}⌀{size}{tol}{}", depth(hole.depth)))
                }
            },
        }
        if let Some((d, dd)) = hole.cbore {
            out.push(format!("⌴ ⌀{} ↧ {}", len(d), len(dd)));
        }
        if let Some((d, ang)) = hole.csink {
            out.push(format!("⌵ ⌀{} X {}°", len(d), format::format_number(ang.to_degrees(), None, 1)));
        }
        out
    };
    let primary = |m: f64| format::format_len(drawing, &a.format, m);
    // Dual (live asb 10-24 (3)): Top / Bottom, the bracketed dual block above
    // / below the primary; Left / Right, each value's bracketed dual before /
    // after it.
    let dual = |m: f64| format::dual_len(drawing, &a.format, m);
    let mut out = match (dual(0.0).is_some(), drawing.props.dual.hole_location) {
        (false, _) => build(&primary, false),
        (true, loc @ (super::DualLocation::Top | super::DualLocation::Bottom)) => dual_block(build(&primary, false), build(&|m| dual(m).unwrap_or_default(), true), loc == super::DualLocation::Top),
        (true, super::DualLocation::Left) => build(&|m| format!("[{}] {}", dual(m).unwrap_or_default(), primary(m)), false),
        (true, super::DualLocation::Right) => build(&|m| format!("{} [{}]", primary(m), dual(m).unwrap_or_default()), false),
    };
    // Prefix / suffix around the callout, reference parentheses, the
    // lines above and below.
    let f = &a.format;
    if let Some(first) = out.first_mut() {
        *first = format!("{}{}{first}", if f.reference { "(" } else { "" }, f.prefix);
    }
    if let Some(last) = out.last_mut() {
        *last = format!("{last}{}{}", f.suffix, if f.reference { ")" } else { "" });
    }
    if !f.above.is_empty() {
        out.insert(0, f.above.clone());
    }
    if !f.below.is_empty() {
        out.push(f.below.clone());
    }
    Some(out)
}

/// A hole callout with its dual block (live asb 10-24 (3)): the dual lines
/// in one pair of brackets, above (`top`) or below the primary lines.
pub(crate) fn dual_block(primary: Vec<String>, mut dual: Vec<String>, top: bool) -> Vec<String> {
    if let Some(first) = dual.first_mut() {
        first.insert(0, '[');
    }
    if let Some(last) = dual.last_mut() {
        last.push(']');
    }
    if top { dual.into_iter().chain(primary).collect() } else { primary.into_iter().chain(dual).collect() }
}

/// The hole count a callout's Count type gives (the panel's "1X").
pub fn hole_callout_count(sheet: &Sheet, sources: &Sources, a: &Annotation) -> Option<usize> {
    let (key, _) = views::view_source(sheet, a.kind.view()?)?;
    let ctx = &sources.get(&key)?.context;
    let (c, radius) = views::resolve_point3(ctx, &a.refs.iter().find(|r| r.slot == 0)?.point)?;
    let radius = radius?;
    let hole = ctx.holes.iter().find(|h| h.matches_circle(c, radius, radius * 1e-3 + 1e-7))?;
    Some(hole_count(ctx, hole, a.hole.as_ref().map(|o| o.count).unwrap_or_default()))
}

/// How many holes a callout counts (its Count type, live drawing/461).
pub fn hole_count(ctx: &crate::regen::context::Context, hole: &crate::regen::context::HoleRecord, count: super::HoleCount) -> usize {
    use super::HoleCount as C;
    let mm = |x: Option<f64>| x.map(|d| (d * 1e6).round() as i64);
    let pair = |x: Option<(f64, f64)>| x.map(|(p, q)| ((p * 1e6).round() as i64, (q * 1e6).round() as i64));
    let same_size = |h: &&crate::regen::context::HoleRecord| mm(Some(h.diameter)) == mm(Some(hole.diameter)) && mm(h.depth) == mm(hole.depth) && h.through == hole.through && h.thread == hole.thread && pair(h.cbore) == pair(hole.cbore) && pair(h.csink) == pair(hole.csink);
    let same_face = |h: &&crate::regen::context::HoleRecord| h.dir.dot(hole.dir) > 1.0 - 1e-9 && (h.start - hole.start).dot(hole.dir).abs() < 1e-7;
    let holes = ctx.holes.iter().filter(same_size);
    match count {
        C::None => 1,
        C::View | C::Part => holes.count(),
        C::Pattern => holes.filter(|h| h.feature == hole.feature).count(),
        C::Face | C::FaceExcludingPatterns => holes.filter(|h| h.feature == hole.feature).filter(same_face).count(),
    }
}

struct Env<'a> {
    drawing: &'a Drawing,
    sheet: &'a Sheet,
    placements: &'a [ViewPlacement],
    views: &'a HashMap<ViewId, Arc<ViewGeometry>>,
    sources: &'a Sources,
    fields: &'a dyn Fn(&str) -> Option<String>,
    revisions: &'a [tables::RevisionRow],
    /// Lines of expanded texts that hold property fields (for their grey
    /// background on screen).
    used: &'a std::cell::RefCell<Vec<String>>,
}

impl AnnotationEnv for Env<'_> {
    fn to_sheet(&self, view: ViewId, p: [f64; 2]) -> [f64; 2] {
        self.placements.iter().find(|pl| pl.id == view).map(|pl| pl.to_sheet(p)).unwrap_or(p)
    }
    fn scale(&self, view: ViewId) -> f64 {
        self.placements.iter().find(|pl| pl.id == view).map(|pl| pl.scale * 1000.0).unwrap_or(1000.0)
    }
    fn rotation(&self, view: ViewId) -> f64 {
        self.placements.iter().find(|pl| pl.id == view).map(|pl| pl.rotation).unwrap_or(0.0)
    }
    fn dual_cad(&self, a: &Annotation, angular: bool) -> Option<super::cad_objects::CadDual> {
        let p = &self.drawing.props;
        if angular || !a.format.dual.unwrap_or(p.dual.show) {
            return None;
        }
        let primary = format::length_units(self.drawing, &a.format);
        let du = p.dual.units_for(!primary.imperial());
        let post = if p.dual.show_unit && !du.symbol().is_empty() { format!("[] {}", du.symbol()) } else { "[]".to_string() };
        Some(super::cad_objects::CadDual { factor: primary.meters() / du.meters(), precision: p.dual.precision.unwrap_or(3), post, below: p.dual.dim_location == super::DualLocation::Bottom })
    }
    fn text(&self, a: &Annotation, value: f64, angular: bool, symbol: &str) -> DimText {
        let mut f = a.format.clone();
        f.prefix = self.expand(&f.prefix);
        f.suffix = self.expand(&f.suffix);
        f.above = self.expand(&f.above);
        f.below = self.expand(&f.below);
        format::dim_text(self.drawing, &f, value, angular, symbol)
    }
    fn expand(&self, s: &str) -> String {
        let out = format::expand_fields_with(s, self.fields, if self.drawing.props.hide_placeholder { "" } else { "----" });
        if s.contains('{') {
            self.used.borrow_mut().extend(out.lines().map(str::to_string));
        }
        out
    }
    fn sheet_style(&self) -> super::sheet::SheetStyle {
        let p = &self.drawing.props;
        super::sheet::SheetStyle { dim_text: p.dim_text_height, dim_arrow: p.dim_arrow, dim_arrow_type: p.dim_arrow_type, dim_color: p.dim_color, ann_text: p.ann_text_height, ann_arrow: p.ann_arrow, ann_arrow_type: p.ann_arrow_type, ann_color: p.ann_color, dim_font: p.dim_font.clone(), ann_font: p.ann_font.clone(), text_gap: p.dim_text_gap, geometry_gap: p.dim_geometry_gap, extension: p.dim_extension, text_horizontal: p.dim_text_horizontal, ann_types: p.ann_types.clone(), datum_size: p.datum_size, callout_justify: p.callout_justify, callout_stack: p.callout_stack, callout_stack_gap: p.callout_stack_gap, target_hatch: p.target_hatch, underline_overridden: p.underline_overridden, lock_jog_45: p.lock_jog_45, ordinate_iso: !self.drawing.ordinate_ansi(), iso: !self.drawing.is_ansi(), virtual_sharp_extension: p.view_tab.virtual_sharp_extension, virtual_sharp_pen: p.view_tab.virtual_sharp_line.thickness, virtual_sharp_color: p.view_tab.virtual_sharp_line.color, revision_callout: super::sheet::RevisionCalloutStyle { shape: p.table_styles.revision_callout_shape, chars: p.table_styles.revision_callout_chars, font: p.table_styles.revision_callout_font.clone(), text: p.table_styles.revision_callout_text.clone() } }
    }
    fn inspection_style(&self) -> Option<super::sheet::InspectionStyle> {
        let p = &self.drawing.props;
        (!p.hide_inspection).then(|| super::sheet::InspectionStyle { shape: p.inspection_shape, text_height: p.inspection_text_height, border: p.inspection_border_size, chars: p.inspection_border_chars, color: p.inspection_color, solid: p.inspection_solid_fill, font: p.inspection_font.clone(), text_color: p.inspection_text_color, fill_color: p.inspection_fill_color })
    }
    fn weld_ansi(&self) -> bool {
        self.drawing.weld_ansi()
    }
    fn chamfer_style(&self) -> super::sheet::ChamferStyle {
        let p = &self.drawing.props;
        super::sheet::ChamferStyle { dim45: p.chamfer_45_dimension, content45: p.chamfer_45_content, dim: p.chamfer_dimension, content: p.chamfer_content, length_precision: p.chamfer_length_precision, angle_precision: p.chamfer_angle_precision }
    }
    fn revision_label(&self, revision: &str) -> Option<String> {
        tables::revision_callout_label(self.drawing, self.revisions, revision)
    }
    fn hole_callout(&self, a: &Annotation) -> Vec<String> {
        hole_callout_marked(self.drawing, self.sheet, self.sources, a).unwrap_or_else(|| vec!["⌀?".into()])
    }
    fn balloon_text(&self, a: &Annotation, text: &str) -> String {
        let (part, anchor) = if let AnnotationKind::Balloon { callout, view, anchor, .. } = &a.kind { (callout.part.clone(), Some((*view, *anchor))) } else { (None, None) };
        // Callouts made before the part was stored (qa5 D8): the part of the
        // view edge at the anchor.
        let body = a.refs.iter().find(|r| r.slot == 0).map(|r| r.point.body().clone()).or(part).or_else(|| anchor.and_then(|(v, p)| self.part_at(v, p)));
        self.callout_text(a, body.as_ref(), text)
    }
    fn stacked_text(&self, a: &Annotation, st: &super::StackedCallout) -> String {
        self.callout_text(a, st.body.as_ref(), &st.text)
    }
}

impl Env<'_> {
    /// The part of view `view`'s edge nearest to `p` (view coordinates),
    /// within 2 mm on the sheet.
    fn part_at(&self, view: ViewId, p: [f64; 2]) -> Option<crate::reference::BodyId> {
        let g = self.views.get(&view)?;
        let pl = self.placements.iter().find(|pl| pl.id == view)?;
        let s = pl.to_sheet(p);
        g.edges
            .iter()
            .filter_map(|e| {
                let me = g.model_edges.get(e.model? as usize)?;
                let d = e.points.windows(2).map(|w| views::dist_to_segment(s, pl.to_sheet(w[0]), pl.to_sheet(w[1]))).fold(f64::INFINITY, f64::min);
                (d < 2.0).then(|| (d, me.body.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, b)| b)
    }

    /// A callout's text: its own (empty: the BOM item number), with the
    /// properties of its part `body` (`{part.*}`) and that part's BOM row
    /// (`{bom.*}`) filled in; "----" when there is none.
    fn callout_text(&self, a: &Annotation, body: Option<&crate::reference::BodyId>, text: &str) -> String {
        let t = text.trim();
        let text = if t.is_empty() || t == "#" { "{bom.item}" } else { text };
        let src = a.kind.view().and_then(|v| views::view_source(self.sheet, v)).and_then(|(key, _)| self.sources.get(&key));
        let part = src.zip(body).and_then(|(s, b)| s.part(Some(b)));
        let row = src.zip(body).and_then(|(s, b)| s.bom.iter().find(|r| r.bodies.contains(b)));
        let lookup = |k: &str| -> Option<String> {
            Some(match k {
                "part.name" => part.map(|p| p.name.clone()).unwrap_or_default(),
                "part.number" => part.map(|p| p.number.clone()).unwrap_or_default(),
                "part.material" => part.map(|p| p.material.clone()).unwrap_or_default(),
                "part.description" => part.map(|p| p.description.clone()).unwrap_or_default(),
                "bom.item" => row.map(|r| r.item.to_string()).unwrap_or_default(),
                "bom.quantity" => row.map(|r| r.quantity.to_string()).unwrap_or_default(),
                "bom.number" => row.map(|r| r.number.clone()).unwrap_or_default(),
                "bom.name" => row.map(|r| r.name.clone()).unwrap_or_default(),
                "bom.material" => row.map(|r| r.material.clone()).unwrap_or_default(),
                "instance.designator" => String::new(),
                _ => return (self.fields)(k),
            })
        };
        format::expand_fields_with(text, &lookup, "----")
    }
}

/// Assemble sheet `si` from computed view geometry.
pub fn assemble_sheet(drawing: &Drawing, si: usize, views: &HashMap<ViewId, Arc<ViewGeometry>>, sources: &Sources, info: &SheetInfo) -> BuiltSheet {
    let empty = Sheet::default();
    let sheet = drawing.sheets.get(si).unwrap_or(&empty);
    let (w, h) = sheet.size.size_mm();
    let mut g = SheetGeometry::new(w, h);
    let mut picks = Picks { table: vec![PickTarget::None], index: HashMap::new() };
    let fields = |k: &str| field_value(drawing, si, info, sources, k);
    let placeholder = if drawing.props.hide_placeholder { "" } else { "----" };
    let used = std::cell::RefCell::new(Vec::<String>::new());
    // Texts made from a `{field}` (the title block's property values are
    // exported in Onshape-Standard-Fields).
    let field_texts = std::cell::RefCell::new(Vec::<String>::new());
    // Title block values are properties (the drawing's or the part's).
    let expand = |s: &str| {
        let out = format::expand_fields_with(s, &fields, placeholder);
        used.borrow_mut().extend(out.lines().map(str::to_string));
        if s.contains('{') {
            field_texts.borrow_mut().extend(out.lines().map(str::to_string));
        }
        out
    };

    // Border and title block; everything inside the title block is pickable
    // as the title block.
    let mark_l = g.lines.len();
    let mark_t = g.texts.len();
    match &sheet.imported {
        Some(space) => g.add_imported(space),
        None => g.add_border_and_title_full(sheet, drawing.projection, &expand),
    }
    let title_rect = super::sheet::title_block_rect(sheet);
    let tb_rect = title_rect.unwrap_or([0.0; 4]);
    let in_tb = |p: &[f64; 2]| p[0] >= tb_rect[0] - 0.01 && p[1] <= tb_rect[3] + 0.01 && p[0] <= tb_rect[2] + 0.01 && p[1] >= tb_rect[1] - 0.01;
    // Formats › Locked: the title block can't be picked.
    let tb = if drawing.props.formats_locked { 0 } else { picks.id(PickTarget::TitleBlock) };
    let fp = &drawing.props;
    let (end_l, end_t) = (g.lines.len(), g.texts.len());
    for i in mark_l..end_l {
        let l = &mut g.lines[i];
        let in_block = l.points.iter().all(in_tb) && l.style != LineStyle::Border;
        if in_block {
            l.pick = tb;
        }
        // Formats layers and colors: border frame, zones (dividers), title
        // block.
        let layer = if in_block { SheetLayer::TitleBlock } else if l.style == LineStyle::Border { SheetLayer::BorderFrame } else { SheetLayer::BorderZones };
        g.layers.push(LayerSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, layer });
        if let Some(rgb) = layer.color(fp) {
            g.colors.push(super::sheet::ColorSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, rgb });
        }
        if let Some(width_mm) = layer.thickness(fp) {
            let dashes = g.lines[i].style.dashes().to_vec();
            g.pens.push(super::sheet::PenSpan { centered: false, lines: i..i + 1, width_mm, dashes });
        }
    }
    for i in mark_t..end_t {
        let in_block = in_tb(&g.texts[i].pos);
        if in_block {
            g.texts[i].pick = tb;
            if field_texts.borrow().contains(&g.texts[i].text) {
                g.cad_text_styles.push(super::cad_layers::CadTextStyleSpan { texts: i..i + 1, style: CadTextStyle::Fields });
            }
        }
        let layer = if in_block { SheetLayer::TitleBlock } else { SheetLayer::BorderZones };
        g.layers.push(LayerSpan { lines: 0..0, texts: i..i + 1, fills: 0..0, layer });
        if let Some(rgb) = layer.color(fp) {
            g.colors.push(super::sheet::ColorSpan { lines: 0..0, texts: i..i + 1, fills: 0..0, rgb });
        }
    }

    // Field cells stay editable notes under Formats › Locked.
    let title_cells: Vec<(u32, [f64; 4])> = std::mem::take(&mut g.title_cells)
        .into_iter()
        .map(|(key, r, texts)| {
            let id = picks.id(PickTarget::TitleField(key));
            g.texts[texts].iter_mut().for_each(|t| t.pick = id);
            (id, r)
        })
        .collect();

    let mut placements: Vec<ViewPlacement> = Vec::new();
    // Parent mirrors of linked views have no place on this sheet.
    for v in sheet.views.iter().filter(|v| !v.id.is_ghost() || v.id.is_child_ghost()) {
        let Some(geo) = views.get(&v.id) else { continue };
        let Some(pl) = placement_of(sheet, v.id, geo) else { continue };
        placements.push(pl);
    }
    // The sheet itself, for dimensions on drawing sketch geometry.
    placements.push(ViewPlacement::new(super::SHEET_VIEW, [0.0, 0.0], [0.0, 0.0], 1.0));
    let place = |id: ViewId| placements.iter().find(|p| p.id == id).copied();

    for v in &sheet.views {
        if v.id.is_ghost() && !v.id.is_child_ghost() {
            continue;
        }
        let (Some(geo), Some(pl)) = (views.get(&v.id), place(v.id)) else { continue };
        let view_pick = picks.id(PickTarget::View(v.id));
        // Mirrors of views on other sheets draw nothing of their own; a
        // linked child's mirror draws only its indicator on the parent.
        'own: {
            if v.id.is_ghost() {
                break 'own;
            }
            // Projected edges: visible ones, then tangent and hidden ones (each
            // a run, so Drawing properties › Views colors them with one span).
            let vt = &drawing.props.view_tab;
            let edges0 = g.lines.len();
            for pass in 0..3 {
                let mark = g.mark();
                // The pass's own pen and colour go in ahead of the per-edge
                // ones (Adjust linestyle wins: the last span covering a line).
                let (pens0, colors0) = (g.pens.len(), g.colors.len());
                for (i, e) in geo.edges.iter().enumerate() {
                    // Show/hide edges: hidden edges are left out, except while
                    // the dialog is open (thick grey, like occluded edges).
                    let marked = !v.hidden_edges.is_empty()
                        && e.model.and_then(|m| geo.model_edges.get(m as usize)).is_some_and(|me| v.hidden_edges.iter().any(|(b, n)| *b == me.body && n == &*me.name));
                    if marked && !v.editing_edges {
                        continue;
                    }
                    let style = match (e.visible, e.class) {
                        _ if e.sketch != views::SketchEdge::None => continue,
                        (_, EdgeClass::Seam) => continue,
                        (_, _) if marked => LineStyle::Visible,
                        (false, EdgeClass::Sharp | EdgeClass::Outline) if v.editing_edges => LineStyle::Hidden,
                        (true, EdgeClass::Smooth) if v.show_tangent_edges && v.tangent_phantom => LineStyle::Phantom,
                        (true, EdgeClass::Smooth) if v.show_tangent_edges => LineStyle::Tangent,
                        (true, EdgeClass::Smooth) => continue,
                        (false, EdgeClass::Smooth) => continue,
                        (true, _) => LineStyle::Visible,
                        (false, _) if v.style == RenderStyle::HiddenLinesVisible => LineStyle::Hidden,
                        (false, _) => continue,
                    };
                    let p = match style {
                        LineStyle::Phantom | LineStyle::Tangent => 1,
                        LineStyle::Hidden => 2,
                        _ => 0,
                    };
                    if p != pass {
                        continue;
                    }
                    let pick = picks.id(PickTarget::Edge { view: v.id, edge: i as u32 });
                    let l0 = g.lines.len();
                    for piece in map_polyline(&pl, &e.points) {
                        g.lines.push(Polyline2 { points: piece, style, closed: false, pick });
                    }
                    if v.editing_edges && (marked || !e.visible) {
                        let lines = l0..g.lines.len();
                        if marked {
                            g.pens.push(super::sheet::PenSpan { centered: false, lines: lines.clone(), width_mm: 0.7, dashes: Vec::new() });
                        }
                        g.colors.push(super::sheet::ColorSpan { lines, texts: 0..0, fills: 0..0, rgb: [170, 170, 170] });
                        continue;
                    }
                    // Adjust linestyle: this edge's own pen and color.
                    if let Some(es) = e.model.and_then(|m| geo.model_edges.get(m as usize)).and_then(|me| v.edge_styles.iter().find(|s| s.body == me.body && s.name == *me.name)) {
                        let lines = l0..g.lines.len();
                        g.pens.push(super::sheet::PenSpan { centered: false, lines: lines.clone(), width_mm: es.thickness, dashes: es.pattern.dashes(es.dashes) });
                        if let Some(p) = g.pens.last_mut() {
                            p.centered = true;
                        }
                        if let Some(rgb) = es.color {
                            g.colors.push(super::sheet::ColorSpan { lines, texts: 0..0, fills: 0..0, rgb });
                        }
                    }
                }
                // Drawing properties › Views › Visible / Tangent / Hidden edges:
                // thickness, dash scale and colour.
                let lines = mark.0..g.lines.len();
                let (thickness, dash_scale, color) = match pass {
                    0 => (vt.visible_thickness, 1.0, vt.visible_color),
                    1 => (vt.tangent_thickness, vt.tangent_dashes, vt.tangent_color),
                    _ => (vt.hidden_thickness, vt.hidden_dashes, vt.hidden_color),
                };
                if !lines.is_empty() {
                    if let Some(rgb) = color {
                        g.colors.insert(colors0, super::sheet::ColorSpan { lines: lines.clone(), texts: 0..0, fills: 0..0, rgb });
                    }
                    let style = g.lines[lines.start].style;
                    if (thickness - style.width_mm()).abs() > 1e-9 || (dash_scale - 1.0).abs() > 1e-9 {
                        // Phantom tangent edges share the pass: each run gets
                        // its own line type's dashes.
                        let mut start = lines.start;
                        while start < lines.end {
                            let st = g.lines[start].style;
                            let end = (start..lines.end).find(|&k| g.lines[k].style != st).unwrap_or(lines.end);
                            let dashes = st.dashes().iter().map(|d| d * dash_scale.max(0.01)).collect();
                            g.pens.insert(pens0, super::sheet::PenSpan { centered: false, lines: start..end, width_mm: thickness, dashes });
                            start = end;
                        }
                    }
                }
            }
            // Sketches drawn in the view: Construction geometry › Part
            // Studio sketches' Solid / Construction lines pens and colours,
            // then their free points (live 10-26 c: "+" arms and "×"
            // diagonals of the size, circle Ø size; the Solid lines pen).
            for kind in [views::SketchEdge::Solid, views::SketchEdge::Construction] {
                let mark = g.mark();
                let (pen, style, dashes) = match kind {
                    views::SketchEdge::Construction => (&vt.sketch_construction, LineStyle::Centerline, super::SKETCH_CONSTRUCTION_DASHES.to_vec()),
                    _ => (&vt.sketch_solid, LineStyle::Visible, Vec::new()),
                };
                // View properties › Show sketch appearances: each sketch in
                // its appearance colour (live asb 10-23 (9)), else the pen's.
                let mut own: Vec<(std::ops::Range<usize>, [u8; 3])> = Vec::new();
                for (i, e) in geo.edges.iter().enumerate().filter(|(_, e)| e.sketch == kind) {
                    let pick = picks.id(PickTarget::Edge { view: v.id, edge: i as u32 });
                    let edge_mark = g.mark();
                    for piece in map_polyline(&pl, &e.points) {
                        g.lines.push(Polyline2 { points: piece, style, closed: false, pick });
                    }
                    if v.sketch_appearances && let Some((_, c)) = geo.sketch_colors.iter().find(|(k, _)| *k == i) {
                        own.push((edge_mark.0..g.lines.len(), *c));
                    }
                }
                if mark.0 < g.lines.len() {
                    g.pens.push(super::sheet::PenSpan { centered: false, lines: mark.0..g.lines.len(), width_mm: pen.thickness, dashes });
                    color_lines_since(&mut g, mark, pen.color);
                }
                for (lines, rgb) in own {
                    g.colors.push(super::sheet::ColorSpan { lines, texts: 0..0, fills: 0..0, rgb });
                }
            }
            let shape = vt.sketch_point_shape;
            if !geo.sketch_points.is_empty() && shape != super::SketchPointShape::None {
                use super::SketchPointShape as S;
                let mark = g.mark();
                let s = vt.sketch_point_size;
                let d = s * std::f64::consts::FRAC_1_SQRT_2;
                for p in &geo.sketch_points {
                    let Some(c) = map_polyline(&pl, &[*p, *p]).into_iter().find_map(|x| x.first().copied()) else { continue };
                    let mut strokes = Vec::new();
                    if matches!(shape, S::Centermark | S::CentermarkCircle) {
                        strokes.push(vec![[c[0] - s, c[1]], [c[0] + s, c[1]]]);
                        strokes.push(vec![[c[0], c[1] - s], [c[0], c[1] + s]]);
                    }
                    if matches!(shape, S::Cross | S::CrossCircle) {
                        strokes.push(vec![[c[0] - d, c[1] - d], [c[0] + d, c[1] + d]]);
                        strokes.push(vec![[c[0] - d, c[1] + d], [c[0] + d, c[1] - d]]);
                    }
                    for points in strokes {
                        g.lines.push(Polyline2 { points, style: LineStyle::Visible, closed: false, pick: view_pick });
                    }
                    if matches!(shape, S::Circle | S::CentermarkCircle | S::CrossCircle) {
                        g.lines.push(Polyline2 { points: circle_points(c, s / 2.0, 32), style: LineStyle::Visible, closed: true, pick: view_pick });
                    }
                }
                if mark.0 < g.lines.len() {
                    g.pens.push(super::sheet::PenSpan { centered: false, lines: mark.0..g.lines.len(), width_mm: vt.sketch_solid.thickness, dashes: Vec::new() });
                    color_lines_since(&mut g, mark, vt.sketch_point_color);
                }
            }
            g.view_edges.push((v.id, edges0..g.lines.len()));
            // Section hatching: computed on the unbroken view, then cropped and
            // broken in view coordinates.
            let cad_mark = g.mark();
            let (hatch_pens0, hatch_colors0) = (g.pens.len(), g.colors.len());
            if !geo.section_segments.is_empty() {
                let ub = pl.unbroken();
                let segs: Vec<[[f64; 2]; 2]> = geo.section_segments.iter().map(|s| [ub.to_sheet(s[0]), ub.to_sheet(s[1])]).collect();
                let hatch_l0 = g.lines.len();
                let whole = geo.clip_poly.is_none() && geo.clip.is_none() && geo.crop.is_none() && pl.breaks().is_empty();
                // Edit hatch section...: restyled regions (the rest keep the
                // drawing's hatch; a cut that doesn't close keeps it all).
                let regions = if v.section_hatches.is_empty() { None } else { views::section_regions(&geo.section_segments) };
                // Drawing properties › Views › View and region hatches: a
                // pattern other than ANSI31 hatches each cut region with it.
                let (hstd, hpat) = vt.hatch_pattern;
                let default_pattern = hstd == 0 && matches!(hpat, 0 | 8);
                let patterned = (!default_pattern && regions.is_none()).then(|| views::section_regions(&geo.section_segments)).flatten();
                if let Some(regions) = &patterned {
                    let ring = |l: &[[f64; 2]]| l.iter().map(|q| ub.to_sheet(*q)).collect::<Vec<_>>();
                    for (outer, holes) in regions {
                        let hs: Vec<Vec<[f64; 2]>> = holes.iter().map(|x| ring(x)).collect();
                        for l in super::sheet_sketch::hatch_pattern_holes(&ring(outer), &hs, hstd, hpat, 1.0, 0.0) {
                            push_hatch_line(&mut g, &pl, geo, l, view_pick);
                        }
                    }
                }
                match regions {
                    None if patterned.is_some() => {}
                    None => {
                        for l in hatch(&segs, vt.hatch_spacing.max(0.3)) {
                            push_hatch_line(&mut g, &pl, geo, l, view_pick);
                        }
                        // An uncut, unbroken section: the boundary for the HATCH entity.
                        if whole && let Some(loops) = super::cad_layers::chain_loops(&segs, 1e-6) {
                            g.hatches.push(super::cad_layers::HatchRegion { loops, spacing: vt.hatch_spacing.max(0.3), lines: hatch_l0..g.lines.len() });
                        }
                    }
                    Some(regions) => {
                        let ring = |l: &[[f64; 2]]| (0..l.len()).map(|i| [ub.to_sheet(l[i]), ub.to_sheet(l[(i + 1) % l.len()])]).collect::<Vec<_>>();
                        let mut plain: Vec<[[f64; 2]; 2]> = Vec::new();
                        for ((outer, holes), style) in regions.iter().zip(views::section_region_styles(&regions, &v.section_hatches)) {
                            let Some(h) = style else {
                                plain.extend(ring(outer));
                                plain.extend(holes.iter().flat_map(|x| ring(x)));
                                continue;
                            };
                            let Some(pattern) = h.pattern else { continue };
                            let mark = g.mark();
                            let o: Vec<[f64; 2]> = outer.iter().map(|q| ub.to_sheet(*q)).collect();
                            let hs: Vec<Vec<[f64; 2]>> = holes.iter().map(|x| x.iter().map(|q| ub.to_sheet(*q)).collect()).collect();
                            for l in super::sheet_sketch::hatch_pattern_holes(&o, &hs, h.standard, pattern, h.scale, h.angle) {
                                push_hatch_line(&mut g, &pl, geo, l, view_pick);
                            }
                            // Solid: filled (the whole region only; not cut by
                            // crops, clips or breaks).
                            if whole && hs.is_empty() && super::hatch_pat::pattern(h.standard, pattern).is_some_and(|p| p.families.is_empty()) {
                                for i in 1..o.len().saturating_sub(1) {
                                    g.fills.push(super::sheet::Fill2 { points: vec![o[0], o[i], o[i + 1]] });
                                }
                            }
                            if let Some(c) = h.color {
                                g.color_since(mark, c);
                            }
                        }
                        for l in hatch(&plain, vt.hatch_spacing.max(0.3)) {
                            push_hatch_line(&mut g, &pl, geo, l, view_pick);
                        }
                    }
                }
            }
            // Show offset cut lines (section views).
            if v.show_offset_cut_lines
                && matches!(v.kind, ViewKind::Section { .. })
                && let (Some(cut), Some(frame), Some(pf)) = (views::section_cut(sheet, v.id), geo.frame.as_ref(), sheet.parent_of(v.id).and_then(|p| views.get(&p)).and_then(|p| p.frame.as_ref()))
            {
                let ub = pl.unbroken();
                for l in views::offset_cut_lines(&cut, pf, frame, &geo.section_segments) {
                    push_hatch_line(&mut g, &pl, geo, [ub.to_sheet(l[0]), ub.to_sheet(l[1])], view_pick);
                }
            }
            // Drawing properties › Views › View and region hatches' pen and
            // colour, ahead of Edit hatch section's own.
            let hatch_lines = cad_mark.0..g.lines.len();
            if !hatch_lines.is_empty() {
                if let Some(rgb) = vt.hatch_line.color {
                    g.colors.insert(hatch_colors0, super::sheet::ColorSpan { lines: hatch_lines.clone(), texts: 0..0, fills: 0..0, rgb });
                }
                if (vt.hatch_line.thickness - LineStyle::Thin.width_mm()).abs() > 1e-9 {
                    g.pens.insert(hatch_pens0, super::sheet::PenSpan { centered: false, lines: hatch_lines, width_mm: vt.hatch_line.thickness, dashes: Vec::new() });
                }
            }
            g.cad_since(cad_mark, CadLayer::ViewHatches);
            // Broken-out profiles (live: a SPLINE on SECTION_JOG_LINES over
            // the cut's edge; a polygon profile as a polyline).
            for (index, b) in v.broken_out.iter().enumerate() {
                let pick = picks.id(PickTarget::BrokenOut { view: v.id, index });
                let outline = views::broken_out_outline(b);
                if outline.len() < 3 {
                    continue;
                }
                let mut closed = outline.clone();
                closed.push(outline[0]);
                let cad_mark = g.mark();
                let pieces = map_polyline(&pl, &closed);
                let whole = pieces.len() == 1;
                for mut piece in pieces {
                    if whole {
                        piece.pop();
                    }
                    g.lines.push(Polyline2 { points: piece, style: LineStyle::Visible, closed: whole, pick });
                }
                if b.spline && pl.breaks().is_empty() && g.lines.len() == cad_mark.0 + 1 {
                    let poles: Vec<[f64; 2]> = super::cad_objects::catmull_rom_poles(&b.profile).into_iter().map(|p| pl.to_sheet(p)).collect();
                    g.add_cad_spline(cad_mark.0, poles);
                }
                g.cad_since(cad_mark, CadLayer::SectionJogLines);
            }
            // Detail boundary ("Show detail view profile"; none with "Clip
            // border to geometry").
            // Detail profile lines: Phantom / Dashed / Solid.
            let detail_line = match vt.detail_line {
                0 => LineStyle::Phantom,
                1 => LineStyle::Hidden,
                _ => LineStyle::Thin,
            };
            let cad_mark = g.mark();
            if !v.detail_hide_profile && !v.detail_clip_border {
                if let Some(poly) = &geo.clip_poly {
                    g.lines.push(Polyline2 { points: poly.iter().map(|p| pl.to_sheet(*p)).collect(), style: detail_line, closed: true, pick: view_pick });
                } else if let Some((c, r)) = geo.clip {
                    g.lines.push(Polyline2 { points: circle_points(pl.to_sheet(c), pl.len_to_sheet(r), 96), style: detail_line, closed: true, pick: view_pick });
                }
            }
            let border = cad_mark.0..g.lines.len();
            pen_lines(&mut g, border, &vt.detail_pen);
            color_lines_since(&mut g, cad_mark, vt.detail_pen.color);
            g.cad_since(cad_mark, CadLayer::DetailViewBorder);
            // Automatic centermarks on face-on circles and centerlines on
            // cylinders seen from the side.
            let auto_lines = v.auto_centerlines.unwrap_or(v.show_centerlines);
            if v.show_centerlines || auto_lines {
                let (ux, uy) = (pl.dir_to_sheet([1.0, 0.0]), pl.dir_to_sheet([0.0, 1.0]));
                let in_band = |p: [f64; 2]| pl.breaks().iter().any(|b| p[b.axis as usize] > b.a && p[b.axis as usize] < b.b);
                let mark = g.mark();
                for (c, r, _) in geo.full_circles().into_iter().filter(|_| v.show_centerlines) {
                    // Deleted one by one (select + Delete).
                    let key = centermark_key(c);
                    if in_band(c) || v.deleted_centermarks.contains(&key) {
                        continue;
                    }
                    let view_pick = picks.id(PickTarget::AutoCentermark { view: v.id, center: key });
                    // Construction geometry › Centermarks (live asb 10-22 b): Onshape adds
                    // 0.045 in to Mark size (a) and Gap size (g); a continuous cross ±a,
                    // then ☑ Extended lines: four center lines from a + g to r + a + g.
                    const HIDDEN: f64 = 0.045 * 25.4;
                    let (c, r) = (pl.to_sheet(c), pl.len_to_sheet(r));
                    let (a, gap) = (vt.centermark_mark_size + HIDDEN, vt.centermark_gap_size + HIDDEN);
                    let at = |u: [f64; 2], t: f64| [c[0] + u[0] * t, c[1] + u[1] * t];
                    for u in [ux, uy] {
                        g.lines.push(Polyline2 { points: vec![at(u, -a), at(u, a)], style: LineStyle::Thin, closed: false, pick: view_pick });
                    }
                    if vt.centermark_extended {
                        for u in [ux, uy, [-ux[0], -ux[1]], [-uy[0], -uy[1]]] {
                            g.lines.push(Polyline2 { points: vec![at(u, a + gap), at(u, r + a + gap)], style: LineStyle::Centerline, closed: false, pick: view_pick });
                        }
                    }
                }
                g.cad_since(mark, CadLayer::Centermarks);
                if let Some(c) = vt.centermark_color {
                    g.color_since(mark, c);
                }
                let marks = mark.0..g.lines.len();
                pen_lines(&mut g, marks, &super::PenProps::new(vt.centermark_thickness));
                let mark = g.mark();
                for a in geo.axes.iter().filter(|_| auto_lines) {
                    for piece in map_polyline(&pl, &[a[0], a[1]]) {
                        let (p, q) = (piece[0], piece[piece.len() - 1]);
                        let d = [q[0] - p[0], q[1] - p[1]];
                        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                        if l < 1e-6 {
                            continue;
                        }
                        let u = [d[0] / l * 2.5, d[1] / l * 2.5];
                        g.lines.push(Polyline2 { points: vec![[p[0] - u[0], p[1] - u[1]], [q[0] + u[0], q[1] + u[1]]], style: LineStyle::Centerline, closed: false, pick: view_pick });
                    }
                }
                if let Some(c) = vt.centerline_color {
                    g.color_since(mark, c);
                }
                if g.lines.len() > mark.0 {
                    let dashes = LineStyle::Centerline.dashes().iter().map(|d| d * vt.centerline_dashes.max(0.01)).collect();
                    g.pens.push(super::sheet::PenSpan { centered: false, lines: mark.0..g.lines.len(), width_mm: vt.centerline_thickness, dashes });
                }
            }
            // Center of mass indicator: a circle with two opposite quarters filled.
            if let Some(com) = geo.com {
                let c = pl.to_sheet(com);
                let r = drawing.props.com_diameter / 2.0;
                g.lines.push(Polyline2 { points: circle_points(c, r, 48), style: LineStyle::Thin, closed: true, pick: view_pick });
                for q in [0.0, std::f64::consts::PI] {
                    let mut pts = vec![c];
                    pts.extend((0..=8).map(|i| {
                        let t = q + i as f64 / 8.0 * std::f64::consts::FRAC_PI_2;
                        [c[0] + r * t.cos(), c[1] + r * t.sin()]
                    }));
                    g.fills.push(Fill2 { points: pts });
                }
                g.lines.push(Polyline2 { points: vec![[c[0] - r, c[1]], [c[0] + r, c[1]]], style: LineStyle::Thin, closed: false, pick: view_pick });
                g.lines.push(Polyline2 { points: vec![[c[0], c[1] - r], [c[0], c[1] + r]], style: LineStyle::Thin, closed: false, pick: view_pick });
            }
            // Flat pattern views: each bend region's two tangent edges (solid)
            // with its bend line between them (Views › Flat pattern views: up /
            // down line style and color), the bend note along it on its left
            // ("UP 90.0° R.091"), and form centermarks.
            let vt = &drawing.props.view_tab;
            if !v.hide_bend_lines {
                for b in &geo.bends {
                    let d = [b.b[0] - b.a[0], b.b[1] - b.a[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
                    let n = [-d[1] / l * b.width / 2.0, d[0] / l * b.width / 2.0];
                    let mark = g.mark();
                    for k in [-1.0, 1.0] {
                        let side = [[b.a[0] + n[0] * k, b.a[1] + n[1] * k], [b.b[0] + n[0] * k, b.b[1] + n[1] * k]];
                        for piece in map_polyline(&pl, &side) {
                            g.lines.push(Polyline2 { points: piece, style: LineStyle::Thin, closed: false, pick: view_pick });
                        }
                    }
                    g.cad_since(mark, CadLayer::BendTangentLines);
                    if let Some(c) = vt.bend_tangent_color {
                        g.color_since(mark, c);
                    }
                    // Flat pattern views › Bend tangent edges' pen.
                    let tangent = mark.0..g.lines.len();
                    pen_lines(&mut g, tangent, &super::PenProps::new(vt.bend_tangent_thickness));
                    let (kind, color) = if b.up { (vt.bend_up_line, vt.bend_up_color) } else { (vt.bend_down_line, vt.bend_down_color) };
                    let style = match kind {
                        0 => LineStyle::Thin,
                        1 => LineStyle::Hidden,
                        _ => LineStyle::Centerline,
                    };
                    let mark = g.mark();
                    for piece in map_polyline(&pl, &[b.a, b.b]) {
                        g.lines.push(Polyline2 { points: piece, style, closed: false, pick: view_pick });
                    }
                    g.cad_since(mark, if b.up { CadLayer::BendLinesUp } else { CadLayer::BendLinesDown });
                    if let Some(c) = color {
                        g.color_since(mark, c);
                    }
                    // Flat pattern views › All bend lines' pen.
                    let bend = mark.0..g.lines.len();
                    pen_lines(&mut g, bend, &super::PenProps::new(vt.bend_lines_thickness));
                }
            }
            // Forms (live "dx (2026-10-27, P3) flat pattern line styles"): an
            // up form always, in Flat pattern views › Form up's style (Center
            // by default); a down form only with hidden lines shown, in Form
            // down's. Counterbore / countersink heads the same way, in
            // Counterbore/Countersink up / down's.
            let forms = geo.flat_forms.iter().map(|(pts, up)| (pts, *up, if *up { 3 } else { 4 }));
            let heads = geo.flat_heads.iter().map(|(pts, up)| (pts, *up, if *up { 0 } else { 1 }));
            for (pts, up, row) in forms.chain(heads) {
                if !up && v.style != RenderStyle::HiddenLinesVisible {
                    continue;
                }
                let (kind, color) = vt.flat_lines.get(row).copied().unwrap_or((2, None));
                let style = match kind {
                    0 => LineStyle::Visible,
                    1 => LineStyle::Hidden,
                    _ => LineStyle::Centerline,
                };
                let mark = g.mark();
                for piece in map_polyline(&pl, pts) {
                    g.lines.push(Polyline2 { points: piece, style, closed: false, pick: view_pick });
                }
                if let Some(c) = color {
                    g.color_since(mark, c);
                }
            }
            if !v.hide_bend_notes && !geo.bends.is_empty() {
                let ts = drawing.props.ann_type(super::sheet::AnnType::BendNotes);
                let th = ts.height.unwrap_or(drawing.props.ann_text_height);
                let mark = g.mark();
                for b in &geo.bends {
                    let (pa, pb) = (pl.to_sheet(b.a), pl.to_sheet(b.b));
                    let mut ang = (pb[1] - pa[1]).atan2(pb[0] - pa[0]);
                    if ang > std::f64::consts::FRAC_PI_2 + 1e-9 {
                        ang -= std::f64::consts::PI;
                    } else if ang <= -std::f64::consts::FRAC_PI_2 + 1e-9 {
                        ang += std::f64::consts::PI;
                    }
                    let f = super::DimFormat::default();
                    let text = format!("{} {} R{}", if b.up { "UP" } else { "DOWN" }, format::dim_text(drawing, &f, b.angle, true, "").main, format::format_len(drawing, &f, b.radius));
                    let mid = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
                    // Beside the bend region, on its left.
                    let off = b.width / 2.0 * pl.scale * 1000.0 + th * 0.4;
                    g.texts.push(Text2 { pos: [mid[0] - ang.sin() * off, mid[1] + ang.cos() * off], height: th, text, angle: ang, anchor: Anchor::BottomCenter, pick: view_pick });
                }
                if let Some(c) = ts.color.or(drawing.props.ann_color) {
                    g.color_since(mark, c);
                }
            }
            // Cosmetic threads at the major diameter, thin (live 10-26 d, sheet
            // y up): ANSI four 60° arcs, each ending on an axis with 30° gaps
            // after it (30°–90°, 120°–180°, 210°–270°, 300°–360°; measured on
            // drawing/ui2-ct-end-ANSI-2x); ISO and JIS one arc of about three
            // quarters (ISO's gap from 188.5° to 270.5°, JIS's from 7.5° to
            // 79.5°).
            let thread_mark = g.mark();
            if !geo.threads.is_empty() {
                // Drawing properties › Views › Cosmetic threads › Thread
                // style, else the sheet's standard.
                let style = vt.thread_style.unwrap_or(if sheet.size.is_ansi() { super::ThreadStyle::Ansi } else { super::ThreadStyle::Iso });
                let arcs: &[(f64, f64)] = match style {
                    super::ThreadStyle::Ansi => &[(30.0, 90.0), (120.0, 180.0), (210.0, 270.0), (300.0, 360.0)],
                    super::ThreadStyle::Iso => &[(-89.5, 188.5)],
                    super::ThreadStyle::Jis => &[(79.5, 367.5)],
                };
                for (c, r) in &geo.threads {
                    for &(a0, a1) in arcs {
                        let (a0, a1) = (a0.to_radians(), a1.to_radians());
                        let n = ((a1 - a0) / std::f64::consts::TAU * 48.0).ceil().max(4.0) as usize;
                        let pts: Vec<[f64; 2]> = (0..=n).map(|i| a0 + (a1 - a0) * i as f64 / n as f64).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect();
                        for piece in map_polyline(&pl, &pts) {
                            g.lines.push(Polyline2 { points: piece, style: LineStyle::Thin, closed: false, pick: view_pick });
                        }
                    }
                }
            }
            if g.lines.len() > thread_mark.0 {
                let lines = thread_mark.0..g.lines.len();
                let st = g.lines[lines.start].style;
                g.pens.push(super::sheet::PenSpan { centered: false, lines, width_mm: vt.thread_line.thickness, dashes: st.dashes().iter().map(|d| d * vt.thread_dashes.max(0.01)).collect() });
            }
            color_lines_since(&mut g, thread_mark, vt.thread_line.color);
            // Flat pattern views › Form centermarks: style and colour.
            let form_mark = g.mark();
            let form_style = match vt.form_centermark_line {
                0 => LineStyle::Thin,
                1 => LineStyle::Hidden,
                _ => LineStyle::Centerline,
            };
            for m in &geo.form_marks {
                let c = pl.to_sheet(*m);
                let arm = 1.5;
                g.lines.push(Polyline2 { points: vec![[c[0] - arm, c[1]], [c[0] + arm, c[1]]], style: form_style, closed: false, pick: view_pick });
                g.lines.push(Polyline2 { points: vec![[c[0], c[1] - arm], [c[0], c[1] + arm]], style: form_style, closed: false, pick: view_pick });
            }
            color_lines_since(&mut g, form_mark, vt.form_centermark_color);
            // Break lines.
            let cad_mark = g.mark();
            let break_mark = g.mark();
            for br in pl.breaks() {
                let pick = picks.id(PickTarget::BreakLine { view: v.id, index: br.index as usize });
                let style = v.breaks.get(br.index as usize).map(|b| b.style).unwrap_or_default();
                let other = 1 - br.axis as usize;
                let margin = 3.0 / (pl.scale * 1000.0);
                let (o0, o1) = (geo.min[other] - margin, geo.max[other] + margin);
                let nrm = pl.dir_to_sheet(if br.axis == 0 { [1.0, 0.0] } else { [0.0, 1.0] });
                for at in [br.a, br.b] {
                    let pt = |o: f64| {
                        let mut q = [0.0; 2];
                        q[br.axis as usize] = at;
                        q[other] = o;
                        pl.to_sheet(q)
                    };
                    let (s0, s1) = (pt(o0), pt(o1));
                    let pts: Vec<[f64; 2]> = match style {
                        BreakStyle::Straight => vec![s0, s1],
                        BreakStyle::ZigZag => {
                            let m = [(s0[0] + s1[0]) / 2.0, (s0[1] + s1[1]) / 2.0];
                            let d = [s1[0] - s0[0], s1[1] - s0[1]];
                            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                            let u = [d[0] / l, d[1] / l];
                            let at = |t: f64, o: f64| [m[0] + u[0] * t + nrm[0] * o, m[1] + u[1] * t + nrm[1] * o];
                            vec![s0, at(-3.0, 0.0), at(-1.5, 2.0), at(1.5, -2.0), at(3.0, 0.0), s1]
                        }
                        BreakStyle::SmallZigZag => {
                            // Small teeth along the whole line.
                            let d = [s1[0] - s0[0], s1[1] - s0[1]];
                            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                            let n = ((l / 2.0).round() as usize).clamp(2, 400);
                            (0..=n)
                                .map(|i| {
                                    let t = i as f64 / n as f64;
                                    let o = if i == 0 || i == n { 0.0 } else if i % 2 == 1 { 0.6 } else { -0.6 };
                                    [s0[0] + d[0] * t + nrm[0] * o, s0[1] + d[1] * t + nrm[1] * o]
                                })
                                .collect()
                        }
                        BreakStyle::Curve => {
                            let n = 24;
                            (0..=n)
                                .map(|i| {
                                    let t = i as f64 / n as f64;
                                    let o = (t * std::f64::consts::TAU).sin() * 1.5;
                                    [s0[0] + (s1[0] - s0[0]) * t + nrm[0] * o, s0[1] + (s1[1] - s0[1]) * t + nrm[1] * o]
                                })
                                .collect()
                        }
                    };
                    g.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
                }
            }
            // Drawing properties › Views › Break views › Break lines.
            if g.lines.len() > break_mark.0 && (vt.break_line.thickness - LineStyle::Thin.width_mm()).abs() > 1e-9 {
                let lines = break_mark.0..g.lines.len();
                pen_lines(&mut g, lines, &vt.break_line);
            }
            color_lines_since(&mut g, break_mark, vt.break_line.color);
            g.cad_since(cad_mark, CadLayer::BreakLines);
            if let Some(err) = &geo.error
                && geo.edges.is_empty()
            {
                g.texts.push(Text2 { pos: pl.position, height: 3.0, text: err.clone(), angle: 0.0, anchor: Anchor::Center, pick: view_pick });
                g.rect(pl.min[0], pl.min[1], pl.max[0], pl.max[1], LineStyle::Thin);
                if let Some(l) = g.lines.last_mut() {
                    l.pick = view_pick;
                }
            }
            // Labels below the view, every line at Drawing properties › Views ›
            // View labels' height (live drawing/1030: "DETAIL B" and "SCALE
            // 4:1" alike, 0.12 in).
            let lh = vt.view_label_height.unwrap_or(drawing.props.ann_text_height);
            let mut label_lines: Vec<(String, f64)> = Vec::new();
            match &v.kind {
                ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } => label_lines.push((format!("{} {label} - {label}", vt.section_prefix).trim().to_string(), lh)),
                ViewKind::Detail { label, .. } => {
                    label_lines.push((format!("{} {label}", vt.detail_prefix).trim().to_string(), lh));
                    label_lines.push((format!("SCALE {}", format_scale(pl.scale)), lh));
                }
                ViewKind::Auxiliary { .. } => {
                    if let Some(a) = v.aux_ref.as_ref().filter(|a| !a.letter.is_empty()) {
                        let l = if vt.aux_double_label { format!("{0} - {0}", a.letter) } else { a.letter.clone() };
                        label_lines.push((format!("{} {l} {}", vt.aux_prefix, vt.aux_suffix).trim().to_string(), lh));
                    }
                }
                _ => {}
            }
            let auto_label = matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. });
            let user_label = v.label.as_ref().filter(|l| !l.is_empty() && !auto_label);
            if let Some(l) = user_label {
                label_lines.push((expand(l), lh));
            }
            // Scale label: View properties › Scale label, else with a view label
            // when the view has its own scale.
            if !matches!(v.kind, ViewKind::Detail { .. }) && v.scale_label.unwrap_or(user_label.is_some() && v.scale.is_some()) {
                label_lines.push((format!("SCALE {}", format_scale(pl.scale)), lh));
            }
            // Live (behaviours "view labels", asb 10-20 u): the label is its
            // own selectable item, moved freely by `label_at`.
            let label_pick = picks.id(PickTarget::ViewLabel(v.id));
            let at = v.label_at.unwrap_or_default();
            let mut y = pl.min[1] - 6.0;
            let label_mark = g.texts.len();
            for (text, height) in label_lines {
                y -= height;
                g.texts.push(Text2 { pos: [(pl.min[0] + pl.max[0]) / 2.0 + at[0], y + at[1]], height, text, angle: 0.0, anchor: Anchor::BottomCenter, pick: label_pick });
                y -= 2.0;
            }
            g.cad_text_style_since(label_mark, CadTextStyle::ViewLabel);
            if let Some(font) = vt.view_label_font.clone().filter(|_| g.texts.len() > label_mark) {
                g.fonts.push(super::sheet::FontSpan { texts: label_mark..g.texts.len(), font });
            }
            if let Some(rgb) = vt.view_label_color {
                g.color_since((g.lines.len(), label_mark, g.fills.len()), rgb);
            }
        }
        // A linked view's parent is on another sheet: its indicator is drawn there.
        if sheet.parent_of(v.id).is_some_and(|p| p.is_ghost()) {
            continue;
        }
        // Cutting line / detail circle in the parent.
        let cad_mark = g.mark();
        match &v.kind {
            ViewKind::Section { parent, label, .. } | ViewKind::AlignedSection { parent, label, .. } => {
                if let (Some(ppl), Some(cut)) = (place(*parent), views::section_cut(sheet, v.id)) {
                    let pick = picks.id(PickTarget::SectionLine(v.id));
                    let mut pts: Vec<[f64; 2]> = cut.points.iter().map(|p| ppl.to_sheet(*p)).collect();
                    // Both end legs run 0.25 in past the view's geometry (live
                    // qa4-r5-s11.dxf, drw2-k: exactly 6.35 mm / 0.25 in).
                    let n = pts.len();
                    for (e, i) in [(0, 1), (n - 1, n - 2)] {
                        let d = [pts[e][0] - pts[i][0], pts[e][1] - pts[i][1]];
                        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                        if l < 1e-9 {
                            continue;
                        }
                        let u = [d[0] / l, d[1] / l];
                        // Where the leg's line leaves the view's bounds.
                        let exit = (0..2)
                            .filter(|&k| u[k].abs() > 1e-12)
                            .map(|k| ((if u[k] > 0.0 { ppl.max[k] } else { ppl.min[k] }) - pts[i][k]) / u[k])
                            .fold(f64::INFINITY, f64::min);
                        if exit.is_finite() && exit > 0.0 {
                            let t = exit + CUT_OVERSHOOT;
                            pts[e] = [pts[i][0] + u[0] * t, pts[i][1] + u[1] * t];
                        }
                    }
                    // Drawing properties › Views › Section views: the cutting
                    // line's pen and colour, its arrowheads and label.
                    let vtab = &drawing.props.view_tab;
                    let cut_line = g.lines.len();
                    g.lines.push(Polyline2 { points: pts.clone(), style: LineStyle::Section, closed: false, pick });
                    pen_lines(&mut g, cut_line..cut_line + 1, &vtab.section_line);
                    // Thick strokes at the jogs.
                    for k in 1..pts.len().saturating_sub(1) {
                        let c = pts[k];
                        for nb in [pts[k - 1], pts[k + 1]] {
                            let d = [nb[0] - c[0], nb[1] - c[1]];
                            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                            let e = [c[0] + d[0] / l * 4.0f64.min(l), c[1] + d[1] / l * 4.0f64.min(l)];
                            g.lines.push(Polyline2 { points: vec![c, e], style: LineStyle::Border, closed: false, pick });
                        }
                    }
                    let sight0 = ppl.dir_to_sheet([-cut.toward_view[0], -cut.toward_view[1]]);
                    let leg0 = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]];
                    let leg_n = [pts[n - 1][0] - pts[n - 2][0], pts[n - 1][1] - pts[n - 2][1]];
                    let turn = leg_n[1].atan2(leg_n[0]) - leg0[1].atan2(leg0[0]);
                    // Each end (live qa4-r5-s11.dxf, drw2-k): from the line a
                    // solid shaft a twelfth of an arrow wide and an arrow long
                    // along the line of sight, the arrow beyond it (its tip
                    // two arrows out), the letter past the tip.
                    let a = vtab.section_arrow.size.unwrap_or(drawing.props.ann_arrow);
                    let h = vtab.section_label.height.unwrap_or(drawing.props.ann_text_height);
                    let t0 = g.texts.len();
                    for (end, sight) in [(pts[0], sight0), (pts[n - 1], rotate2(sight0, turn))] {
                        let neck = [end[0] + sight[0] * a, end[1] + sight[1] * a];
                        let tip = [end[0] + sight[0] * 2.0 * a, end[1] + sight[1] * 2.0 * a];
                        let w = [-sight[1] * a / 24.0, sight[0] * a / 24.0];
                        g.fills.push(Fill2 { points: vec![[end[0] - w[0], end[1] - w[1]], [neck[0] - w[0], neck[1] - w[1]], [neck[0] + w[0], neck[1] + w[1]], [end[0] + w[0], end[1] + w[1]]] });
                        // Under the fill, for picking.
                        g.lines.push(Polyline2 { points: vec![end, neck], style: LineStyle::Thin, closed: false, pick });
                        g.arrow_as(tip, sight, a, vtab.section_arrow.kind);
                        let at = [tip[0] + sight[0] * 1.1 * h, tip[1] + sight[1] * 1.1 * h];
                        g.texts.push(Text2 { pos: at, height: h, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    }
                    color_lines_since(&mut g, cad_mark, vtab.section_line.color);
                    label_since(&mut g, t0, &vtab.section_label);
                }
            }
            // Auxiliary view reference indicator: a viewing-plane line beside
            // the edge with arrows along the line of sight and the letter.
            ViewKind::Auxiliary { parent, .. } => {
                if let (Some(ppl), Some(a)) = (place(*parent), v.aux_ref.as_ref().filter(|a| a.show && drawing.props.view_tab.aux_indicator))
                    && let Some(ind) = aux_indicator(sheet, v.id, &ppl, views.get(parent).map(|g| g.as_ref()))
                {
                    let pick = picks.id(PickTarget::AuxIndicator(v.id));
                    let sight = [-ind.ns[0], -ind.ns[1]];
                    // Drawing properties › Views › Auxiliary views: viewing
                    // lines, arrowheads, viewing line label.
                    let vtab = &drawing.props.view_tab;
                    let head = vtab.aux_arrow.size.unwrap_or(g.style.ann_arrow);
                    let letter_h = vtab.aux_label.height.unwrap_or(AUX_LETTER);
                    let t0 = g.texts.len();
                    // Arrow style (live u-933): one arrow from the line's middle
                    // toward the edge and the letter beside it; no line.
                    if drawing.props.view_tab.aux_arrow_format {
                        let tail = ind.mid;
                        let neck = [tail[0] + sight[0] * AUX_SHAFT, tail[1] + sight[1] * AUX_SHAFT];
                        g.lines.push(Polyline2 { points: vec![tail, neck], style: LineStyle::Thin, closed: false, pick });
                        g.arrow_as([neck[0] + sight[0] * AUX_HEAD, neck[1] + sight[1] * AUX_HEAD], sight, head, vtab.aux_arrow.kind);
                        g.texts.push(Text2 { pos: aux_arrow_letter(&ind, a), height: letter_h, text: a.letter.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    } else {
                        let line = g.lines.len();
                        g.lines.push(Polyline2 { points: ind.elbows.to_vec(), style: LineStyle::Phantom, closed: false, pick });
                        pen_lines(&mut g, line..line + 1, &vtab.aux_line);
                        for ((elbow, tip), letter) in ind.elbows.into_iter().zip(ind.tips).zip(ind.letters) {
                            let neck = [elbow[0] + sight[0] * AUX_SHAFT, elbow[1] + sight[1] * AUX_SHAFT];
                            g.lines.push(Polyline2 { points: vec![elbow, neck], style: LineStyle::Thin, closed: false, pick });
                            g.arrow_as(tip, sight, head, vtab.aux_arrow.kind);
                            g.texts.push(Text2 { pos: letter, height: letter_h, text: a.letter.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                        }
                    }
                    color_lines_since(&mut g, cad_mark, vtab.aux_line.color);
                    label_since(&mut g, t0, &vtab.aux_label);
                }
            }
            ViewKind::Detail { parent, center, radius, label } => {
                if let Some(ppl) = place(*parent) {
                    let pick = picks.id(PickTarget::DetailCircle(v.id));
                    let c = ppl.to_sheet(*center);
                    let r = ppl.len_to_sheet(*radius);
                    // Drawing properties › Views › Detail views: the profile's
                    // pen and colour, arrowheads, label.
                    let vtab = &drawing.props.view_tab;
                    let h = vtab.detail_label.height.unwrap_or(drawing.props.ann_text_height);
                    let a = vtab.detail_arrow.size.unwrap_or(drawing.props.ann_arrow);
                    let t0 = g.texts.len();
                    // The profile in the source view.
                    let outline: Vec<[f64; 2]> = match v.detail_profile.as_ref().filter(|p| p.len() >= 3) {
                        Some(p) => p.iter().map(|q| ppl.to_sheet(*q)).collect(),
                        None => circle_points(c, r, 96),
                    };
                    let top_right = outline.iter().copied().fold([f64::MIN, f64::MIN], |m, p| if p[0] + p[1] > m[0] + m[1] { p } else { m });
                    // In line on a circle (live qa4 r5 s11 2n, qa4-r5-s11.dxf):
                    // the label on the circle at 0°, the circle broken
                    // around it, an arrow each side along the circle
                    // pointing at it, tips 0.75 of the text height away.
                    let in_line_circle = v.detail_profile.is_none() && v.detail_style != 1 && v.detail_style != 2 && v.detail_label_at.is_none();
                    if in_line_circle && r > 1e-6 {
                        let half = |chord: f64| 2.0 * (chord / (2.0 * r)).min(1.0).asin();
                        let tip_a = half(0.75 * h);
                        let base_a = (tip_a + half(a)).min(std::f64::consts::PI * 0.95);
                        let on = |t: f64| [c[0] + r * t.cos(), c[1] + r * t.sin()];
                        let n = 96;
                        let arc: Vec<[f64; 2]> = (0..=n).map(|i| on(base_a + (std::f64::consts::TAU - 2.0 * base_a) * i as f64 / n as f64)).collect();
                        let line = g.lines.len();
                        g.lines.push(Polyline2 { points: arc, style: LineStyle::Thin, closed: false, pick });
                        pen_lines(&mut g, line..line + 1, &vtab.detail_pen);
                        for sign in [1.0, -1.0] {
                            let (tip, base) = (on(sign * tip_a), on(sign * base_a));
                            g.arrow_as(tip, [tip[0] - base[0], tip[1] - base[1]], a, vtab.detail_arrow.kind);
                        }
                        g.texts.push(Text2 { pos: on(0.0), height: h, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    } else {
                        let line = g.lines.len();
                        g.lines.push(Polyline2 { points: outline, style: LineStyle::Thin, closed: true, pick });
                        pen_lines(&mut g, line..line + 1, &vtab.detail_pen);
                    }
                    match (v.detail_style, v.detail_label_at) {
                        // Connected: a dashed line from the profile to the detail view, no label.
                        (1, _) => {
                            let d = [pl.position[0] - c[0], pl.position[1] - c[1]];
                            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                            let from = [c[0] + d[0] / l * r, c[1] + d[1] / l * r];
                            let to = [pl.position[0] - d[0] / l * (pl.max[0] - pl.min[0]) * 0.5, pl.position[1] - d[1] / l * (pl.max[1] - pl.min[1]) * 0.5];
                            g.lines.push(Polyline2 { points: vec![from, to], style: LineStyle::Hidden, closed: false, pick });
                        }
                        // Leader (or Add leader): the label away from the profile, an arrow to it.
                        (2, at) | (_, at @ Some(_)) => {
                            let at = at.map(|o| [c[0] + o[0], c[1] + o[1]]).unwrap_or([top_right[0] + 12.0, top_right[1] + 10.0]);
                            let d = [c[0] - at[0], c[1] - at[1]];
                            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                            let tip = [c[0] - d[0] / l * r, c[1] - d[1] / l * r];
                            g.lines.push(Polyline2 { points: vec![at, tip], style: LineStyle::Thin, closed: false, pick });
                            g.arrow_as(tip, [d[0] / l, d[1] / l], a * 3.5 / 3.0, vtab.detail_arrow.kind);
                            g.texts.push(Text2 { pos: [at[0] - d[0] / l * 3.5, at[1] - d[1] / l * 3.5], height: h, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                        }
                        // In line on another profile: the label beside it.
                        _ if !in_line_circle => {
                            g.texts.push(Text2 { pos: [top_right[0] + 3.0, top_right[1] + 3.0], height: h, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                        }
                        _ => {}
                    }
                    color_lines_since(&mut g, cad_mark, vtab.detail_pen.color);
                    label_since(&mut g, t0, &vtab.detail_label);
                }
            }
            _ => {}
        }
        g.cad_since(cad_mark, if matches!(v.kind, ViewKind::Detail { .. }) { CadLayer::DetailViewParentBorder } else { CadLayer::SectionCuttingLine });
    }

    // Annotations with resolved references.
    let (mut resolved, dangling) = resolve_annotations(drawing, sheet, views, sources);
    // MBD datums from the Part Studio: anchored on their face in their view.
    let to_sheet_in = |id: ViewId, p: [f64; 2]| placements.iter().find(|pl| pl.id == id).map(|pl| pl.to_sheet(p)).unwrap_or(p);
    super::mbd::resolve_model_annotations(&mut resolved, views, sources, &to_sheet_in);
    let (ml, mt, mg) = (g.lines.len(), g.texts.len(), g.grips.len());
    // Note fields are matched only against annotation texts.
    let used_notes = std::cell::RefCell::new(Vec::<String>::new());
    let env = Env { drawing, sheet, placements: &placements, views, sources, fields: &fields, revisions: &info.revisions, used: &used_notes };
    g.add_annotations(&resolved, &env);
    let mut remap: HashMap<u32, u32> = HashMap::new();
    for a in &resolved.annotations {
        remap.insert((a.id & 0xFFFF_FFFF) as u32, picks.id(PickTarget::Annotation(a.id)));
        if a.inspection.is_some() {
            remap.insert(super::sheet::inspection_pick(a.id), picks.id(PickTarget::Inspection(a.id)));
        }
    }
    // Export layers by annotation kind (`annotation_cad_layers`).
    let kinds: HashMap<u32, (CadLayer, CadLayer)> = resolved.annotations.iter().map(|a| ((a.id & 0xFFFF_FFFF) as u32, annotation_cad_layers(&a.kind))).collect();
    let line_kinds: Vec<Option<CadLayer>> = g.lines[ml..].iter().map(|l| kinds.get(&l.pick).map(|k| k.0)).collect();
    let text_kinds: Vec<Option<CadLayer>> = g.texts[mt..].iter().map(|t| kinds.get(&t.pick).map(|k| k.1)).collect();
    g.cad_runs(ml, &line_kinds, mt, &text_kinds);
    // Notes' texts in Onshape-Standard-NoteStyle.
    let notes: std::collections::HashSet<u32> = resolved.annotations.iter().filter(|a| matches!(a.kind, AnnotationKind::Note { .. })).map(|a| (a.id & 0xFFFF_FFFF) as u32).collect();
    for i in mt..g.texts.len() {
        if notes.contains(&g.texts[i].pick) {
            g.cad_text_styles.push(super::cad_layers::CadTextStyleSpan { texts: i..i + 1, style: CadTextStyle::Note });
        }
    }
    for l in &mut g.lines[ml..] {
        l.pick = remap.get(&l.pick).copied().unwrap_or(0);
    }
    for t in &mut g.texts[mt..] {
        t.pick = remap.get(&t.pick).copied().unwrap_or(0);
    }
    // Annotations moved to a format layer take its color and lock with the
    // format (Formats › Locked).
    let moved: Vec<LayerSpan> = g.layers.iter().filter(|s| s.lines.start >= ml && s.texts.start >= mt).cloned().collect();
    for s in moved {
        if let Some(rgb) = s.layer.color(&drawing.props) {
            g.colors.push(super::sheet::ColorSpan { lines: s.lines.clone(), texts: s.texts.clone(), fills: s.fills.clone(), rgb });
        }
        if drawing.props.formats_locked {
            g.lines[s.lines.clone()].iter_mut().for_each(|l| l.pick = 0);
            g.texts[s.texts.clone()].iter_mut().for_each(|t| t.pick = 0);
        }
    }
    for gr in &mut g.grips[mg..] {
        gr.pick = remap.get(&gr.pick).copied().unwrap_or(0);
    }

    // Inserted images (borders when Drawing properties say so).
    for im in &sheet.images {
        g.images.push(super::sheet::GeomImage { pos: im.pos, size: im.size, data_base64: im.data_base64.clone(), rgb: None });
        if drawing.props.image_borders {
            let pick = picks.id(PickTarget::Image(im.id));
            let (p, s) = (im.pos, im.size);
            let image_mark = g.mark();
            g.lines.push(Polyline2 { points: vec![p, [p[0] + s[0], p[1]], [p[0] + s[0], p[1] + s[1]], [p[0], p[1] + s[1]]], style: LineStyle::Thin, closed: true, pick });
            g.cad_since(image_mark, CadLayer::Images);
        }
    }

    // Drawing sketch geometry and hatch regions.
    {
        let cell = std::cell::RefCell::new(&mut picks);
        let ent = |e: noshape_sketch::EntityId| cell.borrow_mut().id(PickTarget::SketchEntity(e.0));
        let hat = |h: u64| cell.borrow_mut().id(PickTarget::Hatch(h));
        let to_sheet = |v: ViewId, p: [f64; 2]| placements.iter().find(|pl| pl.id == v).map(|pl| pl.to_sheet(p));
        let sketch_mark = g.mark();
        let (pens0, colors0) = (g.pens.len(), g.colors.len());
        sheet.sketch.render(&mut g, &ent, &hat, &to_sheet);
        // Construction geometry › Sketch geometry: pen and colour, ahead of
        // each entity's own.
        let sk = &drawing.props.view_tab.sketch_line;
        let lines = sketch_mark.0..g.lines.len();
        if !lines.is_empty() {
            g.pens.insert(pens0, super::sheet::PenSpan { centered: false, lines: lines.clone(), width_mm: sk.thickness, dashes: Vec::new() });
            if let Some(rgb) = sk.color {
                g.colors.insert(colors0, super::sheet::ColorSpan { lines, texts: 0..0, fills: 0..0, rgb });
            }
        }
        g.cad_since(sketch_mark, CadLayer::SketchedGeometry);
    }

    // Tables (and hole tags in their views).
    let mut table_layouts = Vec::new();
    for t in &sheet.tables {
        let pick = picks.id(PickTarget::Table(t.id));
        let view = t.view();
        let geo = view.and_then(|v| views.get(&v)).map(|g| g.as_ref());
        let source = view.and_then(|v| views::view_source(sheet, v)).and_then(|(k, _)| sources.get(&k));
        let origin = match &t.kind {
            TableKind::Hole { origin, origin_ref, .. } => {
                match (origin_ref, geo, source) {
                    (Some(r), Some(g), Some(src)) => resolve_in_view(&src.context, g, r).map(|(p, _)| p).or(Some(*origin)),
                    _ => Some(*origin),
                }
            }
            _ => None,
        };
        let data = tables::table_data_with(drawing, t, geo, source, origin, info, &expand);
        let layout = tables::layout(t, &data);
        let (l0, t0, f0) = (g.lines.len(), g.texts.len(), g.fills.len());
        tables::render(&mut g, t, &data, &layout, pick);
        g.cad_since((l0, t0, f0), CadLayer::Tables);
        g.add_cad_table((l0, t0, f0), &layout, &data, &t.kind);
        // Moved to a format layer: its color, locked with the format.
        if !t.layer.is_drawing() {
            let span = LayerSpan { lines: l0..g.lines.len(), texts: t0..g.texts.len(), fills: f0..g.fills.len(), layer: t.layer };
            if let Some(rgb) = t.layer.color(&drawing.props) {
                g.colors.push(super::sheet::ColorSpan { lines: span.lines.clone(), texts: span.texts.clone(), fills: span.fills.clone(), rgb });
            }
            if drawing.props.formats_locked {
                g.lines[span.lines.clone()].iter_mut().for_each(|l| l.pick = 0);
                g.texts[span.texts.clone()].iter_mut().for_each(|x| x.pick = 0);
            }
            g.layers.push(span);
        }
        if let (TableKind::Hole { flip_x, flip_y, .. }, Some(geo), Some(pl), Some(o)) = (&t.kind, geo, view.and_then(place), origin) {
            let os = pl.to_sheet(o);
            let origin_mark = g.mark();
            // The tag axis: X and Y arrows in the table's directions.
            let (sx, sy) = (if *flip_x { -1.0 } else { 1.0 }, if *flip_y { -1.0 } else { 1.0 });
            let (xt, yt) = ([os[0] + 8.0 * sx, os[1]], [os[0], os[1] + 8.0 * sy]);
            // Drawing properties › Tables › Hole tables: Indicator lines'
            // pen and arrowhead size.
            let ts = &drawing.props.table_styles;
            let l0 = g.lines.len();
            g.lines.push(Polyline2 { points: vec![os, xt], style: LineStyle::Thin, closed: false, pick });
            g.lines.push(Polyline2 { points: vec![os, yt], style: LineStyle::Thin, closed: false, pick });
            g.pens.push(super::sheet::PenSpan { centered: false, lines: l0..l0 + 2, width_mm: ts.hole_indicator_line, dashes: Vec::new() });
            match ts.hole_indicator_arrow {
                Some(a) => {
                    g.arrow_as(xt, [sx, 0.0], a, super::sheet::ArrowType::Filled);
                    g.arrow_as(yt, [0.0, sy], a, super::sheet::ArrowType::Filled);
                }
                None => {
                    g.arrow(xt, [sx, 0.0], 2.0);
                    g.arrow(yt, [0.0, sy], 2.0);
                }
            }
            color_lines_since(&mut g, origin_mark, ts.hole_indicator_color);
            g.texts.push(Text2 { pos: [xt[0] + 1.5 * sx - if sx < 0.0 { 1.8 } else { 0.0 }, xt[1] - 0.9], height: 1.8, text: "X".into(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
            g.texts.push(Text2 { pos: [yt[0] - 0.6, yt[1] + if sy > 0.0 { 1.0 } else { -2.8 }], height: 1.8, text: "Y".into(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
            g.fills.push(Fill2 { points: circle_points(os, 0.8, 8) });
            g.cad_since(origin_mark, CadLayer::OriginIndicators);
            let tag_mark = g.texts.len();
            let tags = &ts.hole_tags;
            for hrow in tables::hole_rows(geo, o) {
                let c = pl.to_sheet(hrow.center);
                let r = pl.len_to_sheet(hrow.diameter / 2.0);
                g.texts.push(Text2 { pos: [c[0] + r * 0.75 + 0.8, c[1] + r * 0.75 + 0.8], height: tags.height.unwrap_or(drawing.props.ann_text_height), text: hrow.tag, angle: 0.0, anchor: Anchor::BottomLeft, pick });
            }
            g.cad_text_style_since(tag_mark, CadTextStyle::HoleTag);
            // Hole tables › Tags: font, Bold / Italic and colour.
            let tags_font = ts.hole_tags_font.clone().map(|f| super::sheet::TextFont { bold: tags.bold, italic: tags.italic, ..f }).or_else(|| (tags.bold || tags.italic).then(|| super::sheet::TextFont { family: super::sheet::DRAWING_DEFAULT_FONT.into(), bold: tags.bold, italic: tags.italic }));
            if let Some(font) = tags_font.filter(|_| g.texts.len() > tag_mark) {
                g.fonts.push(super::sheet::FontSpan { texts: tag_mark..g.texts.len(), font });
            }
            if let Some(rgb) = tags.color.filter(|_| g.texts.len() > tag_mark) {
                g.colors.push(super::sheet::ColorSpan { lines: 0..0, texts: tag_mark..g.texts.len(), fills: 0..0, rgb });
            }
        }
        table_layouts.push((t.id, layout));
    }
    // Revision tables of other sheets: the full table when Linked, else the
    // "See sheet N" pointer table (not pickable here).
    for (hi, home) in drawing.sheets.iter().enumerate().filter(|(i, _)| *i != si) {
        for t in home.tables.iter().filter(|t| matches!(t.kind, TableKind::Revision { .. })) {
            let data = match t.kind {
                TableKind::Revision { see_sheet1: true, .. } => tables::see_sheet_data(drawing, hi),
                _ => tables::table_data(drawing, t, None, None, None, &info.revisions, &expand),
            };
            // Same offset from the sheet corner of its fixed corner.
            let (hw, hh) = home.size.size_mm();
            let mut t = t.clone();
            if matches!(t.corner, super::TableCorner::TopRight | super::TableCorner::BottomRight) {
                t.position[0] = w - (hw - t.position[0]);
            }
            if matches!(t.corner, super::TableCorner::TopLeft | super::TableCorner::TopRight) {
                t.position[1] = h - (hh - t.position[1]);
            }
            let layout = tables::layout(&t, &data);
            tables::render(&mut g, &t, &data, &layout, 0);
        }
    }

    // Notes › property background: grey behind property fields (screen only).
    if !drawing.props.hide_property_background {
        let (used, used_notes) = (used.into_inner(), used_notes.into_inner());
        // Title block values against title block texts, note fields against
        // annotation texts (not zone labels or datums that read like a value).
        let hit = |list: &[String], t: &Text2| !t.text.trim().is_empty() && list.iter().any(|u| u.trim() == t.text.trim());
        g.field_backdrops = g.texts.iter().enumerate().filter(|(i, t)| (*i >= mark_t && *i < end_t && in_tb(&t.pos) && hit(&used, t)) || (*i >= mt && hit(&used_notes, t))).map(|(_, t)| super::font::text_box(t).to_vec()).collect();
    }
    // Mirrors aren't views of this sheet (not picked, not listed).
    placements.retain(|p| !p.id.is_ghost());
    BuiltSheet { geometry: g, picks: picks.table, placements, resolved, dangling, tables: table_layouts, title_rect, title_cells }
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
