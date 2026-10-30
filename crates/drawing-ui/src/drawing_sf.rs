//! The Surface finish dialog (live request 1628): Type ▾ (the six types),
//! ☐ All around (not for the ISO types), ☐ ( ), three cells left and the
//! right cells (three, + adds one) beside a large symbol, Direction lay ▾.
//! It starts empty each time, as live's.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::{SfKind, SfLay, SfSpec};

impl DrawingEditor {
    /// The panel for a new symbol (`edit` None) or an existing one.
    pub fn open_surface_finish_panel(&mut self, edit: Option<u64>, pos: [f64; 2], value: &str, spec: &SfSpec, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input(value, window, cx);
        Self::focus_input(&input, window, cx);
        let left = [self.input(&spec.left[0], window, cx), self.input(&spec.left[1], window, cx), self.input(&spec.left[2], window, cx)];
        let rows = spec.rows();
        let right = (1..rows).map(|i| self.input(spec.right.get(i - 1).map(String::as_str).unwrap_or(""), window, cx)).collect();
        let spec = SfSpec { left: Default::default(), right: Vec::new(), ..spec.clone() };
        self.panel = Some(Panel::SurfaceFinish { edit, pos, input, placed: Vec::new(), spec, left, right });
        cx.notify();
    }

    /// The tool opens the dialog, empty (live request 1628).
    pub fn sync_sf_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.tool, Tool::SurfaceFinish) && !matches!(self.panel, Some(Panel::SurfaceFinish { .. })) {
            self.open_surface_finish_panel(None, self.cursor, noshape_scene::drawing_tools::SURFACE_FINISH_DEFAULT, &SfSpec::default(), window, cx);
        }
    }

    /// The panel's symbol: the first right cell and the rest.
    pub fn sf_panel_values(&self, cx: &App) -> Option<(String, SfSpec)> {
        let Some(Panel::SurfaceFinish { input, spec, left, right, .. }) = &self.panel else { return None };
        let val = |e: &Entity<InputState>| e.read(cx).value().to_string();
        let mut right: Vec<String> = right.iter().map(val).collect();
        // The dialog's two cells kept as rows even when empty; trailing
        // empty ones past them dropped.
        while right.len() > 2 && right.last().is_some_and(String::is_empty) {
            right.pop();
        }
        if right.iter().all(String::is_empty) {
            right.clear();
        }
        let spec = SfSpec { left: [val(&left[0]), val(&left[1]), val(&left[2])], right, all_around: spec.all_around && !spec.kind.iso(), ..spec.clone() };
        Some((val(input), spec))
    }

    fn sf_spec_mut(&mut self) -> Option<&mut SfSpec> {
        match &mut self.panel {
            Some(Panel::SurfaceFinish { spec, .. }) => Some(spec),
            _ => None,
        }
    }

    pub fn sf_panel_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let ed = cx.entity();
        let pal = p();
        let Some(Panel::SurfaceFinish { input, spec, left, right, .. }) = &self.panel else { return Vec::new() };
        let (kind, all_around, parens, lay) = (spec.kind, spec.all_around, spec.parens, spec.lay);
        let types: Vec<MenuEntry> = SfKind::ALL
            .iter()
            .map(|k| {
                let k = *k;
                MenuEntry::check(k.name(), k == kind, move |t, _, cx| {
                    if let Some(s) = t.sf_spec_mut() {
                        s.kind = k;
                    }
                    cx.notify();
                })
            })
            .collect();
        let lays: Vec<MenuEntry> = std::iter::once(MenuEntry::check("No direction lay", lay.is_none(), |t, _, cx| {
            if let Some(s) = t.sf_spec_mut() {
                s.lay = None;
            }
            cx.notify();
        }))
        .chain(SfLay::ALL.iter().map(|l| {
            let l = *l;
            MenuEntry::check(format!("{}  {}", l.glyph(), l.name()), lay == Some(l), move |t, _, cx| {
                if let Some(s) = t.sf_spec_mut() {
                    s.lay = Some(l);
                }
                cx.notify();
            })
        }))
        .collect();
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut SfSpec, bool), cx: &mut Context<Self>| {
            crate::drawing_chrome::DrCheckbox::new(id)
                .label(label)
                .checked(on)
                .on_change(cx.listener(move |this, v: &bool, _, cx| {
                    if let Some(s) = this.sf_spec_mut() {
                        f(s, *v);
                    }
                    cx.notify();
                }))
                .into_any_element()
        };
        let ed2 = ed.clone();
        let mut top = h_flex()
            .gap_2()
            .items_center()
            .child(div().text_xs().text_color(pal.text_muted).child("Type"))
            .child(Button::new("dr-sf-type").label(kind.name()).small().outline().dropdown_caret(true).os_dropdown_menu("dr-sf-type-dd", false, move |m, w, cx| fill_os_menu(m, &types, &ed, w, cx)));
        // Live: no All around for the ISO types.
        if !kind.iso() {
            top = top.child(check("dr-sf-around", "All around", all_around, |s, v| s.all_around = v, cx));
        }
        top = top.child(check("dr-sf-parens", "( )", parens, |s, v| s.parens = v, cx));
        let cell = |e: &Entity<InputState>| Input::new(e).text_size(px(13.33)).xsmall().w(px(80.)).into_any_element();
        let cells = h_flex()
            .gap_2()
            .items_center()
            .child(v_flex().gap_1().children(left.iter().map(cell)))
            .child(div().text_size(px(40.)).text_color(pal.text).child("√"))
            .child(v_flex().gap_1().child(cell(input)).children(right.iter().map(cell)));
        let bottom = h_flex()
            .gap_2()
            .child(Button::new("dr-sf-lay").label(lay.map(|l| l.glyph()).unwrap_or(" ")).small().outline().dropdown_caret(true).tooltip("Direction lay").os_dropdown_menu("dr-sf-lay-dd", false, move |m, w, cx| fill_os_menu(m, &lays, &ed2, w, cx)))
            .child(Button::new("dr-sf-add").label("+").small().outline().tooltip("Add row").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                let e = this.input("", window, cx);
                if let Some(Panel::SurfaceFinish { right, .. }) = &mut this.panel {
                    right.push(e);
                }
                cx.notify();
            })));
        vec![top.into_any_element(), cells.into_any_element(), bottom.into_any_element()]
    }
}
