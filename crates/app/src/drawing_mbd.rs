//! MBD datums from the Part Studio in a drawing (live behaviours "MBD datums
//! in drawings"): dragged only along their face normal, deleted = hidden, and
//! the MBD status panel's rows, message, badge and "Show".

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::mbd::{self, ModelRef};
use noshape_model::drawing::views::{self, ViewFrame};
use noshape_model::drawing::Annotation;

impl DrawingEditor {
    /// A model datum's anchor and unit direction on the sheet now.
    fn model_datum_axis(&self, m: &ModelRef) -> Option<([f64; 2], [f64; 2])> {
        let built = self.built.as_ref()?;
        let pl = built.placements.iter().find(|p| p.id == m.view)?;
        let frame: ViewFrame = self.geoms.get(&m.view)?.frame?;
        let sheet = self.current_sheet();
        let (key, _) = views::view_source(sheet, m.view)?;
        let ma = self.sources.get(&key)?.model_annotations.iter().find(|x| x.id == m.id)?.clone();
        let (a, b) = mbd::model_datum_points(&ma, &frame, &|p| pl.to_sheet(p), 1.0)?;
        Some((a, [b[0] - a[0], b[1] - a[1]]))
    }

    /// Dragging a model datum: the box moves only along the face normal
    /// (live drawing/1075–1076); the stem follows.
    pub fn drag_model_datum(&mut self, orig: &Annotation, start: [f64; 2], cx: &mut Context<Self>) {
        let Some(m) = orig.model.clone() else { return };
        let Some((_, dir)) = self.model_datum_axis(&m) else { return };
        let d = [self.cursor[0] - start[0], self.cursor[1] - start[1]];
        let offset = (m.offset + d[0] * dir[0] + d[1] * dir[1]).max(3.0);
        let mut dr = self.drawing.clone();
        if let Some(s) = dr.sheets.get_mut(self.sheet)
            && let Some(x) = s.annotations.iter_mut().find(|x| x.id == orig.id)
            && let Some(r) = &mut x.model
        {
            r.offset = offset;
        }
        self.set_preview(Some(dr), cx);
    }

    /// MBD status › a row's "Show": back where it was, unless its view shows
    /// no model annotations (live's info toast).
    pub fn show_model_annotation(&mut self, tab: noshape_model::document::TabId, id: u32, cx: &mut Context<Self>) {
        let mut d = self.drawing.clone();
        match d.show_model_annotation(tab, id) {
            Some(true) => self.commit("Show", d, cx),
            Some(false) => {
                self.status = Some("Annotations from the model have been disabled on this view.".into());
                cx.notify();
            }
            None => {}
        }
    }

    /// The MBD status rows for the panel's Model choice (0 Current sheet,
    /// 1 All models, else the source `tab`'s own).
    pub fn mbd_status_rows(&self, model: usize, tab: Option<noshape_model::document::TabId>, cx: &App) -> Vec<mbd::MbdRow> {
        let doc = &self.doc.read(cx).session.doc;
        match model {
            0 => mbd::mbd_rows(&self.drawing, doc, Some(self.sheet)),
            1 => mbd::mbd_rows(&self.drawing, doc, None),
            _ => mbd::mbd_rows(&self.drawing, doc, None).into_iter().filter(|r| Some(r.tab) == tab).collect(),
        }
    }

    /// The MBD status icon's badge (live drawing/1079, 1081): a green ✓ when
    /// every requirement is shown, a red "!" when some aren't; none without
    /// MBD requirements.
    pub fn mbd_badge(&self, cx: &App) -> Option<AnyElement> {
        let rows = mbd::mbd_rows(&self.drawing, &self.doc.read(cx).session.doc, None);
        if rows.is_empty() {
            return None;
        }
        let ok = rows.iter().all(|r| r.visible);
        Some(
            div()
                .absolute()
                .right(px(2.))
                .bottom(px(2.))
                .size(px(11.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(if ok { rgb(0x2e9e44) } else { rgb(0xd9342b) })
                .text_size(px(9.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(if ok { "✓" } else { "!" })
                .into_any_element(),
        )
    }
}
