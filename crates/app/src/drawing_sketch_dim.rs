//! Dimensions on drawing sketch geometry (SK7; live behaviours "Drawing sketch
//! dimensions drive geometry"): with the Dimension tool a click on a sketch
//! line or circle picks it, the linear / diameter dimension follows the
//! pointer and the placing click opens the in-place value box straight away
//! (full-precision value, selected, in a thin black frame, with the small
//! "± 0.1" button above right). Enter drives the geometry; double-clicking
//! the dimension reopens the box.

use crate::drawing_view::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::*;
use noshape_model::drawing::Annotation;

/// The open value box of a sketch dimension.
pub struct SketchDimEdit {
    pub ann: u64,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

impl DrawingEditor {
    /// Dimension tool, nothing picked: a click on a sketch entity picks it;
    /// with one picked, the click places the dimension. True when used.
    pub fn sketch_dim_click(&mut self, c: [f64; 2], window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(e) = self.sketch_dim_pick.take() {
            let si = self.sheet;
            let mut placed = None;
            self.edit("Dimension", cx, |d| placed = d.sheets[si].add_sketch_dimension(e, c));
            if let Some(id) = placed {
                self.open_sketch_dim_box(id, window, cx);
            }
            return true;
        }
        let Some(e) = self.sketch_entity_at() else { return false };
        if self.current_sheet().clone().add_sketch_dimension(e.0, c).is_none() {
            return false;
        }
        self.sketch_dim_pick = Some(e.0);
        cx.notify();
        true
    }

    /// The dimension following the pointer after a sketch entity was picked.
    pub fn sketch_dim_preview(&self) -> Option<Annotation> {
        let e = self.sketch_dim_pick?;
        let mut s = self.current_sheet().clone();
        let id = s.add_sketch_dimension(e, self.cursor)?;
        s.annotations.into_iter().find(|a| a.id == id)
    }

    /// The value text of a length in the drawing's units at full precision
    /// (live: "1.002974", ANSI without the leading zero: ".521026").
    fn sketch_dim_text(&self, mm: f64) -> String {
        let v = if self.drawing.units_mm { mm } else { mm / 25.4 };
        let t = format!("{v:.6}");
        if !self.drawing.units_mm && t.starts_with("0.") { t[1..].to_string() } else { t }
    }

    /// Open the in-place value box on a sketch dimension.
    pub fn open_sketch_dim_box(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = self.current_sheet().sketch_dimension_value(id) else { return };
        let text = self.sketch_dim_text(v);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        // Focus and select after the (double) click's own focus handling.
        let first = input.clone();
        window.defer(cx, move |window, cx| {
            first.update(cx, |st, cx| {
                st.focus(window, cx);
                st.select_all(window, cx);
            })
        });
        let sub = cx.subscribe(&input, move |this: &mut Self, st, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = st.read(cx).value().to_string();
                this.accept_sketch_dim(id, &text, cx);
            }
        });
        self.sketch_dim_edit = Some(SketchDimEdit { ann: id, input, _sub: sub });
        cx.notify();
    }

    /// Enter: the typed value (drawing units unless stated, sign ignored).
    fn accept_sketch_dim(&mut self, id: u64, text: &str, cx: &mut Context<Self>) {
        self.sketch_dim_edit = None;
        let prefs = noshape_units::UnitPrefs { length: if self.drawing.units_mm { noshape_units::LengthUnit::Millimeter } else { noshape_units::LengthUnit::Inch }, ..Default::default() };
        let Ok(q) = noshape_units::evaluate_as(text.trim(), noshape_units::Dim::LENGTH, &prefs, &noshape_units::EmptyScope) else {
            cx.notify();
            return;
        };
        // Meters → sheet mm.
        let mm = q.value * 1000.0;
        let si = self.sheet;
        self.edit("Edit dimension", cx, |d| {
            d.sheets[si].set_sketch_dimension(id, mm);
        });
    }

    /// The value box, at the dimension's text (live drawing/915).
    pub fn render_sketch_dim_box(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let edit = self.sketch_dim_edit.as_ref()?;
        let a = self.current_sheet().annotations.iter().find(|a| a.id == edit.ann)?;
        let p = self.sheet_to_local(a.position);
        let (id, input) = (edit.ann, edit.input.clone());
        Some(
            div()
                .absolute()
                .left(px(p[0] as f32 - 40.))
                .top(px(p[1] as f32 - 16.))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "escape" {
                        this.sketch_dim_edit = None;
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(div().w(px(80.)).h(px(22.)).bg(rgb(0xffffff)).border_1().border_color(rgb(0x000000)).child(Input::new(&input).appearance(false).text_size(px(13.33)).text_color(rgb(0x000000))))
                // The "± 0.1" button (tolerance / precision): the dimension palette.
                .child(
                    div()
                        .id("sk-dim-tol")
                        .absolute()
                        .left(px(84.))
                        .top(px(-20.))
                        .px(px(4.))
                        .h(px(18.))
                        .flex()
                        .items_center()
                        .bg(rgb(0xffffff))
                        .border_1()
                        .border_color(rgb(0xcccccc))
                        .text_size(px(11.))
                        .text_color(rgb(0x333333))
                        .cursor_pointer()
                        .child("± 0.1")
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.sketch_dim_edit = None;
                            this.open_dim_palette(id, window, cx);
                        })),
                )
                .into_any_element(),
        )
    }
}
