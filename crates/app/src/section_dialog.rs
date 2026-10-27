//! Onshape's Section view dialog, shared by the Part Studio and the
//! assembly (Help View: "Available in: Part Studio, Assembly"). Measured on
//! live part-studio/70-71: 220 px at the feature dialog's place; a 29 px
//! header "Section view" ✓ ✕; Exclude | Include tabs (2 px blue / 1 px
//! `#d7d7d7` underlines on one baseline); the "Section planes" field with the
//! plane button flush after it and "Items to exclude" (or include), 3 px in,
//! 7 px under the tabs, 5 px apart; the help ? 4 px from the bottom right.
//! The host view keeps the state and handles each [`Action`].

use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// What the dialog shows.
pub struct Props {
    /// Include tab (else Exclude).
    pub include: bool,
    /// The "Items" field takes picks (else "Section planes").
    pub items_active: bool,
    /// Plane labels ("Top", "Face of Part 1 <1>", …).
    pub planes: Vec<String>,
    /// The plane the manipulator drives (its chip is bold).
    pub active: usize,
    /// Excluded / included items' names.
    pub items: Vec<String>,
    /// The "Select default plane" popup, when open.
    pub popup: Option<PlanePopup>,
}

/// The "Select default plane" popup (live behaviours "Select default
/// plane", ui-snapv/25): a 308 px HUD right of the plane button with a
/// drag handle, [Select a plane ▾] (Top | Front | Right), Flip plane, a
/// green ✓ ("Commit current plane") and ✕ ("Cancel").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanePopup {
    /// Its plane list is open.
    pub list_open: bool,
}

/// What the user did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Accept,
    Cancel,
    /// A tab: `true` Include.
    Include(bool),
    /// A field was clicked: `true` the Items field.
    ItemsActive(bool),
    RemovePlane(usize),
    RemoveItem(usize),
    /// The plane button beside "Section planes".
    PlaneButton,
    /// The popup's [Select a plane ▾] (opens or closes its list).
    PopupList,
    /// A default plane from the popup's list: 0 Top, 1 Front, 2 Right.
    PopupPick(usize),
    PopupFlip,
    PopupCommit,
    PopupCancel,
}

/// The dialog for `view`; `on` runs each action on it. `key` names the
/// panel for dragging and its saved position (`ui::panel_title`).
pub fn render<V: 'static>(props: Props, key: &'static str, view: &Entity<V>, on: fn(&mut V, Action, &mut Window, &mut Context<V>), cx: &App) -> AnyElement {
    let pal = p();
    let act = |a: Action| {
        let v = view.clone();
        move |_: &ClickEvent, w: &mut Window, cx: &mut App| v.update(cx, |this, cx| on(this, a, w, cx))
    };
    let tab = |label: &'static str, on_tab: bool, include: bool| {
        div()
            .id(label)
            .flex_1()
            .h(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .cursor_pointer()
            .relative()
            .when(on_tab, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM))
            .when(!on_tab, |d| d.text_color(pal.text))
            .child(label)
            // Live part-studio/70: a 2 px blue underline on the active tab, 1 px
            // #d7d7d7 on the other, over the label's box (both labels on one baseline).
            .child(div().absolute().left_0().right_0().bottom_0().h(px(if on_tab { 2. } else { 1. })).bg(if on_tab { pal.accent } else { gpui_kit::rgb(0xd7d7d7) }))
            .on_click(act(Action::Include(include)))
    };
    let chip = |id: SharedString, label: String, bold: bool, remove: Action| {
        h_flex()
            .id(id.clone())
            .h(px(22.))
            .px(px(6.))
            .text_size(px(12.))
            .text_color(pal.text)
            .when(bold, |d| d.font_weight(FontWeight::BOLD))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-x")))
                    // Live (part-studio/71): the × is always shown.
                    .cursor_pointer()
                    .child(crate::ui::glyph("ui-clear", 9., pal.icon_tertiary))
                    .on_click({
                        let a = act(remove);
                        move |e, w, cx| {
                            cx.stop_propagation();
                            a(e, w, cx)
                        }
                    }),
            )
            .into_any_element()
    };
    let field = |id: &'static str, caption: String, on_field: bool, rows: Vec<AnyElement>, items: bool| {
        let empty = rows.is_empty();
        v_flex()
            .id(id)
            .flex_1()
            // Long chips end in "…" instead of widening the dialog (live).
            .min_w_0()
            // Live part-studio/70-71: 27 px empty, 42 px with one chip.
            .min_h(px(27.))
            .when(empty, |d| d.justify_center())
            .rounded(px(2.))
            .border_1()
            .border_color(if on_field { pal.accent } else { gpui_kit::rgb(0xd7d7d7) })
            .bg(if on_field { gpui_kit::rgb(0xdef1fb) } else { gpui_kit::rgb(0xffffff) })
            .cursor_pointer()
            .child(div().px(px(6.)).when(empty, |d| d.text_size(px(12.))).when(!empty, |d| d.pt(px(2.)).text_size(px(10.))).text_color(pal.text_muted).child(caption))
            .children(rows)
            .on_click(act(Action::ItemsActive(items)))
    };
    let plane_rows = props.planes.iter().enumerate().map(|(i, l)| chip(SharedString::from(format!("sec-plane-{i}")), if l.is_empty() { format!("Section plane {}", i + 1) } else { format!("Section plane {} ({l})", i + 1) }, i == props.active, Action::RemovePlane(i))).collect();
    let item_rows = props.items.iter().enumerate().map(|(i, n)| chip(SharedString::from(format!("sec-item-{i}")), n.clone(), false, Action::RemoveItem(i))).collect();
    let planes_field = field("sec-planes", "Section planes".into(), !props.items_active, plane_rows, false);
    let items_field = field("sec-items", if props.include { "Items to include".into() } else { "Items to exclude".into() }, props.items_active, item_rows, true);
    let white = gpui_kit::rgb(0xffffff);
    let frame = gpui_kit::rgb(0xcccccc);
    let popup = props.popup.map(|pp| {
        let opt = |i: usize, label: &'static str| {
            div()
                .id(SharedString::from(format!("sec-pop-{label}")))
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .text_size(px(14.))
                .text_color(pal.text)
                .cursor_pointer()
                .hover(|d| d.bg(pal.row_hover))
                .child(label)
                .on_click(act(Action::PopupPick(i)))
        };
        let square = || div().size(px(6.)).border_1().border_color(gpui_kit::rgb(0x999999));
        let button = |id: &'static str, tip: &'static str, a: Action| div().id(id).size(px(26.)).flex_none().flex().items_center().justify_center().rounded(px(3.)).cursor_pointer().tooltip(crate::ui::tooltip(tip)).on_click(act(a));
        v_flex()
            .id("sec-plane-popup")
            .occlude()
            .absolute()
            // Live ui-snapv/25: 14 px right of the plane button, 69 px below the dialog's top.
            .left(px(231.))
            .top(px(71.))
            .w(px(308.))
            .bg(white)
            .border_1()
            .border_color(frame)
            .rounded(px(3.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .child(h_flex().h(px(16.)).justify_center().gap(px(1.)).border_b_1().border_color(frame).child(square()).child(square()).child(square()))
            .child(
                h_flex()
                    .px(px(5.))
                    .py(px(4.))
                    .gap(px(5.))
                    .child(
                        v_flex()
                            .relative()
                            .w(px(203.))
                            .child(
                                h_flex()
                                    .id("sec-pop-select")
                                    .h(px(26.))
                                    .px(px(6.))
                                    .border_b_1()
                                    .border_color(gpui_kit::rgb(0x999999))
                                    .cursor_pointer()
                                    .child(div().flex_1().text_size(px(14.)).text_color(gpui_kit::rgb(0x666666)).child("Select a plane"))
                                    .child(crate::ui::glyph("ui-caret-down", 12., pal.text))
                                    .on_click(act(Action::PopupList)),
                            )
                            .when(pp.list_open, |d| {
                                d.child(v_flex().absolute().top(px(27.)).left_0().w(px(203.)).bg(white).border_1().border_color(frame).child(opt(0, "Top")).child(opt(1, "Front")).child(opt(2, "Right")))
                            }),
                    )
                    .child(button("sec-pop-flip", "Flip plane", Action::PopupFlip).border_1().border_color(frame).hover(|d| d.bg(pal.panel_hover)).child(crate::assets::icon("flip-direction", 20., cx)))
                    .child(button("sec-pop-ok", "Commit current plane", Action::PopupCommit).bg(gpui_kit::rgb(0x009400)).child(crate::assets::icon("dialog-ok", 20., cx)))
                    .child(button("sec-pop-cancel", "Cancel", Action::PopupCancel).border_1().border_color(frame).hover(|d| d.bg(pal.panel_hover)).child(crate::assets::icon("dialog-cancel", 20., cx))),
            )
    });
    v_flex()
        .id("section-dialog")
        .occlude()
        .relative()
        .children(popup)
        .child(crate::ui::panel_probe(key))
        .w(px(220.))
        .bg(pal.dialog_bg)
        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
        .child(
            h_flex()
                .h(px(29.))
                .pl(px(6.))
                .bg(pal.panel)
                .border_b_1()
                .border_color(pal.border)
                .child(crate::ui::panel_title(key, div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Section view")))
                .child(div().id("sec-ok").size(px(28.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::assets::icon("dialog-ok", 24., cx)).on_click(act(Action::Accept)))
                .child(div().id("sec-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().hover(|d| d.bg(pal.panel_hover)).child(crate::assets::icon("dialog-cancel", 24., cx)).on_click(act(Action::Cancel))),
        )
        .child(h_flex().child(tab("Exclude", !props.include, false)).child(tab("Include", props.include, true)))
        .child(
            // Live part-studio/71: fields 3 px in, 7 px under the tabs, 5 px
            // apart; the plane button right after the field.
            v_flex()
                .px(px(3.))
                .pt(px(7.))
                .gap(px(5.))
                .child(
                    h_flex().items_start().child(planes_field).child(
                        // Live (part-studio/71): a 20 px plane sprite in the 26 px side button.
                        div()
                            .id("sec-view-plane")
                            .size(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .tooltip(crate::ui::tooltip("Select default plane"))
                            .child(crate::assets::icon("plane", 20., cx))
                            .on_click(act(Action::PlaneButton)),
                    ),
                )
                .child(items_field)
                .child(h_flex().justify_end().mt(px(4.)).mb(px(2.)).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0x999999)))),
        )
        .into_any_element()
}
