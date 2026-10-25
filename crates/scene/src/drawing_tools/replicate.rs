//! View menu › Select for replication ▸ View | Region… and Replicate
//! annotations (live drawing/447–452, dom/drawing-replicate-*.txt), shared
//! by the drawing editor and the agent API. View remembers the view's
//! annotations; Region… those inside a rectangle. Replicate annotations on
//! another view copies them there: a copy is made where the target view
//! shows the same geometry (each reference the same model point, or a
//! vertex / circle center at the mapped spot); the others are skipped.

use super::SheetPick;
use super::Sel;
use noshape_model::drawing::build::ViewPlacement;
use noshape_model::drawing::views::{self, SnapKind};
use noshape_model::drawing::{Annotation, AnnotationRef, PointRef, ViewId};

/// What Select for replication picked: the annotations (with their
/// reference points, view coordinates), the source view's placement and
/// the region (sheet mm), if any.
#[derive(Clone)]
pub struct Replication {
    pub items: Vec<(Annotation, Vec<[f64; 2]>)>,
    pub src: ViewPlacement,
    pub region: Option<[[f64; 2]; 2]>,
}

/// References match within this (view coordinates, m).
const MATCH_TOL: f64 = 1e-5;

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl SheetPick<'_> {
    /// Select for replication: the view's annotations (those whose drawn
    /// center lies inside `region`, sheet mm, if any).
    pub fn select_for_replication(&self, view: ViewId, region: Option<[[f64; 2]; 2]>) -> Option<Replication> {
        let src = self.placement(view)?;
        let geo = self.geoms.get(&view)?.clone();
        let sheet = self.current_sheet();
        let ctx = views::view_source(sheet, view).and_then(|(k, _)| self.sources.get(&k)).map(|s| s.context.clone())?;
        let inside = |a: &Annotation| -> bool {
            let Some(r) = region else { return true };
            let Some(b) = self.built else { return false };
            let picks = self.picks_of(Sel::Annotation(a.id));
            let pts: Vec<[f64; 2]> = b.geometry.lines.iter().filter(|l| picks.contains(&l.pick)).flat_map(|l| l.points.iter().copied()).collect();
            if pts.is_empty() {
                return false;
            }
            let c = pts.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
            let c = [c[0] / pts.len() as f64, c[1] / pts.len() as f64];
            (r[0][0]..=r[1][0]).contains(&c[0]) && (r[0][1]..=r[1][1]).contains(&c[1])
        };
        let mut items = Vec::new();
        for a in sheet.annotations.iter().filter(|a| a.kind.view() == Some(view) && inside(a)) {
            let pts: Option<Vec<[f64; 2]>> = a.refs.iter().map(|r| views::resolve_in_view(&ctx, &geo, &r.point).map(|(p, _)| p)).collect();
            if let Some(pts) = pts {
                items.push((a.clone(), pts));
            }
        }
        Some(Replication { items, src, region })
    }

    /// Replicate annotations onto `target`: the copies (ids from the
    /// sheet's next annotation id) and how many could have landed (live's
    /// "n/m"). `delta` (sheet mm) moves a region's contents, else view
    /// coordinates carry over.
    pub fn replicate_into(&self, r: &Replication, target: ViewId, delta: Option<[f64; 2]>) -> Option<(Vec<Annotation>, usize)> {
        let (dst, geo) = (self.placement(target)?, self.geoms.get(&target)?.clone());
        let sheet = self.current_sheet();
        let ctx = views::view_source(sheet, target).and_then(|(k, _)| self.sources.get(&k)).map(|s| s.context.clone())?;
        let src = r.src;
        let map = |p: [f64; 2]| match delta {
            None => p,
            Some(d) => {
                let s = src.to_sheet(p);
                dst.to_view([s[0] + d[0], s[1] + d[1]])
            }
        };
        let shift = delta.unwrap_or_else(|| {
            let (a, b) = (src.to_sheet([0.0, 0.0]), dst.to_sheet([0.0, 0.0]));
            [b[0] - a[0], b[1] - a[1]]
        });
        // A placed box is positioned by hand: snap it so the first vertex /
        // circle center lands on the nearest one of its kind (3 mm on the
        // sheet), and move every point by the same amount.
        let kind_of = |p: &PointRef| match p {
            PointRef::Vertex { .. } => Some(SnapKind::Vertex),
            PointRef::Center { .. } => Some(SnapKind::Center),
            _ => None,
        };
        let mut corr = [0.0, 0.0];
        if delta.is_some() {
            let tol = 3.0 / (dst.scale * 1000.0).max(1e-9);
            let first = r.items.iter().flat_map(|(a, pts)| a.refs.iter().zip(pts)).find_map(|(rf, p)| kind_of(&rf.point).map(|k| (k, map(*p))));
            if let Some((k, q)) = first
                && let Some(s) = geo.snaps.iter().filter(|s| s.kind == k).min_by(|a, b| dist(a.p, q).total_cmp(&dist(b.p, q)))
                && dist(s.p, q) < tol
            {
                corr = [s.p[0] - q[0], s.p[1] - q[1]];
            }
        }
        let map = |p: [f64; 2]| {
            let q = map(p);
            [q[0] + corr[0], q[1] + corr[1]]
        };
        let shift = match delta {
            Some(_) => {
                let (a, b) = (dst.to_sheet([0.0, 0.0]), dst.to_sheet(corr));
                [shift[0] + b[0] - a[0], shift[1] + b[1] - a[1]]
            }
            None => shift,
        };
        let mut next = sheet.next_annotation_id();
        let existing: Vec<&Annotation> = sheet.annotations.iter().filter(|a| a.kind.view() == Some(target)).collect();
        let mut out: Vec<Annotation> = Vec::new();
        // Tried: the annotations that could land (live's "n/m").
        let mut tried = 0usize;
        for (a, pts) in &r.items {
            // Region copies need references to land on.
            if a.refs.is_empty() && delta.is_some() {
                continue;
            }
            tried += 1;
            let mut refs = Vec::new();
            for (rf, p) in a.refs.iter().zip(pts) {
                let q = map(*p);
                // A circle's center lands only where the circle shows (live: the
                // Ø of a hole seen edge-on is not replicated).
                let shown = !matches!(rf.point, PointRef::Center { .. }) || geo.snaps.iter().any(|s| s.kind == SnapKind::Center && dist(s.p, q) < MATCH_TOL);
                if shown && views::resolve_in_view(&ctx, &geo, &rf.point).is_some_and(|(x, _)| dist(x, q) < MATCH_TOL) {
                    refs.push(rf.clone());
                    continue;
                }
                let Some(point) = kind_of(&rf.point).and_then(|k| geo.snaps.iter().find(|s| s.kind == k && dist(s.p, q) < MATCH_TOL)).and_then(|s| s.point.clone()) else { break };
                refs.push(AnnotationRef { slot: rf.slot, point });
            }
            if refs.len() != a.refs.len() {
                continue;
            }
            let same = |e: &&Annotation| std::mem::discriminant(&e.kind) == std::mem::discriminant(&a.kind) && e.refs == refs;
            if !refs.is_empty() && (existing.iter().any(same) || out.iter().any(|e| same(&e))) {
                continue;
            }
            let mut b = a.clone();
            b.id = next;
            next += 1;
            b.refs = refs;
            if let Some(v) = b.kind.view_mut() {
                *v = target;
            }
            b.position = [b.position[0] + shift[0], b.position[1] + shift[1]];
            b.group = None;
            b.view_group = None;
            b.ann_group = None;
            b.inspection = None;
            out.push(b);
        }
        Some((out, tried))
    }
}

/// The offset a placed region box applies: its center `at` (sheet mm)
/// minus the source region's center.
pub fn region_delta(region: [[f64; 2]; 2], at: [f64; 2]) -> [f64; 2] {
    let c = [(region[0][0] + region[1][0]) * 0.5, (region[0][1] + region[1][1]) * 0.5];
    [at[0] - c[0], at[1] - c[1]]
}
