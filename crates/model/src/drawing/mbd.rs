//! MBD annotations in drawings (live behaviours "MBD datums in drawings",
//! V5 / CR18 / A65): a Part Studio's Model definitions datums appear once,
//! in the first view of the suggested set that shows their face edge-on, as
//! ordinary drawing datums (filled triangle on the edge, a stem, the boxed
//! letter). They stay linked: the anchor follows the face, the box moves
//! only along the face normal, Delete hides them (the MBD status panel lists
//! them and "Show" brings them back where they were), and a view's
//! "Show annotations from model" off hides its own.

use super::views::{self, Sources, ViewFrame, ViewGeometry};
use super::{Annotation, AnnotationKind, Drawing, Sheet, ViewId};
use crate::annotations::{Annotation as ModelAnnotation, AnnotationKind as ModelKind};
use crate::document::{Document, TabContent, TabId};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A drawing annotation showing a Part Studio's MBD annotation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelRef {
    pub tab: TabId,
    pub id: u32,
    /// The view showing it.
    pub view: ViewId,
    /// The box's distance from the edge along the face normal (sheet mm).
    pub offset: f64,
    /// Deleted from the drawing (MBD status › Show restores it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// Live drawing/1071: the box centre sits about 0.28 in from the edge
/// (datum size 0.12 in, a short stem, the 0.24 in box).
pub const DEFAULT_OFFSET: f64 = 7.2;

/// Is `normal` edge-on (perpendicular to the line of sight) in `frame`?
fn edge_on(frame: &ViewFrame, normal: DVec3) -> bool {
    normal.normalize_or_zero().dot(frame.dir.normalize()).abs() < 1e-6
}

/// Place the source Part Studios' datums in a new drawing (Suggested views):
/// each in the first view, in sheet order, that shows its face edge-on and
/// shows annotations from the model.
pub fn place_model_datums(drawing: &mut Drawing, doc: &Document) {
    let mut placed: Vec<(TabId, u32)> = drawing.sheets.iter().flat_map(|s| &s.annotations).filter_map(|a| a.model.as_ref().map(|m| (m.tab, m.id))).collect();
    for si in 0..drawing.sheets.len() {
        let mut add: Vec<Annotation> = Vec::new();
        {
            let sheet = &drawing.sheets[si];
            for v in &sheet.views {
                if !v.model_annotations {
                    continue;
                }
                let Some((key, _)) = views::view_source(sheet, v.id) else { continue };
                let Some(TabContent::PartStudio(ps)) = doc.tab(key.tab).map(|t| &t.content) else { continue };
                let Some(frame) = views::view_frame(drawing, sheet, v.id) else { continue };
                for m in &ps.annotations {
                    if placed.contains(&(key.tab, m.id)) || !edge_on(&frame, DVec3::from_array(m.normal)) {
                        continue;
                    }
                    let ModelKind::Datum { letter } = &m.kind;
                    placed.push((key.tab, m.id));
                    add.push(Annotation {
                        kind: AnnotationKind::Datum { anchor: v.position, label: letter.clone(), on: None },
                        position: v.position,
                        model: Some(ModelRef { tab: key.tab, id: m.id, view: v.id, offset: DEFAULT_OFFSET, hidden: false }),
                        ..Default::default()
                    });
                }
            }
        }
        let sheet = &mut drawing.sheets[si];
        for mut a in add {
            a.id = sheet.next_annotation_id();
            sheet.annotations.push(a);
        }
    }
}

/// Where a model datum sits now: its anchor on the face's edge and its box
/// square to the edge (sheet mm), from the view's geometry and its sheet
/// placement `to_sheet` (view coordinates → sheet). The model symbol lies in
/// the face, edge-on here, so it can't choose the side: live puts the box
/// below the edge (drawing/1070, mbd-drawing-json: A under the Front view's
/// top edge, inside the view; B under the Top view's bottom edge). A
/// vertical edge keeps the face's outward side.
pub fn model_datum_points(m: &ModelAnnotation, frame: &ViewFrame, to_sheet: &dyn Fn([f64; 2]) -> [f64; 2], offset: f64) -> Option<([f64; 2], [f64; 2])> {
    let at = DVec3::from_array(m.at);
    let n = DVec3::from_array(m.normal).normalize_or_zero();
    if n == DVec3::ZERO {
        return None;
    }
    let a = to_sheet(frame.project(at));
    let b = to_sheet(frame.project(at + n * 0.001));
    let mut d = [b[0] - a[0], b[1] - a[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if l < 1e-9 {
        return None;
    }
    if d[1] > 1e-6 * l {
        d = [-d[0], -d[1]];
    }
    Some((a, [a[0] + d[0] / l * offset, a[1] + d[1] / l * offset]))
}

/// The model datums of `sheet` laid out for drawing: hidden ones, those of
/// views showing no model annotations and those whose face is gone are left
/// out; the others get their anchor and box position.
pub fn resolve_model_annotations(sheet: &mut Sheet, geoms: &HashMap<ViewId, Arc<ViewGeometry>>, sources: &Sources, to_sheet: &dyn Fn(ViewId, [f64; 2]) -> [f64; 2]) {
    let show: HashMap<ViewId, bool> = sheet.views.iter().map(|v| (v.id, v.model_annotations)).collect();
    let keys: HashMap<ViewId, views::SourceKey> = sheet.views.iter().filter_map(|v| views::view_source(sheet, v.id).map(|k| (v.id, k.0))).collect();
    sheet.annotations.retain_mut(|a| {
        let Some(m) = &a.model else { return true };
        if m.hidden || !show.get(&m.view).copied().unwrap_or(false) {
            return false;
        }
        let Some(frame) = geoms.get(&m.view).and_then(|g| g.frame) else { return false };
        let Some(src) = keys.get(&m.view).and_then(|k| sources.get(k)) else { return false };
        let Some(ma) = src.model_annotations.iter().find(|x| x.id == m.id) else { return false };
        let view = m.view;
        let Some((anchor, pos)) = model_datum_points(ma, &frame, &|p| to_sheet(view, p), m.offset) else { return false };
        let ModelKind::Datum { letter } = &ma.kind;
        a.kind = AnnotationKind::Datum { anchor, label: letter.clone(), on: None };
        a.position = pos;
        true
    });
}

/// One MBD requirement of the drawing's sources, for the MBD status panel.
#[derive(Clone, Debug, PartialEq)]
pub struct MbdRow {
    pub tab: TabId,
    pub id: u32,
    pub name: String,
    /// Shown in some view of the drawing.
    pub visible: bool,
}

/// The MBD datums of the Part Studios the drawing's views show (all sheets,
/// or `sheet` only), with whether each is visible in some view.
pub fn mbd_rows(drawing: &Drawing, doc: &Document, sheet: Option<usize>) -> Vec<MbdRow> {
    let mut tabs: Vec<TabId> = Vec::new();
    for (si, s) in drawing.sheets.iter().enumerate() {
        if sheet.is_some_and(|x| x != si) {
            continue;
        }
        for v in &s.views {
            if let Some((k, _)) = views::view_source(s, v.id)
                && !tabs.contains(&k.tab)
            {
                tabs.push(k.tab);
            }
        }
    }
    let mut out = Vec::new();
    for tab in tabs {
        let Some(TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else { continue };
        for m in &ps.annotations {
            let ModelKind::Datum { letter } = &m.kind;
            let visible = drawing.sheets.iter().any(|s| {
                s.annotations.iter().any(|a| a.model.as_ref().is_some_and(|r| r.tab == tab && r.id == m.id && !r.hidden && s.view(r.view).is_some_and(|v| v.model_annotations)))
            });
            out.push(MbdRow { tab, id: m.id, name: letter.clone(), visible });
        }
    }
    out
}

impl Drawing {
    /// MBD status › Show: un-hide a model datum where it was (false when its
    /// view shows no model annotations: live's info toast).
    pub fn show_model_annotation(&mut self, tab: TabId, id: u32) -> Option<bool> {
        for s in &mut self.sheets {
            let shows: HashMap<ViewId, bool> = s.views.iter().map(|v| (v.id, v.model_annotations)).collect();
            for a in &mut s.annotations {
                if let Some(m) = &mut a.model
                    && m.tab == tab
                    && m.id == id
                {
                    if !shows.get(&m.view).copied().unwrap_or(false) {
                        return Some(false);
                    }
                    m.hidden = false;
                    return Some(true);
                }
            }
        }
        None
    }
}
