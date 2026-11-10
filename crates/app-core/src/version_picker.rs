//! The Reference manager's Version graph (live, behaviours "Change to version…" 295–296): the panel's body swapped for
//! ‹ back, the document's name over "⎇ <picked>", the legend, then Main, the versions newest first and Start, the
//! picked row light blue. A click on a row picks it.

use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

type Pick = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// `rows`: Main first, then the versions and Start; `picked` indexes them. Rows are `menu:<label>` targets and the
/// open list is noted as live's reader reads it (kind "list").
pub fn version_picker(name: &str, rows: &[String], picked: usize, on_back: impl Fn(&mut Window, &mut App) + 'static, on_pick: impl Fn(usize, &mut Window, &mut App) + 'static, cx: &App) -> AnyElement {
    crate::ui_script::note_list(Some(serde_json::json!({ "kind": "list", "rows": rows, "highlighted": picked, "disabled": [] })));
    let pal = p();
    let on_pick: Pick = std::rc::Rc::new(on_pick);
    let legend = |icon: &str, text: &'static str| h_flex().gap(px(4.)).child(crate::assets::icon(icon, 14., cx)).child(text);
    let header = h_flex()
        .h(px(44.))
        .gap(px(6.))
        .px(px(4.))
        .border_b_1()
        .border_color(rgb(0xeaeaea))
        .child(
            div()
                .id("rm-picker-back")
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0xeeeeee)))
                .text_size(px(18.))
                .text_color(pal.text)
                .child("‹")
                .on_click(move |_, window, cx| on_back(window, cx)),
        )
        .child(
            v_flex()
                .min_w_0()
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).truncate().child(crate::ui::user_text(name.to_string())))
                .child(h_flex().gap(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child(crate::assets::icon("version-graph", 14., cx)).child(rows.get(picked).cloned().unwrap_or_default())),
        );
    let mut list = v_flex().id("rm-picker-rows").flex_1().min_h_0().overflow_y_scroll();
    for (k, label) in rows.iter().enumerate() {
        let icon = if k == 0 { "legend-workspace" } else { "legend-version" };
        let pick = on_pick.clone();
        let l = label.clone();
        list = list.child(
            h_flex()
                .id(SharedString::from(format!("rm-picker-{k}")))
                .debug_selector(move || format!("rm-picker-{k}"))
                .map(|d| crate::ui_script::mark(d, move || format!("menu:{l}")))
                .h(px(30.))
                .px(px(8.))
                .gap(px(8.))
                .cursor_pointer()
                .text_size(px(13.))
                .text_color(pal.text)
                .when(k == picked, |d| d.bg(rgb(0xb2ddf6)))
                .when(k != picked, |d| d.hover(|d| d.bg(rgb(0xf0f0f0))))
                .child(crate::assets::icon(icon, 16., cx))
                .child(crate::ui::user_text(label.clone()))
                .on_click(move |_, window, cx| pick(k, window, cx)),
        );
    }
    v_flex()
        .size_full()
        .child(header)
        .child(h_flex().h(px(26.)).px(px(8.)).gap(px(12.)).text_size(px(11.)).text_color(pal.text_muted).child(legend("legend-workspace", "Workspace")).child(legend("legend-version", "Version")))
        .child(list)
        .into_any_element()
}
