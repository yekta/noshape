//! The bar that replaces a tab's toolbar while a version or a history change is viewed (live navigation/301,
//! docver-1121): "Versions are view only. Viewing V1." with the tab's own buttons, [Return to <workspace>] and the
//! versions help (?).

use crate::doc_model::{DocEvent, DocModel};
use crate::theme::p;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// "Versions are view only. Viewing <version>." (the Drawing's has no full stop: live docver-1121).
pub fn version_text(version: &str, full_stop: bool) -> String {
    format!("Versions are view only. Viewing {version}{}", if full_stop { "." } else { "" })
}

/// A button of the bar (white, grey border), as its Restore to … and Custom features.
pub fn bar_button(id: &'static str, label: impl Into<SharedString>) -> Stateful<Div> {
    let pal = p();
    let label: SharedString = label.into();
    let mark = label.clone();
    div()
        .id(id)
        .map(move |d| crate::ui_script::mark(d, move || format!("button:{mark}")))
        .h(px(24.))
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(4.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgb(0xaaaaaa))
        .bg(rgb(0xffffff))
        .cursor_pointer()
        .hover(|d| d.bg(pal.row_hover))
        .text_size(px(12.))
        .child(label)
}

/// The bar: `text`, the tab's `extras`, [Return to <workspace>] and, for a version, the help (?).
pub fn render(id: &'static str, doc: &Entity<DocModel>, text: String, extras: Vec<AnyElement>, help: bool, cx: &App) -> AnyElement {
    let pal = p();
    let branch = {
        let s = &doc.read(cx).session;
        s.store.branch(s.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into())
    };
    let doc = doc.clone();
    let back = div()
        .id("vb-return")
        .map(|d| crate::ui_script::mark(d, || format!("button:Return to {branch}")))
        .h(px(24.))
        .px(px(8.))
        .flex()
        .items_center()
        .rounded(px(3.))
        .bg(pal.accent)
        .cursor_pointer()
        .text_size(px(12.))
        .text_color(rgb_static(0xffffff))
        .child(format!("Return to {branch}"))
        // Back to the workspace; the Versions and history panel closes (live ver-view-version-return).
        .on_click(move |_, _, cx| {
            doc.update(cx, |d, cx| {
                if d.session.view_microversion(None).is_ok() {
                    d.reloaded(cx);
                    cx.emit(DocEvent::Ui(noshape_ops::UiRequest::CloseHistory));
                }
            })
        });
    // Its ? opens help on versions (live: help versionshistorymanager).
    let help = help.then(|| {
        div()
            .id("vb-help")
            .map(|d| crate::ui_script::mark(d, || "button:Versions help".into()))
            .size(px(16.))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(crate::ui::glyph("help-dot", 17.8, crate::theme::help_dot(0x999999)))
            .on_click(|_, _, cx| crate::help::open("Versions and workspaces", cx))
    });
    crate::ui::toolbar()
        .id(id)
        .debug_selector(move || id.into())
        .justify_center()
        .child(
            h_flex()
                .h(px(32.))
                .mt(px(-4.))
                .px(px(20.))
                .gap(px(10.))
                .bg(rgb(0xffffff))
                .rounded_b(px(10.))
                .child(crate::ui::with_tooltip("tt-vb-text", text.clone(), div().id("vb-text").debug_selector({ let t = text.clone(); move || format!("vb-text {t}") }).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x333333)).child(text)))
                .children(extras)
                .child(back)
                .children(help),
        )
        .into_any_element()
}
