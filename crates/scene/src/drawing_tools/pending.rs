//! Drawing slice 3: the annotation and view the active tool would create
//! at the pointer (the live preview, and what a click commits), from
//! `crates/app/src/drawing_tools.rs`, `drawing_extra.rs`, `drawing_more.rs`
//! and `drawing_sketch_dim.rs`.

use super::geom::*;
use super::{DimTool, Pick, SheetPick, Tool};
use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// Editor and dialog state the previews read, as plain values (the app
/// fills it from its dialogs; the agent API from its arguments).
#[derive(Clone, Debug, Default)]
pub struct ToolValues {
    /// The Callout dialog's texts and border, while it is open.
    pub callout: Option<(String, CalloutStyle)>,
    /// The Datum dialog's label text, while it is open.
    pub datum_label: Option<String>,
    /// Datum targets waiting in the Datum dialog.
    pub datum_rows: Vec<Annotation>,
    /// Datum tool: the annotation line the triangle was put on.
    pub datum_on: Option<DatumOn>,
    /// Datum tool: the view point the triangle was put on.
    pub datum_at: Option<(ViewId, [f64; 2])>,
    /// Datum target tool: the letter of the set being placed.
    pub datum_target_letter: Option<char>,
    /// Dimension tool: the drawing-sketch entity picked (SK7).
    pub sketch_dim_pick: Option<u32>,
}

impl SheetPick<'_> {
    /// The annotation the active dimension tool would create at the cursor.
    pub fn pending_annotation(&self, v: &ToolValues) -> Option<Annotation> {
        // New dimensions follow Drawing properties › Break dimension by default.
        let mut a = self.pending_annotation_raw(v)?;
        a.unbroken = a.kind.is_dimension() && !self.drawing.props.break_dims;
        Some(a)
    }

    fn pending_annotation_raw(&self, v: &ToolValues) -> Option<Annotation> {
        let c = self.cursor;
        let id = self.current_sheet().next_annotation_id();
        let ann = |kind, refs: Vec<(u8, Option<PointRef>)>| Annotation {
            id,
            kind,
            position: c,
            refs: refs.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect(),
            ..Default::default()
        };
        if let Some(a) = self.extra_pending_annotation(id, v) {
            return Some(a);
        }
        if let Some(a) = self.sketch_dim_preview(v.sketch_dim_pick) {
            return Some(a);
        }
        match self.tool {
            Tool::Dimension { kind, picks } if kind.linear() => {
                if *kind != DimTool::Linear && picks.len() < 2 {
                    return None;
                }
                let (a, b) = match picks.as_slice() {
                    [Pick::Line { a, b }] => (a.clone(), b.clone()),
                    [Pick::Point(a), Pick::Point(b)] if a.view == b.view => (a.clone(), b.clone()),
                    // The smart Dimension (d): the picks decide the kind (drawing_smart).
                    _ => {
                        let (kind, refs) = self.smart_dimension(picks)?;
                        let mut a = ann(kind, refs);
                        if matches!(a.kind, AnnotationKind::LinearDimension { .. }) && self.cylinder_silhouettes(picks) {
                            a.format.prefix = "Ø".into();
                        }
                        return Some(a);
                    }
                };
                let (ps, qs) = (self.pick_sheet(&a)?, self.pick_sheet(&b)?);
                // Horizontal / vertical / aligned follows the cursor (Onshape).
                let o = auto_orientation(ps, qs, c);
                let offset = dim_offset(ps, qs, o, c);
                Some(ann(AnnotationKind::LinearDimension { view: a.view, a: a.p, b: b.p, offset, orientation: o, override_text: None }, vec![(0, a.r), (1, b.r)]))
            }
            // Diameter on a cylinder's silhouette: across both silhouettes.
            Tool::Dimension { kind: DimTool::Diameter, picks } if matches!(picks.as_slice(), [Pick::Line { .. }, Pick::Line { .. }]) => {
                let (kind, refs) = self.smart_dimension(picks)?;
                let mut a = ann(kind, refs);
                if self.cylinder_silhouettes(picks) {
                    a.format.prefix = "Ø".into();
                }
                Some(a)
            }
            Tool::Dimension { kind: kind @ (DimTool::Radius | DimTool::Diameter), picks } => {
                let [Pick::Circle { view, center, radius, r }] = picks.as_slice() else { return None };
                let cs = self.placement(*view)?.to_sheet(*center);
                let d = sub(c, cs);
                Some(ann(AnnotationKind::RadialDimension { view: *view, center: *center, radius: *radius, diameter: *kind == DimTool::Diameter, angle: d[1].atan2(d[0]), dim_line: *kind == DimTool::Diameter && self.drawing.radial_dim_lines(), knee: self.radial_knee(*view, *center, *radius, c) }, vec![(0, r.clone())]))
            }
            Tool::Dimension { kind: DimTool::Angle3, picks } => {
                let [Pick::Point(v), Pick::Point(pa), Pick::Point(pb)] = picks.as_slice() else { return None };
                if v.view != pa.view || v.view != pb.view {
                    return None;
                }
                // The smaller angle between the legs (Onshape measures < 180°).
                let ang = |p: [f64; 2]| (p[1] - v.p[1]).atan2(p[0] - v.p[0]);
                let mut sweep = ang(pb.p) - ang(pa.p);
                while sweep < 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                let (mut a, mut b) = if sweep > std::f64::consts::PI { (pb, pa) } else { (pa, pb) };
                let pl = self.placement(v.view)?;
                // The arc on the other side of a leg: 360° minus the angle.
                if !in_sweep(pl.to_sheet(v.p), pl.to_sheet(a.p), pl.to_sheet(b.p), c) {
                    std::mem::swap(&mut a, &mut b);
                }
                let radius = len(sub(c, pl.to_sheet(v.p))).max(3.0);
                Some(ann(AnnotationKind::AngularDimension { view: v.view, vertex: v.p, a: a.p, b: b.p, radius, three_point: true, flip_a: false, flip_b: false, text_angle: None }, vec![(0, v.r.clone()), (1, a.r.clone()), (2, b.r.clone())]))
            }
            Tool::Dimension { kind: DimTool::Angle, picks } => {
                let [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }] = picks.as_slice() else { return None };
                let (kind, refs) = self.angular_from_lines(a1, b1, a2, b2)?;
                Some(ann(kind, refs))
            }

            Tool::HoleCallout { circle: Some((view, center, radius, r)) } => {
                let mut a = ann(AnnotationKind::HoleCallout { view: *view, center: *center, radius: *radius, leaders: Vec::new() }, vec![(0, r.clone())]);
                // Help hole_thread_callout (drawing.md A58): the shaft class
                // starts hidden ("Toggle shaft class visibility").
                a.format.fit.hide_shaft = true;
                Some(a)
            }
            Tool::Balloon { anchor: Some(a) } => {
                // In assembly views a callout on a part shows its BOM item (empty text = linked).
                // A click on an edge away from its snap points: that edge's part.
                // On a dimension: no part (its leader ends on the dimension).
                let part = if a.r.is_none() && v.datum_on.is_none() { self.part_near(a.view, a.p) } else { None };
                let linked = views::view_source(self.current_sheet(), a.view).and_then(|(k, _)| self.sources.get(&k)).is_some_and(|s| s.assembly) && (a.r.is_some() || part.is_some());
                // The Callout dialog's texts and border (empty centre = the
                // BOM item number).
                let (text, style) = v.callout.clone().unwrap_or_default();
                let mut kind = AnnotationKind::balloon(a.view, a.p, text);
                // A click on an edge or snap point: the leader ends in an
                // arrow, also on an assembly part; a dot only inside a face
                // (`finish_callout`, live qa4 round 4 S2).
                if let AnnotationKind::Balloon { callout, .. } = &mut kind {
                    *callout = style;
                    callout.on_face = leader_dot(true, linked);
                    callout.part = part;
                    callout.on = v.datum_on;
                }
                Some(ann(kind, vec![(0, a.r.clone())]))
            }
            _ => None,
        }
    }

    /// Sheet direction perpendicular to the ordinate's measured axis.
    pub fn ordinate_normal(&self, view: ViewId, horizontal: bool) -> [f64; 2] {
        let rot = self.placement(view).map(|p| p.rotation).unwrap_or(0.0);
        views::rotate2(if horizontal { [0.0, 1.0] } else { [1.0, 0.0] }, rot)
    }

    pub fn extra_pending_annotation(&self, id: u64, v: &ToolValues) -> Option<Annotation> {
        if let Some(a) = self.more_pending_annotation(id, v) {
            return Some(a);
        }
        let c = self.cursor;
        let refs = |v: Vec<(u8, Option<PointRef>)>| v.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect::<Vec<_>>();
        match self.tool {
            Tool::Dimension { kind: DimTool::Chamfer, picks } => {
                let [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }] = picks.as_slice() else { return None };
                if a1.view != a2.view {
                    return None;
                }
                Some(Annotation {
                    id,
                    kind: AnnotationKind::ChamferDimension { view: a1.view, a: a1.p, b: b1.p, c: a2.p, d: b2.p },
                    position: c,
                    refs: refs(vec![(0, a1.r.clone()), (1, b1.r.clone()), (2, a2.r.clone()), (3, b2.r.clone())]),
                    ..Default::default()
                })
            }
            Tool::Dimension { kind: DimTool::ArcLength, picks } => {
                let [Pick::Arc { view, center, radius, a, b, r, .. }] = picks.as_slice() else { return None };
                let pl = self.placement(*view)?;
                // The side of the arc the pointer is on: the picked arc, or
                // across its ends the conjugate.
                let (mid, offset) = super::jog::arc_length_place(&pl, *center, *radius, c);
                Some(Annotation {
                    id,
                    kind: AnnotationKind::ArcLengthDimension { view: *view, center: *center, radius: *radius, a: a.p, b: b.p, mid, offset },
                    position: c,
                    refs: refs(vec![(0, r.clone()), (1, a.r.clone()), (2, b.r.clone())]),
                    ..Default::default()
                })
            }
            Tool::Ordinate { origin: Some(o), horizontal: None, .. } => {
                let os = self.pick_sheet(o)?;
                let rot = self.placement(o.view).map(|p| p.rotation).unwrap_or(0.0);
                let d = views::rotate2(sub(c, os), -rot);
                if len(d) < 3.0 {
                    return None;
                }
                let horizontal = d[1].abs() >= d[0].abs();
                let n = self.ordinate_normal(o.view, horizontal);
                let pos = add2(os, mul(n, dot(sub(c, os), n)));
                // A new ordinate group holding the datum's own "0".
                Some(Annotation { id, kind: AnnotationKind::OrdinateGroup { view: o.view, origin: o.p, points: vec![o.p], horizontal, shifts: Vec::new(), knees: Vec::new() }, position: pos, refs: refs(vec![(0, o.r.clone()), (1, o.r.clone())]), ..Default::default() })
            }
            Tool::Ordinate { origin: Some(o), horizontal: Some(_), group: Some(gid), .. } => {
                // The group with the point under the cursor added.
                let p = self.snap_at(c).filter(|p| p.view == o.view)?;
                let mut a = self.current_sheet().annotations.iter().find(|a| a.id == *gid)?.clone();
                let AnnotationKind::OrdinateGroup { points, .. } = &mut a.kind else { return None };
                if points.iter().any(|q| len(sub(*q, p.p)) < 1e-9) {
                    return None;
                }
                points.push(p.p);
                let slot = points.len() as u8;
                if let Some(r) = p.r.clone() {
                    a.refs.push(AnnotationRef { slot, point: r });
                }
                Some(a)
            }
            Tool::Datum { anchor: Some(a) } => {
                if len(sub(c, *a)) < 3.0 {
                    return None;
                }
                Some(Annotation { id, kind: AnnotationKind::Datum { anchor: *a, label: v.datum_label.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| self.next_datum_label()), on: v.datum_on, at: v.datum_at }, position: c, ..Default::default() })
            }
            _ => None,
        }
    }

    /// Placement of a section view perpendicular to the first leg.
    pub fn section_position(&self, parent: &DrawingView, a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
        let pl = self.placement(parent.id)?;
        let e = norm(sub(pl.to_sheet(b), pl.to_sheet(a)));
        let n = [-e[1], e[0]];
        let t = dot(sub(self.cursor, parent.position), n);
        Some(add2(parent.position, mul(n, t)))
    }

    pub fn extra_pending_view(&self, id: ViewId) -> Option<DrawingView> {
        let sheet = self.drawing.sheets.get(self.sheet)?;
        match self.tool {
            Tool::JoggedSection { parent: Some(pid), points, done: true } if points.len() >= 2 => {
                let parent = sheet.view(*pid)?;
                let pos = self.section_position(parent, points[0], points[1])?;
                Some(DrawingView { id, kind: ViewKind::Section { parent: *pid, line: points.clone(), label: sheet.next_label(), cuts: vec![] }, position: pos, show_tangent_edges: parent.show_tangent_edges, model_annotations: parent.model_annotations, ..Default::default() })
            }
            Tool::AlignedSection { parent: Some(pid), center: Some(c), a: Some(a), b: Some(b) } => {
                let parent = sheet.view(*pid)?;
                let pos = self.section_position(parent, *a, *c)?;
                Some(DrawingView { id, kind: ViewKind::AlignedSection { parent: *pid, center: *c, a: *a, b: *b, label: sheet.next_label() }, position: pos, show_tangent_edges: parent.show_tangent_edges, model_annotations: parent.model_annotations, ..Default::default() })
            }
            _ => None,
        }
    }

    pub fn more_pending_annotation(&self, id: u64, v: &ToolValues) -> Option<Annotation> {
        let c = self.cursor;
        let refs = |v: Vec<(u8, Option<PointRef>)>| v.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect::<Vec<_>>();
        match self.tool {
            Tool::Dimension { kind: DimTool::MaxMin, picks } if picks.len() == 2 => {
                let Pick::CircleAt { view, center, radius, r, at } = &picks[0] else { return None };
                let mut edge = Vec::new();
                let (b, rb, cb, rref) = match &picks[1] {
                    Pick::Point(p) => (p.p, 0.0, ArcCondition::Center, p.r.clone()),
                    Pick::CircleAt { center: c2, radius: r2, r, at: at2, .. } => {
                        let near = dot(sub(*at2, *c2), sub(*center, *c2)) > 0.0;
                        (*c2, *r2, if near { ArcCondition::Min } else { ArcCondition::Max }, r.clone())
                    }
                    // A straight edge: measured square to it, from the
                    // perpendicular's foot.
                    Pick::Line { a: e0, b: e1 } => {
                        edge = vec![(2, e0.r.clone()), (3, e1.r.clone())];
                        (noshape_model::drawing::build::foot_on_line(*center, e0.p, e1.p), 0.0, ArcCondition::Center, None)
                    }
                    _ => return None,
                };
                let ca = if dot(sub(*at, *center), sub(b, *center)) > 0.0 { ArcCondition::Min } else { ArcCondition::Max };
                let pl = self.placement(*view)?;
                let (ps, qs) = (pl.to_sheet(*center), pl.to_sheet(b));
                let o = if edge.is_empty() { auto_orientation(ps, qs, c) } else { DimOrientation::Aligned };
                let offset = dim_offset(ps, qs, o, c);
                Some(Annotation {
                    id,
                    kind: AnnotationKind::MaxMinDimension { view: *view, a: *center, ra: *radius, ca, b, rb, cb, offset, orientation: o },
                    position: c,
                    refs: refs([vec![(0, r.clone()), (1, rref)], edge].concat()),
                    ..Default::default()
                })
            }
            Tool::Dimension { kind: DimTool::Foreshorten, picks } if picks.len() == 2 => {
                let Pick::Line { a, b } = &picks[0] else { return None };
                let e = match &picks[1] {
                    Pick::Point(p) => p.clone(),
                    Pick::Line { a, .. } => a.clone(),
                    _ => return None,
                };
                Some(Annotation {
                    id,
                    kind: AnnotationKind::ForeshortenedDiameter { view: a.view, axis: [a.p, b.p], edge: e.p, direct: false },
                    position: c,
                    refs: refs(vec![(0, a.r.clone()), (1, b.r.clone()), (2, e.r.clone())]),
                    ..Default::default()
                })
            }
            Tool::DatumTarget { anchor: Some(a) } => {
                let s = self.pick_sheet(a)?;
                if len(sub(c, s)) < 4.0 {
                    return None;
                }
                // Help datum: the digit increments within a set (A1, A2…), a
                // new set takes the next letter (B1, C1…).
                let letter = v.datum_target_letter.unwrap_or_else(|| self.next_target_set());
                let in_set = |x: &&Annotation| matches!(&x.kind, AnnotationKind::DatumTarget { label, .. } if label.starts_with(letter));
                // Targets waiting in the Datum dialog count too.
                let pending = v.datum_rows.iter().filter(in_set).count();
                let n = self.drawing.sheets[self.sheet].annotations.iter().filter(in_set).count() + pending + 1;
                Some(Annotation { id, kind: AnnotationKind::DatumTarget { view: a.view, anchor: a.p, label: format!("{letter}{n}"), area: DatumTargetArea::Point }, position: c, refs: refs(vec![(0, a.r.clone())]), ..Default::default() })
            }
            _ => None,
        }
    }

    /// The letter of a new datum target set: after the last set's letter
    /// (datum letters, skipping I, O, Q, S, X, Z). The drawing's, not the
    /// preview's, which holds the target being placed (qa DWG-DATUM-TARGET-MENU: B1).
    pub fn next_target_set(&self) -> char {
        const LETTERS: &str = "ABCDEFGHJKLMNPRTUVWY";
        let last = self.drawing.sheets[self.sheet].annotations.iter().filter_map(|a| if let AnnotationKind::DatumTarget { label, .. } = &a.kind { label.chars().next() } else { None }).filter_map(|c| LETTERS.find(c)).max();
        match last {
            Some(i) => LETTERS.chars().nth(i + 1).unwrap_or('A'),
            None => 'A',
        }
    }

    pub fn next_datum_label(&self) -> String {
        let used: Vec<String> = self.current_sheet().annotations.iter().filter_map(|a| if let AnnotationKind::Datum { label, .. } = &a.kind { Some(label.clone()) } else { None }).collect();
        "ABCDEFGHJKLMNPRTUVWY".chars().map(|c| c.to_string()).find(|l| !used.contains(l)).unwrap_or_else(|| "A".into())
    }

    /// The dimension following the pointer after a sketch entity was picked.
    pub fn sketch_dim_preview(&self, pick: Option<u32>) -> Option<Annotation> {
        let e = pick?;
        let mut s = self.current_sheet().clone();
        let id = s.add_sketch_dimension(e, self.cursor)?;
        s.annotations.into_iter().find(|a| a.id == id)
    }
}

/// Whether a callout's leader ends in a dot: only when it is attached inside a
/// face (or a silhouette) of an assembly part; on an edge or a snap point it
/// ends in an arrow (live qa4 round 4 S2: arrowheads on Parts 1 and 3's edges,
/// a dot inside Part 2's face).
pub(crate) fn leader_dot(on_edge_or_point: bool, assembly_part: bool) -> bool {
    !on_edge_or_point && assembly_part
}

#[cfg(test)]
mod leader_tests {
    #[test]
    fn callout_leaders_end_in_arrows_on_edges() {
        assert!(!super::leader_dot(true, true), "an edge of an assembly part: arrow");
        assert!(super::leader_dot(false, true), "inside an assembly part's face: dot");
        assert!(!super::leader_dot(false, false));
    }
}
