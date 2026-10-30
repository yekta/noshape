//! The Shift+/ Keyboard shortcuts modal every studio opens (live menus/616;
//! moved here from the Part Studio so the Part Studio, assemblies and
//! drawings share one copy): a white sheet over the dimmed window with the
//! title, "Customize keyboard shortcuts", a search box, ↗ and ×; tabs
//! General, Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing;
//! rows of right-aligned key chips and the action (🔒 = Onshape
//! controlled); a legend at the bottom. Searching lists matches of every
//! tab. It emits [`DismissEvent`] to be closed (Esc, Shift+/, ×, a click
//! outside); the host drops it and takes the focus back.

use crate::shortcuts::{self, KeyStyle, SheetRow, Tab};
use crate::theme::{p, with_alpha};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::sync::OnceLock;

/// What the sheet needs of the app: the key chip (Preferences' look), and
/// the Preferences page / the shortcuts page window.
pub struct Hooks {
    pub key_chip: fn(SharedString, KeyStyle) -> Div,
    pub show_keyboard_shortcuts: fn(&mut App),
    pub show_shortcuts_page: fn(Option<Size<Pixels>>, &mut App),
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Called once at startup by the app.
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn key_chip(text: impl Into<SharedString>, style: KeyStyle) -> Div {
    match HOOKS.get() {
        Some(h) => (h.key_chip)(text.into(), style),
        None => div().child(text.into()),
    }
}

/// The open sheet.
pub struct ShortcutSheet {
    query: String,
    search: Entity<InputState>,
    tab: usize,
    scroll: ScrollHandle,
    /// The rows' list: it has the keyboard while the sheet is open.
    list: FocusHandle,
    _sub: Subscription,
}

impl EventEmitter<DismissEvent> for ShortcutSheet {}

impl ShortcutSheet {
    /// Open on `tab` (a studio's own; live: a drawing's sheet starts on
    /// Drawing). Live (behaviours "qa (2026-10-27) Keyboard shortcuts
    /// modal", SHORTCUT-MAP-FOCUS): the list takes the keyboard, not the
    /// search box.
    pub fn open(tab: Tab, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search shortcuts"));
            let _sub = cx.subscribe(&search, |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.query = st.read(cx).value().to_string();
                    cx.notify();
                }
            });
            let list = cx.focus_handle();
            window.focus(&list, cx);
            let tab = Tab::SHEET.iter().position(|t| *t == tab).unwrap_or(0);
            Self { query: String::new(), search, tab, scroll: ScrollHandle::new(), list, _sub }
        })
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }
}

impl Render for ShortcutSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let query = self.query.clone();
        let tab = self.tab.min(Tab::SHEET.len() - 1);
        // Keys as chips (`span.key`); customized blue, disabled struck through.
        let keys_el = |r: &SheetRow| {
            let mut row = h_flex().gap(px(4.)).justify_end();
            if let Some(prefix) = r.prefix {
                row = row.child(div().text_size(px(14.)).text_color(pal.text).child(prefix));
            }
            for (i, group) in r.keys.iter().enumerate() {
                if i > 0 {
                    row = row.child(div().px(px(2.)).text_size(px(16.)).text_color(pal.text).child("/"));
                }
                for k in group {
                    row = row.child(key_chip(k.clone(), r.style));
                }
            }
            row
        };
        let custom = shortcuts::current();
        let rows: Vec<SheetRow> = if query.trim().is_empty() { shortcuts::sheet_rows(&custom, Tab::SHEET[tab]) } else { shortcuts::sheet_search(&custom, &query) };
        // Live (menus/616): the rows fill the tab bar's 599 px column, the
        // chips right-aligned in its first 230 px, the action 14 px after.
        // The rows' area: the sheet less its header, tabs and legend.
        let vp_h = window.viewport_size().height;
        let long = px(rows.len() as f32 * 39. + 21.) > vp_h - px(154. + 55. + 46. + 59.);
        let body = rows.into_iter().map(|r| {
            h_flex()
                .w_full()
                .h(px(39.))
                .flex_none()
                .gap(px(14.))
                .child(div().w(px(230.)).flex_none().child(keys_el(&r)))
                .child(
                    h_flex()
                        .flex_1()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(r.name)
                        // Live: the lock's 20 px box right after the name (4 px blank before the glyph).
                        .when(r.locked, |d| d.child(crate::assets::icon("lock", 20., cx)))
                        .when(r.conflict, |d| d.child(div().ml_1().child(crate::assets::icon("shortcut-conflict", 16., cx)))),
                )
        });
        let tabs = Tab::SHEET.iter().enumerate().map(|(i, t)| {
            let on = i == tab && query.trim().is_empty();
            div()
                .id(SharedString::from(format!("sc-tab-{i}")))
                .px(px(6.))
                .pb(px(6.))
                .cursor_pointer()
                .text_size(px(14.))
                .relative()
                .text_color(if on { pal.emphasis } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::MEDIUM))
                .child(t.label())
                // Live: the active tab's 2 px underline sits on the bar's 1 px line.
                .when(on, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(px(3.)).bg(pal.emphasis)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.tab = i;
                    this.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }))
        });
        let chip = |style| key_chip("example", style);
        let legend = h_flex()
            .gap(px(5.))
            .px(px(8.))
            .py(px(6.))
            .border_1()
            .border_color(pal.text)
            .rounded(px(3.))
            .text_size(px(14.))
            .text_color(pal.text)
            .child(chip(KeyStyle::Customized))
            .child("Customized")
            .child(div().w(px(4.)))
            .child(chip(KeyStyle::Default))
            .child("Onshape default")
            .child(div().w(px(4.)))
            .child(chip(KeyStyle::Disabled))
            .child("Disabled")
            .child(div().w(px(4.)))
            .child(crate::assets::icon("lock", 16., cx))
            .child("Onshape controlled")
            .child(div().w(px(4.)))
            .child(crate::assets::icon("shortcut-conflict", 18., cx))
            .child("Has conflict")
            .child(div().w(px(4.)))
            .child("* applies in drawing sketches");
        // Live (menus/616): over the whole window (navbar and panels dimmed
        // too), 1000 px wide and centred, 26 px from the top, the window's
        // height less 154 px tall.
        let vp = window.viewport_size();
        let sheet = div()
            .id("shortcut-sheet")
            .occlude()
            .w(vp.width)
            .h(vp.height)
            .flex()
            .justify_center()
            .pt(px(26.))
            .bg(with_alpha(pal.overlay_shadow, 0.5))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    this.close(cx);
                }
            }))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
            .child(
                v_flex()
                    .id("shortcut-panel")
                    .w(px(1000.))
                    .max_w(relative(0.95))
                    .h((vp.height - px(154.)).max(px(300.)))
                    .bg(gpui_kit::rgb(0xffffff))
                    .shadow_lg()
                    // Clicks inside the sheet (search box, tabs) keep it open.
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .flex_none()
                            .h(px(55.))
                            .px(px(10.))
                            .gap(px(6.))
                            .border_b_1()
                            .border_color(pal.border)
                            // Live (menus/616): #555 (#d7d7d7 in dark, 2446), 12 px in.
                            .child(div().ml(px(2.)).text_size(px(19.)).text_color(crate::theme::themed(0x555555, 0xd7d7d7)).child("Keyboard shortcuts"))
                            .child(
                                div()
                                    .id("sc-customize")
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    // Live: #094174 (616), #b2ddf6 in dark (2446).
                                    .text_color(crate::theme::themed(0x094174, 0xb2ddf6))
                                    .cursor_pointer()
                                    .child("Customize keyboard shortcuts")
                                    // Onshape opens the account Preferences page (a new tab)
                                    // at Keyboard shortcuts; the sheet stays open here.
                                    .on_click(|_: &ClickEvent, _, cx| {
                                        if let Some(h) = HOOKS.get() {
                                            (h.show_keyboard_shortcuts)(cx)
                                        }
                                    }),
                            )
                            .child(div().flex_1())
                            .child(div().w(px(198.)).h(px(34.)).child(Input::new(&self.search).h(px(34.))))
                            // Live (menus/616): ↗ opens the page in its own window.
                            .child(
                                div()
                                    .id("sc-popout")
                                    // Live menus/616: 21 px after the search box, 6 px above centre.
                                    .ml(px(17.))
                                    .relative()
                                    .top(px(-4.))
                                    .cursor_pointer()
                                    .child(crate::assets::icon("popout-arrow", 14., cx))
                                    .on_click(|_: &ClickEvent, window, cx| {
                                        if let Some(h) = HOOKS.get() {
                                            (h.show_shortcuts_page)(Some(window.bounds().size), cx)
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .id("sc-close")
                                    .ml(px(-5.))
                                    .relative()
                                    .top(px(-3.))
                                    .cursor_pointer()
                                    // Live (menus/616): the modal's × 10 x 10, darkest 136, 9 px after ↗.
                                    .opacity(0.95)
                                    .child(crate::ui::glyph("close", 19., gpui_kit::rgb(0x333333)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
                            ),
                    )
                    .child(h_flex().flex_none().w(px(599.)).mx_auto().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).gap(px(17.)).pt(px(16.)).children(tabs))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .w(px(599.))
                            .mx_auto()
                            .mt(px(10.))
                            .relative()
                            .child(
                                div()
                                    .id("sc-rows")
                                    .size_full()
                                    .overflow_y_scroll()
                                    // "Input": a studio's bare-key shortcuts stay quiet while the map has the
                                    // keyboard, as while typing; Shift+/ and Esc close it here.
                                    .track_focus(&self.list)
                                    .key_context("ShortcutMap Input")
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                        let k = &ev.keystroke;
                                        if k.key == "escape" || k.key == "?" || (k.key == "/" && k.modifiers.shift) {
                                            cx.stop_propagation();
                                            this.close(cx);
                                        }
                                    }))
                                    .track_scroll(&self.scroll)
                                    .pt(px(3.))
                                    .pb(px(8.))
                                    .pr(px(gpui_kit::component::menu::NATIVE_SCROLLBAR_WIDTH))
                                    .child(v_flex().children(body)),
                            )
                            .when(long, |d| d.child(gpui_kit::component::menu::native_scrollbar(&self.scroll, cx)))
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<gpui_kit::component::menu::NativeThumbDrag>, window, cx| {
                                gpui_kit::component::menu::native_scrollbar_drag(&this.scroll, e.event.position.y);
                                window.refresh();
                                cx.notify();
                            })),
                    )
                    .child(div().flex_none().p(px(10.)).border_t_1().border_color(pal.border).flex().justify_center().child(legend)),
            );
        // Drawn over everything in the window, from its top-left corner.
        deferred(anchored().position(point(px(0.), px(0.))).child(sheet)).with_priority(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    /// Moved from the Part Studio with the sheet.
    #[test]
    fn shortcut_sheet_lists_every_part_studio_key() {
        let s = shortcuts::Shortcuts::default();
        let keys: Vec<String> = Tab::SHEET.iter().flat_map(|t| shortcuts::sheet_rows(&s, *t)).flat_map(|r| r.keys.into_iter().map(|g| g.join(" "))).collect();
        for k in ["y", "shift y", "shift i", "shift t", "w", "s", "`", "[", "shift p"] {
            assert!(keys.iter().any(|x| x == k), "{k} missing from the shortcut sheet");
        }
        assert!(shortcuts::sheet_search(&s, "isol").iter().any(|r| r.name == "Make isolate"));
        assert!(shortcuts::sheet_search(&s, "shift+i").iter().any(|r| r.name == "Make isolate"));
        assert!(!shortcuts::sheet_search(&s, "fillet").iter().any(|r| r.name == "Zoom to window"));
    }
}
