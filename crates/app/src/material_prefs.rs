//! My account › Preferences › Material libraries: the libraries added from
//! the Material panel, with Remove, and [Add material library…] (Help:
//! "Manage my material libraries in account preferences"; live prefs/05).

use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

pub struct MaterialLibrariesView;

impl MaterialLibrariesView {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self
    }
}

/// A Preferences section: the page's 18 px / 500 heading over a 1 px `#333`
/// rule from the content edge, the section's content 15 px in (live prefs/04–05).
fn heading(title: &'static str) -> Div {
    let pal = p();
    v_flex().pl(px(15.)).child(h_flex().ml(px(-15.)).h(px(32.)).pl(px(15.)).border_b_1().border_color(pal.text).child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title)))
}

/// Material libraries table: three 425 px columns (cells 22 px in), the
/// action column right-aligned (live prefs/05).
const LIB_COL: f32 = 425.;
const LIB_RULE: u32 = 0xd0dbe5;

impl Render for MaterialLibrariesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let libs = crate::material_libraries::added();
        // Live's 425 px columns (1919 px window) share the width here, so the
        // Remove buttons stay in view in narrower windows.
        let cell = |t: String| div().flex_1().max_w(px(LIB_COL)).min_w_0().pl(px(22.)).pr(px(8.)).truncate().child(t);
        // Live (prefs/05): every row has a small primary Remove (26 px, 12 px
        // text, 15 px from the right), the Onshape library's too.
        let remove = |id: SharedString, cx: &mut Context<Self>, f: Box<dyn Fn()>| {
            div()
                .id(id)
                .mr(px(15.))
                .h(px(26.))
                .px(px(8.))
                .flex_none()
                .flex()
                .items_center()
                .rounded(px(3.5))
                .bg(rgb(0x1651b0))
                .text_size(px(12.))
                .text_color(gpui_kit::rgb_static(0xffffff))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0x2b65c4)))
                .child("Remove")
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    f();
                    cx.notify();
                }))
        };
        // The header has the same trailing spacer and action column, so
        // the cells line up with their titles at every width.
        let action = || h_flex().flex_none().w(px(90.));
        let row = |name: String, document: String, owner: String, button: Stateful<Div>| h_flex().h(px(48.)).text_size(px(14.)).text_color(pal.text).child(cell(name)).child(cell(document)).child(cell(owner)).child(div().flex_1()).child(action().justify_end().child(button));
        let mut lib_rows: Vec<AnyElement> = Vec::new();
        if crate::material_libraries::onshape_listed() {
            let b = remove("matlib-remove-onshape".into(), cx, Box::new(|| crate::material_libraries::set_onshape_listed(false)));
            lib_rows.push(row("Onshape Material Library".into(), "std.mat".into(), "Onshape".into(), b).into_any_element());
        }
        for (i, l) in libs.iter().enumerate() {
            let b = remove(
                SharedString::from(format!("matlib-remove-{i}")),
                cx,
                Box::new(move || {
                    let mut list = crate::material_libraries::added();
                    if i < list.len() {
                        list.remove(i);
                        crate::material_libraries::set_added(&list);
                    }
                }),
            );
            lib_rows.push(row(l.name.clone(), l.document.clone(), l.owner.clone(), b).into_any_element());
        }
        v_flex()
            .w_full()
            .child(
                heading("Material libraries")
                    .mt(px(38.))
                    .child(
                        v_flex()
                            .mt(px(18.))
                            // Live: the table stops 15 px before the rule's end.
                            .mr(px(15.))
                            .child(h_flex().h(px(44.)).border_b_1().border_color(rgb(LIB_RULE)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(cell("Library".into())).child(cell("Document".into())).child(cell("Owner".into())).child(div().flex_1()).child(action()))
                            .children(lib_rows),
                    )
                    // Live (prefs/05): [Add material library…] under the table
                    // opens the "Add material library" modal.
                    // Live 1902: a primary (blue) button, 34 px under the last row.
                    .child(h_flex().mt(px(34.)).mb(px(38.)).child(crate::os_ui::primary_button("matlib-add-library", "Add material library…").on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                        let view = cx.entity();
                        crate::material_libraries::open(move |_, _, cx| view.update(cx, |_, cx| cx.notify()), window, cx);
                    })))),
            )
    }
}
