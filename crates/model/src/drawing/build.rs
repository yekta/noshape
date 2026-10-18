//! Assemble a whole sheet: views scaled, rotated, broken and placed,
//! section hatching, cutting lines, detail circles, break lines, view
//! labels, associative annotations, tables, border and title block, into
//! one [`SheetGeometry`] plus a pick table for the editor.

use super::font;
use super::format::{self, DimText};
use super::sheet::{Anchor, AnnotationEnv, Fill2, LineStyle, Polyline2, SheetGeometry, Text2, format_scale};
use super::tables::{self, TableLayout};
use super::views::{self, Sources, ViewCache, ViewGeometry, dist_to_segment, resolve_point, rotate2};
use super::{Annotation, AnnotationKind, BreakStyle, Drawing, RenderStyle, Sheet, TableKind, ViewId, ViewKind};
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
    Table(u64),
    /// Break lines of break `index` of a view.
    BreakLine { view: ViewId, index: usize },
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
}

/// Document-level data for property fields.
#[derive(Clone, Debug, Default)]
pub struct SheetInfo {
    pub doc_name: String,
    pub drawing_name: String,
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
}

impl BuiltSheet {
    pub fn placement(&self, id: ViewId) -> Option<&ViewPlacement> {
        self.placements.iter().find(|p| p.id == id)
    }

    pub fn table_layout(&self, id: u64) -> Option<&TableLayout> {
        self.tables.iter().find(|(t, _)| *t == id).map(|(_, l)| l)
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
        best.map(|(k, _)| k)
    }
}

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
        let frame = views.get(&view).and_then(|g| g.frame);
        let ctx = views::view_source(sheet, view).and_then(|(key, _)| sources.get(&key)).map(|s| s.context.clone());
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

/// Pieces of a view-coordinate polyline outside the view's break bands,
/// mapped to the sheet.
fn map_polyline(pl: &ViewPlacement, pts: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    let mut pieces = vec![pts.to_vec()];
    for br in pl.breaks() {
        pieces = pieces.into_iter().flat_map(|p| views::clip_polyline_band(&p, br.axis as usize, br.a, br.b)).collect();
    }
    pieces.into_iter().map(|p| p.into_iter().map(|q| pl.to_sheet(q)).collect()).collect()
}

/// Lookup of property fields for a sheet.
fn field_value(drawing: &Drawing, si: usize, info: &SheetInfo, sources: &Sources, key: &str) -> Option<String> {
    let sheet = drawing.sheets.get(si)?;
    // Part shown by the first base view of this sheet (else of the drawing).
    let part = || {
        let base = sheet.views.iter().chain(drawing.sheets.iter().flat_map(|s| s.views.iter())).find(|v| matches!(v.kind, ViewKind::Base { .. }))?;
        let (k, part) = match &base.kind {
            ViewKind::Base { source, config, part, .. } => (views::SourceKey { tab: *source, config: config.clone(), explode: base.explode }, part.clone()),
            _ => return None,
        };
        let src = sources.get(&k)?;
        Some((src.name.clone(), src.part(part.as_ref()).cloned(), src.assembly))
    };
    Some(match key {
        "part.name" => part().map(|(n, p, asm)| if asm { n } else { p.map(|p| p.name).unwrap_or_default() }).unwrap_or_default(),
        "part.number" => part().and_then(|(_, p, _)| p).map(|p| p.number).unwrap_or_default(),
        "part.material" => part().and_then(|(_, p, _)| p).map(|p| p.material).unwrap_or_default(),
        "part.description" => part().and_then(|(_, p, _)| p).map(|p| p.description).unwrap_or_default(),
        "model.name" => part().map(|(n, _, _)| n).unwrap_or_default(),
        "doc.name" => info.doc_name.clone(),
        "drawing.name" => info.drawing_name.clone(),
        "sheet.name" => sheet.name.clone(),
        "sheet.number" => (si + 1).to_string(),
        "sheet.count" => drawing.sheets.len().to_string(),
        "sheet.scale" => format_scale(sheet.scale),
        "sheet.size" => format!("{:?}", sheet.size),
        "date" => format::today(),
        "units" => if drawing.units_mm { "mm".into() } else { "in".into() },
        _ => return None,
    })
}

struct Env<'a> {
    drawing: &'a Drawing,
    sheet: &'a Sheet,
    placements: &'a [ViewPlacement],
    sources: &'a Sources,
    fields: &'a dyn Fn(&str) -> Option<String>,
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
    fn text(&self, a: &Annotation, value: f64, angular: bool, symbol: &str) -> DimText {
        let mut f = a.format.clone();
        f.prefix = self.expand(&f.prefix);
        f.suffix = self.expand(&f.suffix);
        format::dim_text(self.drawing, &f, value, angular, symbol)
    }
    fn expand(&self, s: &str) -> String {
        format::expand_fields(s, self.fields)
    }
    fn balloon_text(&self, a: &Annotation, text: &str) -> String {
        let t = text.trim();
        if !(t.is_empty() || t == "#") {
            return self.expand(text);
        }
        let item = (|| {
            let view = a.kind.view()?;
            let (key, _) = views::view_source(self.sheet, view)?;
            let src = self.sources.get(&key)?;
            let body = a.refs.iter().find(|r| r.slot == 0).map(|r| match &r.point {
                super::PointRef::Vertex { body, .. } | super::PointRef::Center { body, .. } => body.clone(),
            })?;
            src.bom_item(&body)
        })();
        item.map(|i| i.to_string()).unwrap_or_else(|| "?".into())
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
    let expand = |s: &str| format::expand_fields(s, &fields);

    // Border and title block; everything inside the title block is pickable
    // as the title block.
    let mark_l = g.lines.len();
    let mark_t = g.texts.len();
    g.add_border_and_title_with(sheet, &expand);
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
        let Some(pl) = placement_of(sheet, v.id, geo) else { continue };
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
            for piece in map_polyline(&pl, &e.points) {
                g.lines.push(Polyline2 { points: piece, style, closed: false, pick });
            }
        }
        // Section hatching: computed on the unbroken view, then cropped and
        // broken in view coordinates.
        if !geo.section_segments.is_empty() {
            let ub = pl.unbroken();
            let segs: Vec<[[f64; 2]; 2]> = geo.section_segments.iter().map(|s| [ub.to_sheet(s[0]), ub.to_sheet(s[1])]).collect();
            for l in hatch(&segs, 2.5) {
                let mut pieces = vec![vec![ub.to_view(l[0]), ub.to_view(l[1])]];
                if let Some((c, r)) = geo.clip {
                    pieces = pieces.into_iter().flat_map(|p| views::clip_polyline_circle(&p, c, r)).collect();
                }
                if let Some(crop) = &geo.crop {
                    pieces = pieces.into_iter().flat_map(|p| views::clip_to_crop(&p, crop)).collect();
                }
                for p in pieces {
                    for q in map_polyline(&pl, &p) {
                        g.lines.push(Polyline2 { points: q, style: LineStyle::Thin, closed: false, pick: view_pick });
                    }
                }
            }
        }
        // Detail boundary.
        if let Some((c, r)) = geo.clip {
            g.lines.push(Polyline2 { points: circle_points(pl.to_sheet(c), pl.len_to_sheet(r), 96), style: LineStyle::Thin, closed: true, pick: view_pick });
        }
        // Automatic centermarks on face-on circles and centerlines on
        // cylinders seen from the side.
        if v.show_centerlines {
            let (ux, uy) = (pl.dir_to_sheet([1.0, 0.0]), pl.dir_to_sheet([0.0, 1.0]));
            let in_band = |p: [f64; 2]| pl.breaks().iter().any(|b| p[b.axis as usize] > b.a && p[b.axis as usize] < b.b);
            for (c, r, _) in geo.circles() {
                if in_band(c) {
                    continue;
                }
                let (c, r) = (pl.to_sheet(c), pl.len_to_sheet(r) * 1.15 + 1.0);
                g.lines.push(Polyline2 { points: vec![[c[0] - ux[0] * r, c[1] - ux[1] * r], [c[0] + ux[0] * r, c[1] + ux[1] * r]], style: LineStyle::Centerline, closed: false, pick: view_pick });
                g.lines.push(Polyline2 { points: vec![[c[0] - uy[0] * r, c[1] - uy[1] * r], [c[0] + uy[0] * r, c[1] + uy[1] * r]], style: LineStyle::Centerline, closed: false, pick: view_pick });
            }
            for a in &geo.axes {
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
        }
        // Break lines.
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
            ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } => label_lines.push((format!("SECTION {label}-{label}"), 5.0)),
            ViewKind::Detail { label, .. } => {
                label_lines.push((format!("DETAIL {label}"), 5.0));
                label_lines.push((format!("SCALE {}", format_scale(pl.scale)), 3.5));
            }
            _ => {}
        }
        if let Some(l) = v.label.as_ref().filter(|l| !l.is_empty())
            && !matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. })
        {
            label_lines.push((expand(l), 5.0));
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
            ViewKind::Section { parent, label, .. } | ViewKind::AlignedSection { parent, label, .. } => {
                if let (Some(ppl), Some(cut)) = (place(*parent), views::section_cut(sheet, v.id)) {
                    let pick = picks.id(PickTarget::SectionLine(v.id));
                    let pts: Vec<[f64; 2]> = cut.points.iter().map(|p| ppl.to_sheet(*p)).collect();
                    g.lines.push(Polyline2 { points: pts.clone(), style: LineStyle::Section, closed: false, pick });
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
                    let n = pts.len();
                    let leg0 = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]];
                    let leg_n = [pts[n - 1][0] - pts[n - 2][0], pts[n - 1][1] - pts[n - 2][1]];
                    let turn = leg_n[1].atan2(leg_n[0]) - leg0[1].atan2(leg0[0]);
                    for (end, inner, sight) in [(pts[0], pts[1], sight0), (pts[n - 1], pts[n - 2], rotate2(sight0, turn))] {
                        let d = [end[0] - inner[0], end[1] - inner[1]];
                        let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                        let u = [d[0] / l, d[1] / l];
                        // Short thick end stroke, then the arrow showing the line of sight.
                        let e2 = [end[0] - u[0] * 6.0, end[1] - u[1] * 6.0];
                        g.lines.push(Polyline2 { points: vec![e2, end], style: LineStyle::Border, closed: false, pick });
                        let base = [end[0] - u[0] * 1.5, end[1] - u[1] * 1.5];
                        let tail = [base[0] - sight[0] * 8.0, base[1] - sight[1] * 8.0];
                        g.lines.push(Polyline2 { points: vec![tail, base], style: LineStyle::Thin, closed: false, pick });
                        g.arrow(base, sight, 3.5);
                        let tp = [tail[0] + u[0] * 3.0 - sight[0] * 1.0, tail[1] + u[1] * 3.0 - sight[1] * 1.0];
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
    let env = Env { drawing, sheet, placements: &placements, sources, fields: &fields };
    g.add_annotations(&resolved, &env);
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

    // Tables (and hole tags in their views).
    let mut table_layouts = Vec::new();
    for t in &sheet.tables {
        let pick = picks.id(PickTarget::Table(t.id));
        let view = t.view();
        let geo = view.and_then(|v| views.get(&v)).map(|g| g.as_ref());
        let source = view.and_then(|v| views::view_source(sheet, v)).and_then(|(k, _)| sources.get(&k));
        let origin = match &t.kind {
            TableKind::Hole { origin, origin_ref, .. } => {
                let frame = geo.and_then(|g| g.frame);
                match (origin_ref, frame, source) {
                    (Some(r), Some(f), Some(src)) => resolve_point(&src.context, &f, r).map(|(p, _)| p).or(Some(*origin)),
                    _ => Some(*origin),
                }
            }
            _ => None,
        };
        let data = tables::table_data(drawing, t, geo, source, origin, &expand);
        let layout = tables::layout(t, &data);
        tables::render(&mut g, t, &data, &layout, pick);
        if let (TableKind::Hole { .. }, Some(geo), Some(pl), Some(o)) = (&t.kind, geo, view.and_then(place), origin) {
            let os = pl.to_sheet(o);
            g.lines.push(Polyline2 { points: vec![[os[0] - 3.0, os[1]], [os[0] + 3.0, os[1]]], style: LineStyle::Thin, closed: false, pick });
            g.lines.push(Polyline2 { points: vec![[os[0], os[1] - 3.0], [os[0], os[1] + 3.0]], style: LineStyle::Thin, closed: false, pick });
            g.fills.push(Fill2 { points: circle_points(os, 0.8, 8) });
            for hrow in tables::hole_rows(geo, o) {
                let c = pl.to_sheet(hrow.center);
                let r = pl.len_to_sheet(hrow.diameter / 2.0);
                g.texts.push(Text2 { pos: [c[0] + r * 0.75 + 0.8, c[1] + r * 0.75 + 0.8], height: 2.5, text: hrow.tag, angle: 0.0, anchor: Anchor::BottomLeft, pick });
            }
        }
        table_layouts.push((t.id, layout));
    }

    BuiltSheet { geometry: g, picks: picks.table, placements, resolved, dangling, tables: table_layouts }
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
