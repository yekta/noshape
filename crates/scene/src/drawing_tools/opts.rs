//! Drawing slice 4: the drawing dialogs' options as plain values, and what
//! they build. The app's dialogs (with their input fields) fill these from
//! what is typed and chosen; the agent API fills them from its arguments.
//! From `crates/app/src/drawing_tools.rs` (pending_view), `drawing_section.rs`,
//! `drawing_detail.rs`, `drawing_insert.rs`, `drawing_hatch.rs`,
//! `drawing_table_tool.rs` and `drawing_gdt.rs`.

use super::geom::*;
use super::{SheetPick, Tool};
use noshape_model::drawing::hatch_pat;
use noshape_model::drawing::sheet_sketch::HatchRegion;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// Insert view panel: what new base views take.
/// View orientation › a named view: its name, frame and section planes.
pub type NamedPick = (String, [[f64; 3]; 2], Vec<[f64; 4]>);

#[derive(Clone, Debug)]
pub struct InsertOpts {
    /// View scale (None: the sheet scale).
    pub scale: Option<f64>,
    pub simplification: Simplification,
    pub flat: bool,
    pub sketch_appearances: bool,
    pub model_annotations: bool,
    /// Assemblies: Explode/Position and Display state (None: Show all).
    pub asm_position: AsmPosition,
    pub display_state: Option<String>,
    /// View orientation › a named view of the source.
    pub named: Option<NamedPick>,
    /// Insert a sketch of the source (a sketch view).
    pub sketch: Option<noshape_model::FeatureId>,
    /// Another document's Part Studio / assembly (Other documents): the
    /// view is pinned to that document's version (its reference key).
    pub pinned: Option<PinnedVersion>,
}

/// The Insert view panel's starting values (as the app's `InsertViewState`).
impl Default for InsertOpts {
    fn default() -> Self {
        Self { scale: None, simplification: Simplification::Auto, flat: false, sketch_appearances: false, model_annotations: true, asm_position: AsmPosition::Default, display_state: None, named: None, sketch: None, pinned: None }
    }
}

/// Auxiliary view dialog.
#[derive(Clone, Debug, Default)]
pub struct AuxOpts {
    pub reverse: bool,
    pub show_ref: bool,
    /// Where the picked edge / line is (parent view coordinates).
    pub anchor: Option<[f64; 2]>,
    /// The view label (reference indicator letter).
    pub letter: String,
}

/// Section view dialog.
#[derive(Clone, Debug, Default)]
pub struct SectionOpts {
    /// "Show cut geometry only".
    pub cut_only: bool,
    pub flip: bool,
    /// "Section depth" is on (its field is shown), its Blind value (m), Up
    /// to entity instead, and the picked point.
    pub depth_on: bool,
    pub depth_value: Option<f64>,
    pub up_to: bool,
    pub depth_ref: Option<PointRef>,
    /// "Exclude from cut" (assembly views).
    pub exclude: Vec<noshape_model::BodyId>,
    /// "Cut from section" (sectioning a section view).
    pub cut_from: bool,
    /// The model point under the cutting line's first click (the line
    /// follows it).
    pub anchor: Option<PointRef>,
}

/// Detail view dialog.
#[derive(Clone, Debug, Default)]
pub struct DetailOpts {
    /// View scale (None: twice the parent's).
    pub scale: Option<f64>,
    /// The finished profile (polygon, parent view coordinates).
    pub profile: Option<Vec<[f64; 2]>>,
    /// 0 In line, 1 Connected, 2 Leader.
    pub style: u8,
    pub show_profile: bool,
    pub clip_border: bool,
    /// The view label (empty: the next free letter).
    pub label: String,
}

/// The view dialogs a new view reads (None: the dialog is closed).
#[derive(Clone, Debug, Default)]
pub struct ViewOptions {
    pub insert: InsertOpts,
    pub aux: Option<AuxOpts>,
    pub section: Option<SectionOpts>,
    pub detail: Option<DetailOpts>,
}

impl SheetPick<'_> {
    /// The view the active view tool would create at the cursor.
    pub fn pending_view(&self, o: &ViewOptions) -> Option<DrawingView> {
        let c = self.cursor;
        let sheet = &self.drawing.sheets.get(self.sheet)?;
        let id = sheet.next_view_id();
        // Base views take Drawing properties › Views › Insert view
        // defaults; derived views copy their parent's.
        let mk = |kind: ViewKind, position: [f64; 2], parent: Option<&DrawingView>| {
            let mut v = DrawingView { id, kind, position, scale: None, label: None, ..Default::default() };
            match parent {
                Some(p) => {
                    // Live: child views don't inherit shading.
                    v.style = if p.style == RenderStyle::Shaded { RenderStyle::HiddenLinesRemoved } else { p.style };
                    v.show_tangent_edges = p.show_tangent_edges;
                    v.tangent_phantom = p.tangent_phantom;
                    v.show_centerlines = p.show_centerlines;
                    v.auto_centerlines = p.auto_centerlines;
                    // Live 3102: child views inherit Show annotations from model.
                    v.model_annotations = p.model_annotations;
                }
                None => self.drawing.props.view_defaults.apply(&mut v),
            }
            v
        };
        if let Some(v) = self.extra_pending_view(id) {
            return Some(v);
        }
        match self.tool {
            Tool::InsertView { source, orientation, part } => {
                let mut v = mk(ViewKind::Base { source: *source, config: Default::default(), orientation: *orientation, part: part.clone() }, c, None);
                // Insert view panel › View scale (None: the sheet scale), View simplification.
                v.scale = o.insert.scale;
                v.simplification = Some(o.insert.simplification);
                v.flat = o.insert.flat;
                v.sketch_appearances = o.insert.sketch_appearances;
                v.model_annotations = o.insert.model_annotations;
                // Assemblies: Explode/Position and Display state.
                v.set_asm_position(o.insert.asm_position);
                v.display_state = o.insert.display_state.clone();
                // All views of a part use the same version: a pinned
                // reference pins the new view too.
                v.pinned = o.insert.pinned.clone().or_else(|| self.drawing.sheets.iter().flat_map(|s| &s.views).find(|o| matches!(&o.kind, ViewKind::Base { source: s2, part: p2, .. } if s2 == source && p2 == part) && o.pinned.is_some()).and_then(|o| o.pinned.clone()));
                // View orientation › a named view: its camera.
                if let Some((name, frame, section)) = &o.insert.named {
                    v.custom_frame = Some(*frame);
                    v.named_view = Some(name.clone());
                    v.named_section.clone_from(section);
                }
                // A sketch view faces its sketch plane.
                if let Some(f) = &o.insert.sketch {
                    v.sketch = Some(*f);
                    v.custom_frame = self.insert_sketch_frame(*source, &o.insert);
                }
                Some(v)
            }
            Tool::Projected { parent: Some(pid) } => {
                let parent = sheet.view(*pid)?;
                let d = sub(c, parent.position);
                if len(d) < 15.0 {
                    return None;
                }
                // Direction in the parent's (unrotated) view frame.
                let rot = views::view_rotation(sheet, *pid);
                let dir = views::snap_direction(views::rotate2(d, -rot));
                let pos = if dir[0] == 0.0 || dir[1] == 0.0 {
                    let axis = views::rotate2(dir, rot);
                    let t = dot(d, axis);
                    [parent.position[0] + axis[0] * t, parent.position[1] + axis[1] * t]
                } else {
                    c
                };
                Some(mk(ViewKind::Projected { parent: *pid, direction: Some(dir) }, pos, Some(parent)))
            }
            Tool::Auxiliary { parent: Some((pid, edge_dir)) } => {
                let parent = sheet.view(*pid)?;
                let n = norm([-edge_dir[1], edge_dir[0]]);
                let t = dot(sub(c, parent.position), n);
                if t.abs() < 15.0 {
                    return None;
                }
                let pos = [parent.position[0] + n[0] * t, parent.position[1] + n[1] * t];
                let mut v = mk(ViewKind::Auxiliary { parent: *pid, edge_dir: *edge_dir }, pos, Some(parent));
                // Auxiliary view dialog: label, Reverse, reference indicator.
                if let Some(d) = &o.aux {
                    v.flip = d.reverse;
                    v.aux_ref = d.anchor.map(|anchor| noshape_model::drawing::AuxRef { anchor, letter: d.letter.clone(), show: d.show_ref, offset: None, ext: None, letter_at: None });
                }
                Some(v)
            }
            Tool::Section { parent: Some(pid), p0, line: Some((a, b)), jogs, .. } => {
                let parent = sheet.view(*pid)?;
                let pl = self.placement(*pid)?;
                let (sa, sb) = (pl.to_sheet(*a), pl.to_sheet(*b));
                let e = norm(sub(sb, sa));
                let n = [-e[1], e[0]];
                let t = dot(sub(c, parent.position), n);
                let pos = [parent.position[0] + n[0] * t, parent.position[1] + n[1] * t];
                let mut v = mk(ViewKind::Section { parent: *pid, line: jogged_line(*a, *b, jogs), label: sheet.next_label(), cuts: Vec::new() }, pos, Some(parent));
                v.style = RenderStyle::HiddenLinesRemoved;
                v.partial = matches!(self.tool, Tool::Section { partial: true, .. });
                // Section view dialog: Show cut geometry only, Section depth.
                self.apply_section_options(&mut v, o);
                v.cut_anchor = o.section.as_ref().and_then(|d| d.anchor.clone()).zip(*p0);
                Some(v)
            }
            Tool::Detail { parent: Some(pid), center: Some(center), radius: Some(r) } => {
                let parent = sheet.view(*pid)?;
                let mut v = mk(ViewKind::Detail { parent: *pid, center: *center, radius: *r, label: sheet.next_label() }, c, Some(parent));
                v.scale = Some(views::view_scale(sheet, *pid) * 2.0);
                // Detail view dialog: profile, scale, label, style, options.
                self.apply_detail_options(&mut v, views::view_scale(sheet, *pid), o);
                Some(v)
            }
            _ => None,
        }
    }

    /// A sketch view's camera: the source's sketch plane.
    pub fn insert_sketch_frame(&self, source: noshape_model::document::TabId, o: &InsertOpts) -> Option<[[f64; 3]; 2]> {
        let f = o.sketch.as_ref()?;
        let key = views::SourceKey { tab: source, config: Default::default(), explode: None, version: None, position: None, display: None };
        views::sketch_frame(&self.sources.get(&key)?.context, f)
    }

    /// Section view dialog: Show cut geometry only, Flip, Section depth,
    /// Exclude from cut, Cut from section.
    pub fn apply_section_options(&self, v: &mut DrawingView, o: &ViewOptions) {
        if let Some(d) = &o.section {
            v.cut_only = d.cut_only;
            v.flip = d.flip;
            v.section_depth = d.depth_value.filter(|_| d.depth_on && !d.up_to);
            v.section_depth_ref = d.depth_ref.clone().filter(|_| d.up_to && d.depth_on);
            v.exclude_from_cut = d.exclude.clone();
            // Live drawing/505: ☑ Cut from section by default, only when
            // sectioning a section view.
            v.cut_from_section = d.cut_from
                && match v.kind {
                    ViewKind::Section { parent, .. } => self.current_sheet().view(parent).is_some_and(|p| matches!(p.kind, ViewKind::Section { .. })),
                    _ => false,
                };
        }
    }

    /// Detail view dialog: scale, profile, style, label and options.
    pub fn apply_detail_options(&self, v: &mut DrawingView, parent_scale: f64, o: &ViewOptions) {
        let Some(d) = &o.detail else { return };
        v.scale = Some(d.scale.unwrap_or(parent_scale * 2.0));
        v.detail_profile = d.profile.clone();
        v.detail_style = d.style;
        v.detail_hide_profile = !d.show_profile;
        v.detail_clip_border = d.clip_border;
        if !d.label.is_empty()
            && let ViewKind::Detail { label, .. } = &mut v.kind
        {
            *label = d.label.clone();
        }
    }
}

/// A Detail view profile from the dialog's shape (1 Rectangle: two
/// corners; 2 Spline, 3 Polygon: the points) in parent view coordinates:
/// the closed polygon, its center and the radius around it (the detail's
/// circle). None when there are too few points.
pub fn detail_profile(shape: u8, pts: &[[f64; 2]]) -> Option<(Vec<[f64; 2]>, [f64; 2], f64)> {
    let poly: Vec<[f64; 2]> = match shape {
        1 if pts.len() >= 2 => {
            let (a, b) = (pts[0], pts[1]);
            vec![a, [b[0], a[1]], b, [a[0], b[1]]]
        }
        2 if pts.len() >= 3 => views::closed_spline(pts, 16),
        3 if pts.len() >= 3 => pts.to_vec(),
        _ => return None,
    };
    let n = poly.len() as f64;
    let center = [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n];
    let radius = poly.iter().map(|p| ((p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2)).sqrt()).fold(0.0, f64::max);
    Some((poly, center, radius))
}

// ------------------------------------------------------------ hatch

/// A closed profile: outer boundary and holes (sheet mm, or view
/// coordinates for a profile in a view).
pub type Profile = (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>);

/// "Auto" (None) or the number typed (a trailing unit or ° ignored).
pub fn parse_auto(s: &str) -> Option<f64> {
    let t = s.trim().trim_end_matches(['°', ' ']).trim_end_matches("deg").trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") { None } else { t.parse::<f64>().ok() }
}

/// Hatch region dialog.
#[derive(Clone, Debug, Default)]
pub struct HatchOpts {
    /// Pattern group (tab) and pattern ([`hatch_pat::pattern`]).
    pub standard: usize,
    pub pattern: usize,
    /// Scale and angle as typed: "Auto" or a number (degrees).
    pub scale: String,
    pub angle: String,
    /// Use the drawing's hatch color, else `color`.
    pub drawing_color: bool,
    pub color: Option<[u8; 3]>,
    /// The picked closed profiles, each in a view or on the sheet.
    pub profiles: Vec<(Profile, Option<ViewId>)>,
}

impl HatchOpts {
    pub fn scale_value(&self) -> f64 {
        if self.standard == 2 {
            return 1.0;
        }
        parse_auto(&self.scale).filter(|s| *s > 0.0).unwrap_or(1.0)
    }

    /// The angle added to the pattern's own.
    pub fn angle_value(&self) -> f64 {
        if self.standard == 2 {
            return 0.0;
        }
        parse_auto(&self.angle).map(|a| a - hatch_pat::base_angle(self.standard, self.pattern)).unwrap_or(0.0)
    }

    /// The regions the dialog creates, ids from `first_id`.
    pub fn regions(&self, first_id: u64) -> Vec<HatchRegion> {
        let (scale, angle) = (self.scale_value(), self.angle_value());
        self.profiles
            .iter()
            .enumerate()
            .map(|(i, ((boundary, holes), view))| HatchRegion {
                id: first_id + i as u64,
                boundary: boundary.clone(),
                holes: holes.clone(),
                view: *view,
                standard: self.standard,
                pattern: self.pattern,
                scale,
                angle,
                color: if self.drawing_color { None } else { self.color.or(Some([0, 0, 0])) },
            })
            .collect()
    }
}

// ------------------------------------------------------------ table tool

/// New general tables: column width and row height (mm).
pub const TABLE_COL_W: f64 = 28.0;

/// Table dialog: rows and columns as typed (defaults 2, clamped 1–60 and 1–20).
pub fn table_size(rows: &str, cols: &str) -> (usize, usize) {
    let r = rows.trim().parse::<usize>().unwrap_or(2).clamp(1, 60);
    let c = cols.trim().parse::<usize>().unwrap_or(2).clamp(1, 20);
    (r, c)
}

/// The general table the Table dialog creates, its fixed corner at `at`.
pub fn general_table(rows: usize, cols: usize, title_row: bool, header_row: bool, corner: TableCorner, at: [f64; 2]) -> Table {
    let cells = vec![vec![String::new(); cols]; rows + header_row as usize];
    let mut t = Table::new(0, TableKind::General { cells, title: title_row.then(String::new), header: header_row }, at);
    t.col_widths = vec![TABLE_COL_W; cols];
    t.corner = corner;
    t
}

// ------------------------------------------------------------ geometric tolerance

/// The Type list (glyph, name) in Onshape's order.
pub const GDT_TYPES: [(&str, &str); 14] = [
    ("⌖", "Position"),
    ("◎", "Concentricity"),
    ("⌯", "Symmetry"),
    ("∥", "Parallelism"),
    ("⟂", "Perpendicularity"),
    ("∠", "Angularity"),
    ("⌭", "Cylindricity"),
    ("▱", "Flatness"),
    ("○", "Roundness"),
    ("⏤", "Straightness"),
    ("⌓", "Profile (Surface)"),
    ("⌒", "Profile (Line)"),
    ("↗", "Circular runout"),
    ("⌰", "Total runout"),
];

/// One frame row of the Geometric tolerance dialog, as typed.
#[derive(Clone, Debug, Default)]
pub struct GdtRowValues {
    /// Index into [`GDT_TYPES`] (None: the first row's default, Position;
    /// later rows: Composite).
    pub symbol: Option<usize>,
    pub prefix: String,
    /// The tolerance box's before / value / after slots.
    pub before: String,
    pub value: String,
    pub after: String,
    pub datums: [String; 3],
    pub suffix: String,
}

/// The dialog's frame (first row) and extra; None when nothing is entered.
pub fn gdt_values(rows: &[GdtRowValues], upper: &str, lower: &str, leader: GdtLeader) -> Option<(Vec<String>, GdtExtra)> {
    let first = rows.first()?;
    let cells = |r: &GdtRowValues, first: bool| -> Vec<String> {
        let symbol = match r.symbol {
            Some(i) => GDT_TYPES[i].0.to_string(),
            None if first => GDT_TYPES[0].0.to_string(),
            None => "Composite".to_string(),
        };
        let mut c = vec![symbol, format!("{}{}{}", r.before.trim(), r.value.trim(), r.after.trim())];
        c.extend(r.datums.iter().map(|d| d.trim().to_string()));
        while c.len() > 2 && c.last().is_some_and(|x| x.is_empty()) {
            c.pop();
        }
        c
    };
    let frame = cells(first, true);
    let mut extra = GdtExtra {
        upper: upper.trim().to_string(),
        lower: lower.trim().to_string(),
        prefix: rows.iter().map(|r| r.prefix.trim().to_string()).collect(),
        suffix: rows.iter().map(|r| r.suffix.trim().to_string()).collect(),
        rows: rows.iter().skip(1).map(|r| cells(r, false)).collect(),
        leader,
        on: None,
    };
    if extra.prefix.iter().all(String::is_empty) {
        extra.prefix.clear();
    }
    if extra.suffix.iter().all(String::is_empty) {
        extra.suffix.clear();
    }
    let entered = frame.iter().skip(1).any(|c| !c.is_empty()) || !extra.upper.is_empty() || !extra.lower.is_empty() || extra.rows.iter().any(|r| r.iter().skip(1).any(|c| !c.is_empty()));
    entered.then_some((frame, extra))
}

/// The cutting line from `a` to `b` (view coordinates) jogged through each of
/// `jogs` in turn: along the line to the jog point's level, across to it,
/// then on along the line's direction to `b`'s level.
pub fn jogged_line(a: [f64; 2], b: [f64; 2], jogs: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if l < 1e-12 {
        return vec![a, b];
    }
    let e = [d[0] / l, d[1] / l];
    let along = |from: [f64; 2], to: [f64; 2]| {
        let t = (to[0] - from[0]) * e[0] + (to[1] - from[1]) * e[1];
        [from[0] + e[0] * t, from[1] + e[1] * t]
    };
    let mut out = vec![a];
    let mut cur = a;
    for q in jogs {
        out.push(along(cur, *q));
        out.push(*q);
        cur = *q;
    }
    out.push(if jogs.is_empty() { b } else { along(cur, b) });
    out
}

#[cfg(test)]
mod tests {
    use super::jogged_line;

    #[test]
    fn a_jog_runs_through_the_snap_point() {
        // Vertical line x = 0 from y 10 down to y -10, jogged through (5, 2).
        let l = jogged_line([0.0, 10.0], [0.0, -10.0], &[[5.0, 2.0]]);
        assert_eq!(l, vec![[0.0, 10.0], [0.0, 2.0], [5.0, 2.0], [5.0, -10.0]]);
        assert_eq!(jogged_line([0.0, 10.0], [0.0, -10.0], &[]), vec![[0.0, 10.0], [0.0, -10.0]]);
    }
}
