//! The keyboard shortcuts page that the shortcut map's ↗ opens in its own
//! window (live menus/629, dom/proj-shortcuts-fullpage.txt): the title at the
//! top left; every section in two half-width columns (General, Sketch,
//! Drawing | 3D view, Part Studio, Assembly, Feature Studio), each a blue
//! title over the shortcut map's rows (key chips right-aligned at 40 % of the
//! column, the action 14 px after them, 39 px apart); the legend in a bar
//! fixed at the bottom; a footer line with the version at the right (Onshape's
//! copyright and Terms links are its own, rule 6 / cloud).

use crate::shortcuts::{self, KeyStyle, SheetRow, Tab};
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Live: column-major, left then right.
const COLUMNS: [&[Tab]; 2] = [&[Tab::General, Tab::Sketch, Tab::Drawing], &[Tab::View3d, Tab::PartStudio, Tab::Assembly, Tab::FeatureStudio]];

struct ShortcutsWindow {
    handle: AnyWindowHandle,
}

impl Global for ShortcutsWindow {}

pub struct ShortcutsPage {
    scroll: ScrollHandle,
}

/// Opens the page, or brings an open one to the front. Live opens it as a
/// new browser tab, so at the opener's size and place (live 2460: 1919×1079).
pub fn show(opener: &Window, cx: &mut App) {
    if let Some(g) = cx.try_global::<ShortcutsWindow>() {
        let handle = g.handle;
        if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return;
        }
    }
    let options = WindowOptions {
        window_bounds: Some(opener.window_bounds()),
        window_min_size: Some(size(px(800.), px(480.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..gpui_kit::component::TitleBar::window_options()
    };
    match crate::os_menu::open_window(options, cx, |window, cx| {
        window.set_window_title("Keyboard shortcuts — noshape");
        cx.new(|_| ShortcutsPage { scroll: ScrollHandle::new() })
    }) {
        Ok((handle, _)) => cx.set_global(ShortcutsWindow { handle }),
        Err(e) => log::error!("could not open the keyboard shortcuts page: {e:#}"),
    }
}

/// A row's key chips (alternatives split by "/"), as in the shortcut map.
fn keys(r: &SheetRow) -> Div {
    let pal = p();
    let mut row = h_flex().gap(px(4.)).justify_end();
    if let Some(prefix) = r.prefix {
        row = row.child(div().text_size(px(14.)).text_color(pal.text).child(prefix));
    }
    for (i, group) in r.keys.iter().enumerate() {
        if i > 0 {
            row = row.child(div().px(px(2.)).text_size(px(16.)).text_color(pal.text).child("/"));
        }
        for k in group {
            row = row.child(crate::preferences_window::key_chip(k.clone(), r.style));
        }
    }
    row
}

fn section(tab: Tab, custom: &shortcuts::Shortcuts, cx: &App) -> Div {
    let pal = p();
    let rows = shortcuts::sheet_rows(custom, tab).into_iter().map(|r| {
        h_flex()
            .h(px(39.))
            .flex_none()
            .gap(px(14.))
            .child(div().w(relative(0.396)).flex_none().child(keys(&r)))
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
    v_flex()
        .pt(px(2.))
        .pb(px(16.))
        // Live: the title over the actions' column, blue, 16 px, 34 px above the first row.
        .child(h_flex().h(px(30.)).gap(px(13.)).child(div().w(relative(0.396)).flex_none()).child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.emphasis).child(tab.label())))
        .children(rows)
}

impl Render for ShortcutsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let custom = shortcuts::current();
        let columns = COLUMNS.iter().map(|tabs| v_flex().w(relative(0.5)).flex_none().children(tabs.iter().map(|t| section(*t, &custom, cx))));
        let chip = |style| crate::preferences_window::key_chip("example", style);
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
        v_flex()
            .size_full()
            .bg(gpui_kit::rgb(0xffffff))
            .text_color(pal.text)
            // The window's own bar (moving, closing); live is a browser popup.
            .child(gpui_kit::component::TitleBar::new().bg(pal.panel_inactive).border_b_1().border_color(pal.border).child(div().pl(px(8.)).text_size(px(12.)).text_color(pal.text_muted).child("Keyboard shortcuts — noshape")))
            .child(div().flex_none().h(px(45.)).px(px(10.)).flex().items_center().text_size(px(19.)).text_color(crate::theme::themed(0x555555, 0xd7d7d7)).child(div().relative().top(px(3.)).child("Keyboard shortcuts")))
            // Chrome's scrollbar on the page's right edge (live: the columns are half of what is left).
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(div().id("sc-page-rows").size_full().overflow_y_scroll().track_scroll(&self.scroll).pr(px(gpui_kit::component::menu::NATIVE_SCROLLBAR_WIDTH)).child(h_flex().items_start().children(columns)))
                    .child(gpui_kit::component::menu::native_scrollbar(&self.scroll, cx))
                    .on_drag_move(cx.listener(|this, e: &DragMoveEvent<gpui_kit::component::menu::NativeThumbDrag>, window, cx| {
                        gpui_kit::component::menu::native_scrollbar_drag(&this.scroll, e.event.position.y);
                        window.refresh();
                        cx.notify();
                    })),
            )
            // Live: the legend 10 px under the bar's top line (not centred).
            .child(div().flex_none().h(px(72.)).pt(px(10.)).border_t_1().border_color(pal.border).flex().items_start().justify_center().child(legend))
            .child(h_flex().flex_none().h(px(29.)).border_t_1().border_color(crate::theme::themed(0xd0dbe5, 0x555555)).px(px(240.)).justify_end().text_size(px(11.)).text_color(pal.text_muted).child(format!("({})", crate::VERSION)))
    }
}
