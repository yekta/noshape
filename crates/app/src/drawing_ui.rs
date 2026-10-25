//! Drawing editor chrome: toolbar, status bar, inline property panels
//! (note text, title block, scales, labels). Export: `drawing_export`.

use crate::drawing_view::*;
use crate::theme::{p, with_alpha as theme_with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::*;

/// Parse "1:2", "2:1", "1/2", "0.5" into a scale factor.
pub use noshape_ops::drawing::parse_scale;

impl DrawingEditor {
    pub fn render_status(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut parts: Vec<String> = Vec::new();
        if let Some(pr) = self.prompt() {
            parts.push(format!("{}: {pr}", self.tool.name()));
        }
        if self.computing {
            parts.push("computing views…".into());
        }
        // Out of date: Onshape's only cue is the bright yellow Update button.
        if let Some(b) = &self.built
            && !b.dangling.is_empty()
        {
            parts.push(format!("{} dangling annotation(s)", b.dangling.len()));
        }
        // Messages ("Exported …", errors) are Onshape's message bubble at the
        // top center (os_ui::message_bubble), closed with its ×.
        let bubble = self.status.clone().map(|msg| {
            let kind = if msg.contains("failed") || msg.contains("Error") { crate::ui::AlertKind::Error } else { crate::ui::AlertKind::Info };
            let ed = cx.entity();
            div().absolute().top(px(8.)).left_0().right_0().flex().justify_center().child(crate::os_ui::message_bubble("dr-toast", kind, msg, move |_, _, cx| {
                ed.update(cx, |this, cx| {
                    this.status = None;
                    cx.notify();
                })
            }, cx))
        });
        // Onshape has no status bar: the tool prompt and drawing state show as
        // quiet text at the bottom-left of the drawing area.
        let quiet = (!parts.is_empty()).then(|| {
            div()
                .absolute()
                .bottom(px(6.))
                .left(px(10.))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(3.))
                .bg(theme_with_alpha(pal.panel, 0.85))
                .text_size(px(12.))
                .text_color(pal.text_muted)
                .child(parts.join("    "))
        });
        div().absolute().inset_0().children(bubble).children(quiet).into_any_element()
    }

    // ------------------------------------------------------------ panels

    pub fn render_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if matches!(self.panel, Some(Panel::Note { .. })) || self.cell_inplace_box().is_some() {
            return self.render_note_panel(cx);
        }
        let pal = p();
        let panel = self.panel.as_ref()?;
        let (title, rows): (&str, Vec<AnyElement>) = match panel {
            Panel::Note { edit, .. } => {
                let title = if edit.is_some() { "Edit note" } else { "Note" };
                (title, self.note_panel_rows(cx))
            }
            Panel::Datum { .. } | Panel::SurfaceFinish { .. } | Panel::Cell { .. } | Panel::SheetName { .. } | Panel::Rotation { .. } => self.extra_panel_rows(cx)?,
            Panel::BrokenOut { .. } | Panel::CutAngle { .. } | Panel::CellFormat { .. } | Panel::CellResize { .. } | Panel::DatumTarget { .. } => self.more_panel_rows(cx)?,
            Panel::BomCell { .. } => ("BOM cell", self.bom_cell_rows(cx)),
            Panel::TitleBlock { inputs } => (
                "Title block (fields like {part.name}, {sheet.scale} allowed)",
                inputs
                    .iter()
                    .map(|(k, st)| h_flex().gap_2().child(div().w(px(90.)).text_xs().text_color(pal.text_muted).child(k.clone())).child(Input::new(st).xsmall().w(px(220.))).into_any_element())
                    .collect(),
            ),
        };
        Some(
            v_flex()
                .id("dr-panel")
                // Live (qa4's input audit): Esc in Rename sheet (and the other
                // small panels) cancels: the panel closes, nothing applied.
                // In a field Esc is the input's Escape action (dispatched
                // before key listeners, and it reverts an edit), so catch the
                // action too.
                .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                    this.panel = None;
                    window.focus(&this.focus, cx);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    if e.keystroke.key == "escape" {
                        this.panel = None;
                        window.focus(&this.focus, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .p_3()
                .gap_2()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .rounded_md()
                .shadow_md()
                .text_sm()
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .children(rows)
                .child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(Button::new("dr-panel-cancel").label("Cancel").ghost().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.panel = None;
                            cx.notify();
                        })))
                        .child(Button::new("dr-panel-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_panel(cx)))),
                )
                .into_any_element(),
        )
    }

    /// The floating "Note" ✓ ✕ panel just above the note (live drawing/225)
    /// or the table cell (live 741) edited in place.
    fn render_note_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (b, above) = match self.cell_inplace_box() {
            Some(b) => (b, 0.0),
            None => (self.note_inplace_box(cx)?, 27.0),
        };
        let rows = self.note_panel_rows(cx);
        let color_dlg = self.render_note_color(cx);
        // Above the text (and the note's ruler).
        let bottom = (self.size_px()[1] - (b.text_top.min(b.top) - above) + 6.0) as f32;
        let left = (b.left - 3.0).max(4.0) as f32;
        Some(
            v_flex()
                .id("dr-note-panel")
                .absolute()
                .left(px(left))
                .bottom(px(bottom))
                // Live: 394 px (dom/drawing-note-edit-dialog.txt).
                .w(px(394.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(gpui_kit::rgb(0x333333))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Note"))
                        .child(div().id("note-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_panel(cx))))
                        .child(div().id("note-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.discard_inplace(cx)))),
                )
                .child(v_flex().p(px(4.)).text_size(px(12.)).children(rows))
                .children(color_dlg)
                .into_any_element(),
        )
    }

    pub fn textarea(&mut self, value: &str, rows: usize, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextareaState> {
        let st = cx.new(|cx| TextareaState::new(window, cx).rows(rows).default_value(value.to_string()));
        st.update(cx, |s, cx| s.focus(window, cx));
        // Opened from a click on the canvas: the canvas takes the focus back
        // while that click is handled, so focus the editor again after it.
        let again = st.downgrade();
        window.defer(cx, move |window, cx| {
            if let Some(st) = again.upgrade() {
                st.update(cx, |s, cx| s.focus(window, cx));
            }
        });
        st
    }

    /// A drawing panel field. Onshape's drawing editor uses Wt widgets, not
    /// the Part Studio's fields (live behaviours.md "Input behaviour outside
    /// feature dialogs"): a click puts the caret at the click point (nothing
    /// selected) and Esc never reverts the text (it passes to the panel,
    /// which ignores it or closes, e.g. the Note editor).
    pub fn input(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(value.to_string()));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.apply_panel(cx);
            }
        });
        self._subs.push(sub);
        st
    }

    pub fn focus_input(st: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        // Opened from a menu or a double click: focus again after their own
        // focus handling (else Esc and typing go to the canvas).
        let again = st.downgrade();
        window.defer(cx, move |window, cx| {
            if let Some(st) = again.upgrade() {
                st.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
            }
        });
    }

    pub fn open_note_panel(&mut self, edit: Option<u64>, pos: [f64; 2], window: &mut Window, cx: &mut Context<Self>) {
        let note = edit
            .and_then(|id| self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id))
            .and_then(|a| if let AnnotationKind::Note { text, height_mm, style } = &a.kind { Some((text.clone(), *height_mm, style.clone())) } else { None });
        // New notes: Drawing properties › Annotations › Notes text height, else All annotations'.
        let default_h = self.drawing.props.ann_type(noshape_model::drawing::sheet::AnnType::Notes).height.unwrap_or(self.drawing.props.ann_text_height);
        let (text, h, mut style) = note.unwrap_or((String::new(), default_h, Default::default()));
        if edit.is_none()
            && let Some(l) = self.note_leader.take()
        {
            style.leaders.push(l);
        }
        let input = self.textarea(&text, 5, window, cx);
        // Tab moves the text to the next tab stop of the ruler.
        input.update(cx, |s, cx| s.set_tab_size(gpui_kit::component::input::TabSize { tab_size: 4, hard_tabs: true }, cx));
        let height = self.input(&crate::drawing_note::note_height_text(h, self.drawing.units_mm), window, cx);
        self.panel = Some(Panel::Note { input, edit, pos, height, style, field: None });
        cx.notify();
    }

    /// A title block field's cell opens the Note editor in place, its text
    /// the field's (live qa4 round 5 s11 (4): the cell is a note, editable
    /// while the formats are locked).
    pub fn open_title_field(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = self.built.as_ref() else { return };
        let target = noshape_model::drawing::build::PickTarget::TitleField(key.clone());
        let Some(&(pick, r)) = b.title_cells.iter().find(|(p, _)| b.picks.get(*p as usize) == Some(&target)) else { return };
        let h = b.geometry.texts.iter().find(|t| t.pick == pick).map(|t| t.height).unwrap_or(self.drawing.props.ann_text_height);
        let text = self.drawing.sheets[self.sheet].title_field(&key).unwrap_or("").to_string();
        let input = self.textarea(&text, 5, window, cx);
        input.update(cx, |s, cx| s.set_tab_size(gpui_kit::component::input::TabSize { tab_size: 4, hard_tabs: true }, cx));
        let height = self.input(&crate::drawing_note::note_height_text(h, self.drawing.units_mm), window, cx);
        let pos = [r[0] + 1.0, r[3] - 1.0];
        self.panel = Some(Panel::Note { input, edit: None, pos, height, style: Default::default(), field: Some(key) });
        cx.notify();
    }

    pub fn open_title_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fields = self.drawing.sheets[self.sheet].title_block.fields.clone();
        let inputs: Vec<(String, Entity<InputState>)> = fields.iter().map(|(k, v)| (k.clone(), self.input(v, window, cx))).collect();
        if let Some((_, first)) = inputs.first() {
            Self::focus_input(first, window, cx);
        }
        self.panel = Some(Panel::TitleBlock { inputs });
        cx.notify();
    }

    pub fn apply_panel(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.panel.take() else { return };
        match panel {
            Panel::Note { input, edit, pos, height, style, field } => {
                let text = input.read(cx).value().to_string();
                let h = crate::drawing_note::parse_note_height(&height.read(cx).value(), self.drawing.units_mm);
                if let Some(key) = field {
                    self.edit_sheet("Edit title block", cx, |s| noshape_scene::drawing_tools::set_title_field(s, &key, text.trim_end()));
                } else if text.trim().is_empty() {
                    if let Some(id) = edit {
                        self.edit_sheet("Delete note", cx, |s| s.annotations.retain(|a| a.id != id));
                    }
                } else if let Some(id) = edit {
                    self.edit_sheet("Edit note", cx, |s| {
                        if let Some(AnnotationKind::Note { text: t, height_mm, style: st }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                            *t = text;
                            *st = style;
                            if let Some(h) = h {
                                *height_mm = h;
                            }
                        }
                    });
                } else {
                    // New notes take Drawing properties › Notes text height;
                    // a typed height becomes the new Notes default.
                    let sheet = self.sheet;
                    if let Some(note) = noshape_scene::drawing_tools::new_note(&mut self.drawing, sheet, text, h, style, pos) {
                        self.edit_sheet("Note", cx, |s| s.annotations.push(note));
                    }
                    self.tool = Tool::Select;
                }
            }
            Panel::TitleBlock { inputs } => {
                let values: Vec<(String, String)> = inputs.iter().map(|(k, st)| (k.clone(), st.read(cx).value().to_string())).collect();
                self.edit_sheet("Edit title block", cx, |s| s.title_block.fields = values);
            }
            other => self.apply_extra_panel(other, cx),
        }
        self.update_preview(cx);
        cx.notify();
    }

}

#[cfg(test)]
mod tests {
    use super::parse_scale;

    #[test]
    fn scales() {
        assert_eq!(parse_scale("1:2"), Some(0.5));
        assert_eq!(parse_scale("2:1"), Some(2.0));
        assert_eq!(parse_scale("0.25"), Some(0.25));
        assert_eq!(parse_scale("x"), None);
    }
}
