//! Drawing editor: Onshape's Inspection item tool (right end of the
//! toolbar, with the "Hide inspection items" toggle): Manual (click
//! annotations; numbers increase) or Automatic (Scope: drawing, sheet or
//! selected view; Preview), the border shape, Reorder inspection items,
//! Remove (all) inspection item(s), and the "Inspection list" CSV.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::inspection::Scope;
use noshape_model::drawing::*;

/// Inspection item dialog state.
pub struct InspectionDlg {
    pub automatic: bool,
    /// Automatic scope: 0 drawing, 1 current sheet, 2 selected view.
    pub scope: u8,
    /// Reorder: the next number to assign (typed in the Number field).
    pub reorder: Option<Entity<InputState>>,
    /// Edit (double-click / context menu): the item and its Number field.
    pub edit: Option<(u64, Entity<InputState>)>,
    /// Manual: the hovered annotation previewed with the next number.
    pub hover: Option<u64>,
}

impl DrawingEditor {
    pub fn open_inspection(&mut self, cx: &mut Context<Self>) {
        self.inspection = match self.inspection {
            Some(_) => None,
            None => Some(InspectionDlg { automatic: false, scope: 0, reorder: None, edit: None, hover: None }),
        };
        self.set_preview(None, cx);
        self.tool = Tool::Select;
        self.status = self.inspection.as_ref().map(|_| "Inspection item: click a dimension or annotation".into());
        cx.notify();
    }

    /// Context menu › Reorder inspection items: type a number, click items.
    pub fn start_reorder_inspection(&mut self, from: u32, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(from.to_string()));
        self.inspection = Some(InspectionDlg { automatic: false, scope: 0, reorder: Some(st), edit: None, hover: None });
        self.status = Some("Reorder inspection items: click the items in their new order".into());
        cx.notify();
    }

    /// Edit an item (double-click it, or Edit in its context menu): the
    /// dialog with its Number; the checkmark applies.
    pub fn edit_inspection(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(n) = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).and_then(|a| a.inspection) else { return };
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(n.to_string()));
        self.inspection = Some(InspectionDlg { automatic: false, scope: 0, reorder: None, edit: Some((id, st)), hover: None });
        self.selection = vec![Sel::Annotation(id)];
        cx.notify();
    }

    /// The annotation whose inspection bubble is under pick `pick`.
    pub fn inspection_at(&self, pick: Option<u32>) -> Option<u64> {
        match self.built.as_ref()?.picks.get(pick? as usize)? {
            PickTarget::Inspection(id) => Some(*id),
            _ => None,
        }
    }

    /// End of a bubble drag: the number moves to the annotation under the
    /// cursor (Onshape: drag an inspection item onto another annotation).
    pub fn drop_inspection(&mut self, from: u64, cx: &mut Context<Self>) {
        self.update_hover(cx);
        let Some(Sel::Annotation(to)) = self.sel_at(self.hover, self.cursor) else { return };
        if to == from {
            return;
        }
        self.edit_sheet("Move inspection item", cx, |s| {
            let n = s.annotations.iter_mut().find(|a| a.id == from).and_then(|a| a.inspection.take());
            if let Some(a) = s.annotations.iter_mut().find(|a| a.id == to) {
                a.inspection = n;
            }
        });
        self.selection = vec![Sel::Annotation(to)];
    }

    /// Manual mode: hovering an annotation previews its inspection item.
    pub fn inspection_hover(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.inspection else { return };
        if d.automatic || d.edit.is_some() {
            return;
        }
        let reorder = d.reorder.as_ref().map(|st| st.read(cx).value().trim().parse::<u32>().unwrap_or(1));
        let prev = d.hover;
        let id = match self.sel_at(self.hover, self.cursor) {
            Some(Sel::Annotation(id)) if reorder.is_some() || self.drawing.sheets[self.sheet].annotations.iter().any(|a| a.id == id && a.inspection.is_none()) => Some(id),
            _ => None,
        };
        if id == prev {
            return;
        }
        if let Some(d) = &mut self.inspection {
            d.hover = id;
        }
        let preview = id.map(|id| {
            let n = reorder.unwrap_or_else(|| self.drawing.next_inspection_number());
            let mut dr = self.drawing.clone();
            if let Some(a) = dr.sheets[self.sheet].annotations.iter_mut().find(|a| a.id == id) {
                a.inspection = Some(n);
            }
            dr
        });
        self.set_preview(preview, cx);
    }

    /// Esc: close without applying (drops the hover preview).
    pub fn cancel_inspection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.inspection.take() else { return false };
        if d.hover.is_some() {
            self.set_preview(None, cx);
        }
        true
    }

    /// Checkmark: apply an edited number, close the dialog.
    fn close_inspection(&mut self, cx: &mut Context<Self>) {
        if let Some(InspectionDlg { edit: Some((id, st)), .. }) = self.inspection.take()
            && let Ok(n) = st.read(cx).value().trim().parse::<u32>()
        {
            self.edit_sheet("Edit inspection item", cx, |s| {
                if let Some(a) = s.annotations.iter_mut().find(|a| a.id == id) {
                    a.inspection = Some(n);
                }
            });
        }
        self.status = None;
        self.set_preview(None, cx);
        cx.notify();
    }

    /// Click while the dialog is open (Manual / Reorder). True when used.
    pub fn inspection_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.inspection else { return false };
        if d.automatic || d.edit.is_some() {
            return d.automatic;
        }
        let reorder = d.reorder.clone();
        self.update_hover(cx);
        let Some(Sel::Annotation(id)) = self.sel_at(self.hover, self.cursor) else { return true };
        let number = match &reorder {
            Some(st) => st.read(cx).value().trim().parse::<u32>().unwrap_or(1),
            None => self.drawing.next_inspection_number(),
        };
        self.preview = None;
        if let Some(d) = &mut self.inspection {
            d.hover = None;
        }
        self.edit_sheet("Inspection item", cx, |s| {
            if let Some(a) = s.annotations.iter_mut().find(|a| a.id == id) {
                a.inspection = Some(number);
            }
        });
        if let Some(st) = reorder {
            // The Number field increments after each item (Onshape).
            st.update(cx, |s, cx| s.set_value((number + 1).to_string(), window, cx));
        }
        true
    }

    /// Automatic: number the scope (Preview applies; Undo reverts).
    fn inspection_preview(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.inspection else { return };
        let view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
        let scope = match (d.scope, view) {
            (1, _) => Scope::Sheet(self.sheet),
            (2, Some(v)) => Scope::View(self.sheet, v),
            (2, None) => {
                self.status = Some("Select a view for the scope".into());
                cx.notify();
                return;
            }
            _ => Scope::Drawing,
        };
        let mut n = 0;
        self.edit("Inspection items", cx, |dr| n = dr.auto_inspection(scope));
        self.status = Some(format!("{n} inspection items added").into());
        cx.notify();
    }

    pub fn toggle_hide_inspection(&mut self, cx: &mut Context<Self>) {
        let hide = !self.drawing.props.hide_inspection;
        self.edit(if hide { "Hide inspection items" } else { "Show inspection items" }, cx, |d| d.props.hide_inspection = hide);
    }

    /// Context menu entries of an annotation with an inspection item.
    pub fn inspection_entries(&self, a: &Annotation) -> Vec<MenuEntry> {
        let Some(n) = a.inspection else { return vec![] };
        let id = a.id;
        vec![
            MenuEntry::item("Edit", move |t, w, cx| t.edit_inspection(id, w, cx)),
            MenuEntry::item("Reorder inspection items", move |t, w, cx| t.start_reorder_inspection(n, w, cx)),
            MenuEntry::item("Remove inspection item", move |t, _, cx| {
                t.edit_sheet("Remove inspection item", cx, |s| {
                    if let Some(x) = s.annotations.iter_mut().find(|x| x.id == id) {
                        x.inspection = None;
                    }
                })
            }),
            MenuEntry::item("Remove all inspection items", |t, _, cx| t.edit("Remove all inspection items", cx, |d| d.remove_all_inspection())),
            MenuEntry::Separator,
        ]
    }

    /// Inspection rows with hole callout texts from the model; `all` = the
    /// Drawing definition list.
    pub fn inspection_rows(&self, all: bool) -> Vec<noshape_model::drawing::inspection::InspectionRow> {
        let (d, sources) = (&self.drawing, &self.sources);
        self.drawing.inspection_rows_with(&|si, a| noshape_model::drawing::build::hole_callout_lines(d, &d.sheets[si], sources, a), all)
    }

    /// Export › Inspection list / Drawing definition list (CSV).
    pub fn export_inspection_list(&mut self, all: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into());
        let csv = noshape_model::drawing::inspection::rows_csv(&self.inspection_rows(all));
        let this = cx.entity().downgrade();
        let file = format!("{name} {}.csv", if all { "drawing definition list" } else { "inspection list" });
        crate::files::request_save_path("Export inspection list", file, "csv", window, cx, move |path, cx| {
            let msg = match std::fs::write(&path, csv) {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            this.update(cx, |this, cx| {
                this.status = Some(msg.into());
                cx.notify();
            })
            .ok();
        });
    }

    pub fn render_inspection_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.inspection.as_ref()?;
        let pal = p();
        let (automatic, scope) = (d.automatic, d.scope);
        let reorder = d.reorder.clone();
        let edit = d.edit.as_ref().map(|e| e.1.clone());
        let tab = |id: &'static str, label: &'static str, on: bool, value: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(9.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |e| e.border_b_2().border_color(pal.accent).text_color(pal.accent).font_weight(FontWeight::MEDIUM))
                .when(!on, |e| e.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.inspection {
                        d.automatic = value;
                        d.reorder = None;
                    }
                    cx.notify();
                }))
        };
        let ed = cx.entity();
        let shape = self.drawing.props.inspection_shape;
        let ed2 = ed.clone();
        let shapes = Button::new("insp-shape").label(shape.name()).ghost().small().w(px(120.)).dropdown_caret(true).os_dropdown_menu("insp-shape-dd", true, move |mut m, _, _| {
            for s in InspectionShape::ALL {
                let e = ed2.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(s.name()).checked(s == shape).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit("Inspection border shape", cx, |d| d.props.inspection_shape = s))));
            }
            m
        });
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(30.)).px(px(6.)).child(div().w(px(90.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        let mut body = v_flex().p(px(3.));
        if let Some(st) = reorder.as_ref().or(edit.as_ref()) {
            body = body.child(row("Number", Input::new(st).small().into_any_element()));
        } else {
            body = body.child(h_flex().child(tab("insp-manual", "Manual", !automatic, false, cx)).child(tab("insp-auto", "Automatic", automatic, true, cx)));
            body = body.child(row("Border shape", shapes.into_any_element()));
            if automatic {
                let labels = ["Drawing", "Current sheet", "Selected view"];
                let ed3 = ed.clone();
                let scope_btn = Button::new("insp-scope").label(labels[scope as usize]).ghost().small().w(px(120.)).dropdown_caret(true).os_dropdown_menu("insp-scope-dd", true, move |mut m, _, _| {
                    for (i, l) in labels.iter().enumerate() {
                        let e = ed3.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(*l).checked(i as u8 == scope).on_click(move |_, _, cx| {
                            e.update(cx, |t, cx| {
                                if let Some(d) = &mut t.inspection {
                                    d.scope = i as u8;
                                }
                                cx.notify();
                            })
                        }));
                    }
                    m
                });
                body = body.child(row("Scope", scope_btn.into_any_element())).child(
                    div().px(px(6.)).py(px(4.)).child(Button::new("insp-preview").label("Preview").outline().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.inspection_preview(cx)))),
                );
            } else {
                body = body.child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child(format!("Next number: {}", self.drawing.next_inspection_number())));
            }
        }
        Some(
            v_flex()
                .id("dr-inspection-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(240.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if reorder.is_some() { "Reorder inspection items" } else { "Inspection item" }))
                        .child(
                            div()
                                .id("insp-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_inspection(cx))),
                        ),
                )
                .child(body)
                .into_any_element(),
        )
    }
}
