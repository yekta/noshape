//! Onshape's Named views dialog (Shift+V, camera menu "Named views…"),
//! shared by the Part Studio and the assembly. Live (part-studio/75-76,
//! psb/nv-911/915, dom/panel-named-views.txt): 265 px at the feature dialog's
//! place; a 29 px header "Named views" ×; the 220 px "New view name" field
//! and a 30 x 30 "+" (save the current view); then "No views created", or a
//! native "Select a view" list (picking one goes there) and a 30 x 30 ⊗;
//! the help row. The host keeps the views and handles each [`Action`].

use crate::theme::{p, with_alpha};
use gpui_kit::component::input::InputState;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// What the dialog shows.
pub struct Props {
    /// Saved views' names, in order.
    pub views: Vec<String>,
    /// The view picked in the list.
    pub picked: Option<String>,
}

/// What the user did.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Close,
    /// "+" or Enter: save the current view under the typed name.
    Add,
    /// A view picked in the list (applied at once).
    Pick(String),
    /// ⊗: delete the picked view (no confirmation, live).
    Remove,
}

/// The dialog for `view`, its name field `input`; `on` runs each action on
/// it. `key` names the panel for dragging and its saved position.
pub fn render<V: 'static>(props: Props, input: &Entity<InputState>, key: &'static str, view: &Entity<V>, on: fn(&mut V, Action, &mut Window, &mut Context<V>), cx: &mut App) -> AnyElement {
    let pal = p();
    let act = |a: Action| {
        let v = view.clone();
        move |_: &ClickEvent, w: &mut Window, cx: &mut App| v.update(cx, |this, cx| on(this, a.clone(), w, cx))
    };
    // Live (psb/nv-911, behaviours "Named views"): a native <select>,
    // "Select a view" (a disabled placeholder) until one is picked;
    // picking a view applies it at once.
    let current = props.picked.as_ref().and_then(|n| props.views.iter().position(|x| x == n)).map_or(0, |i| i + 1);
    let picks = props.views.clone();
    let v = view.clone();
    let select = crate::os_ui::select_field_flat("named-view-select", std::iter::once("Select a view".to_string()).chain(props.views.iter().cloned()).collect(), vec![0], 1, current, move |i, w, cx| {
        let Some(name) = i.checked_sub(1).and_then(|k| picks.get(k)).cloned() else { return };
        v.update(cx, |this, cx| on(this, Action::Pick(name), w, cx));
    });
    let empty_name = input.read(cx).value().is_empty();
    let picked = props.picked.is_some();
    let square = |id: &'static str, icon: AnyElement, border: Rgba| {
        div()
            .id(id)
            .size(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(border)
            .rounded(px(3.5))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(icon)
    };
    // Live part-studio/75 (measured): until a name is typed the "+" is
    // #aeaeae in a #ddd border; enabled #999 in #aaa (dom/panel-named-views).
    let (add_color, add_border) = if empty_name { (gpui_kit::rgb(0xaeaeae), gpui_kit::rgb(0xdddddd)) } else { (gpui_kit::rgb(0x999999), gpui_kit::rgb(0xaaaaaa)) };
    let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }];
    let esc = view.clone();
    v_flex()
        .id("named-views")
        .occlude()
        .on_action(move |_: &gpui_kit::component::input::Escape, w, cx| esc.update(cx, |this, cx| on(this, Action::Close, w, cx)))
        .child(crate::ui::panel_probe(key))
        .w(px(265.))
        .bg(with_alpha(gpui_kit::rgb(0xffffff), 0.95))
        .shadow(shadow)
        .text_color(pal.text)
        .child(
            h_flex()
                .h(px(29.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0xd7d7d7))
                .child(crate::ui::panel_title(key, div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Named views")))
                .child(
                    div()
                        .id("nv-close")
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .opacity(0.5)
                        .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 23., gpui_kit::rgb(0x333333)))
                        .on_click(act(Action::Close)),
                ),
        )
        .child(
            h_flex()
                .h(px(40.))
                .px(px(5.))
                .gap(px(5.))
                // Live: a focused 2 px blue underline (1 px + inset), italic
                // placeholder, text 6 px down; the field's top at 115, 1 px
                // under the centred position.
                .child(
                    div()
                        .relative()
                        .top(px(1.))
                        .w(px(220.))
                        .h(px(27.))
                        .pt(px(3.))
                        .border_b_1()
                        .border_color(pal.accent)
                        .child(crate::ui::input_with_placeholder(input, "New view name", cx).appearance(false).small().text_size(px(13.33)).pl(px(3.)))
                        .child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.accent)),
                )
                .child(square("nv-add", crate::ui::glyph("add-button", 20., add_color).into_any_element(), add_border).tooltip(crate::ui::tooltip("Add view")).on_click(act(Action::Add))),
        )
        .when(props.views.is_empty(), |d| d.child(div().px(px(5.)).h(px(26.)).flex().items_center().text_size(px(12.)).child("No views created")))
        .when(!props.views.is_empty(), |d| {
            d.child(
                h_flex()
                    .h(px(40.))
                    .px(px(5.))
                    .gap(px(5.))
                    .child(div().w(px(220.)).child(select))
                    // Live psb/nv-911 / nv-915: a solid #333 ⊗ in an #aaa border once a
                    // view is picked, 40 % in a #ddd border before.
                    .child(
                        square("nv-remove", crate::os_ui::glyph(crate::os_ui::GLYPH_REMOVE_CIRCLE, 20., if picked { Hsla::from(gpui_kit::rgb(0x333333)) } else { Hsla::from(gpui_kit::rgb(0x333333)).opacity(0.4) }).into_any_element(), if picked { gpui_kit::rgb(0xaaaaaa) } else { gpui_kit::rgb(0xdddddd) })
                            .tooltip(crate::ui::tooltip("Remove view"))
                            .on_click(act(Action::Remove)),
                    ),
            )
        })
        .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(div().size(px(14.)).rounded_full().flex().items_center().justify_center().child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa)))))
        .into_any_element()
}
