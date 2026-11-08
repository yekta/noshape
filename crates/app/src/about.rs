//! Help › About (live: the ? menu's last row, its About modal; onshape-research/live/psui/about-1105): an 800 px
//! modal 25 px from the top with no title bar and a ✕ in its corner: the logo and wordmark, the copyright, Open
//! CASCADE's notice, the link to the open source components' notices (shipped with the app, opened by the system,
//! offline) and the build in the footer.

use crate::os_menu::{OsMenu, OsMenuItem};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogClose;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::*;
use noshape_ops::about::{COPYRIGHT, OCCT_NOTICE};

actions!(noshape_about, [ShowAbout]);

/// The macOS app menu's "About noshape" (the other OSs' Help menus have the row): the modal in the active window.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &ShowAbout, cx| {
        // (After the menu's dispatch: an update of the window it went to fails from inside it.)
        cx.defer(|cx| {
            let mut w = cx.active_window().or_else(|| cx.windows().first().copied());
            if w.is_none() {
                crate::files::show_start_window(cx);
                w = cx.active_window().or_else(|| cx.windows().first().copied());
            }
            if let Some(w) = w
                && let Err(e) = w.update(cx, |_, window, cx| open(window, cx))
            {
                log::error!("about: could not open the dialog: {e:#}");
            }
        });
    });
}

/// The Help menu's About row (live: "About", after System check).
pub fn help_item(menu: OsMenu) -> OsMenu {
    menu.item(OsMenuItem::new("About").on_click(|_, window, cx| open(window, cx)))
}

/// The About modal in `window`.
pub fn open(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, |dialog, _, cx| {
        // Live's reader: a modal with no title, fields or buttons (its content is an iframe).
        noshape_app_core::ui_script::note_dialog(|| serde_json::json!({ "kind": "modal", "title": "", "fields": [], "buttons": [] }));
        // Live's colours: text #131313, links #0000ee, the version #666, the footer's rule #dee2e6, the ✕ #999.
        let link = rgb(0x0000ee);
        dialog
            .w(px(800.))
            .margin_top(px(25.))
            .close_button(false)
            .p_0()
            .gap_0()
            .rounded(px(5.))
            .child(
                div()
                    .relative()
                    .h(px(472.))
                    .px(px(30.))
                    .pt(px(37.))
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x131313))
                    .child(
                        div().absolute().top(px(12.)).right(px(12.)).child(DialogClose::new().child(
                            div().id("about-close").size(px(24.)).flex().items_center().justify_center().cursor_pointer().text_color(rgb(0x999999)).hover(|d| d.text_color(rgb(0x666666))).child(Icon::empty().data(crate::os_ui::GLYPH_MODAL_CLOSE).size(px(18.))),
                        )),
                    )
                    .child(
                        h_flex()
                            .h(px(75.))
                            .gap(px(14.))
                            .child(crate::assets::icon("noshape-logo", 64., cx))
                            .child(div().text_size(px(44.)).line_height(px(52.)).text_color(rgb(0x3d4647)).child("noshape")),
                    )
                    .child(div().mt(px(20.)).debug_selector(|| "about-copyright".into()).child(COPYRIGHT))
                    .child(div().mt(px(20.)).max_w(px(740.)).child(OCCT_NOTICE))
                    .child(
                        div()
                            .mt(px(20.))
                            .flex()
                            .flex_wrap()
                            .child(
                                div()
                                    .id("about-oss")
                                    .debug_selector(|| "about-oss".into())
                                    .text_color(link)
                                    .underline()
                                    .cursor_pointer()
                                    .on_click(|_, _, cx| open_notices(cx))
                                    .child("Open source software components"),
                            )
                            .child("\u{a0}used in noshape."),
                    ),
            )
            .footer(
                h_flex()
                    .w_full()
                    .h(px(52.))
                    .justify_center()
                    .border_t_1()
                    .border_color(rgb(0xdee2e6))
                    .text_size(px(14.))
                    .text_color(rgb(0x666666))
                    .debug_selector(|| "about-version".into())
                    .child(crate::build_label()),
            )
    });
}

/// The notices written out and opened by the system's viewer (offline: they ship with the app; `app_about`).
fn open_notices(cx: &mut App) {
    match noshape_ops::about::notices_file() {
        Ok(path) => system_open(&path, cx),
        Err(e) => log::error!("about: could not write the notices: {e:#}"),
    }
}

#[cfg(not(test))]
fn system_open(path: &std::path::Path, cx: &App) {
    cx.open_with_system(path);
}

#[cfg(test)]
thread_local! {
    /// What the link would have opened (the test platform opens nothing).
    static OPENED: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn system_open(path: &std::path::Path, _: &App) {
    OPENED.with(|o| *o.borrow_mut() = Some(path.to_path_buf()));
}

#[cfg(test)]
mod tests {
    use super::{OPENED, ShowAbout, init};
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Context, IntoElement, Modifiers, ParentElement as _, Point, Render, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, px, size};

    struct HelpHost;

    impl Render for HelpHost {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(noshape_start_ui::start_window::help_menu(cx))
        }
    }

    fn draw(cx: &mut TestAppContext, w: AnyWindowHandle) {
        cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        cx.run_until_parked();
    }

    fn host(cx: &mut TestAppContext) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        crate::register_start_hooks();
        cx.update(init);
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        // (app-core's window: it draws the open menu, as the app's windows do.)
        let w = cx.update(|cx| crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| HelpHost)).unwrap().0);
        draw(cx, w);
        w
    }

    /// The About modal as live's: the copyright, Open CASCADE's notice, the components link and the build; the link
    /// opens the notices shipped with the app.
    fn assert_about(cx: &mut TestAppContext, w: AnyWindowHandle) {
        assert!(cx.update_window(w, |_, window, cx| window.has_active_dialog(cx)).unwrap(), "the About modal is open");
        let mut vcx = VisualTestContext::from_window(w, cx);
        for part in ["about-copyright", "about-oss", "about-version"] {
            assert!(vcx.debug_bounds(part).is_some(), "{part} shown");
        }
        let modal = vcx.debug_bounds("about-copyright").unwrap();
        assert!(modal.origin.y > px(25.), "below the modal's top");
        let link = vcx.debug_bounds("about-oss").unwrap();
        vcx.simulate_click(link.center(), Modifiers::default());
        draw(cx, w);
        let opened = OPENED.with(|o| o.borrow_mut().take()).expect("the link opened the notices");
        let text = std::fs::read_to_string(&opened).unwrap();
        assert_eq!(text, noshape_ops::about::NOTICES);
        assert!(text.contains("Open CASCADE exception"), "Open CASCADE's LGPL in the notices");
    }

    /// Help › About (the start window's ? menu; a document's has the same items): live's last row, after System check,
    /// opens the About modal.
    #[gpui::test]
    fn help_about_opens_the_about_modal(cx: &mut TestAppContext) {
        let w = host(cx);
        let menu = noshape_start_ui::start_window::HELP_MENU;
        let mut vcx = VisualTestContext::from_window(w, cx);
        let b = vcx.debug_bounds(menu).expect("the Help button");
        vcx.simulate_click(b.center(), Modifiers::default());
        draw(cx, w);
        let rows = cx.update(|cx| noshape_app_core::os_ui::open_dropdown_labels(menu, cx)).expect("the Help menu is open");
        assert!(rows.iter().position(|r| r == "About") == rows.iter().position(|r| r == "System check").map(|i| i + 1), "{rows:?}");
        // (The dropdown's rows are laid out in the frame after it opens.)
        draw(cx, w);
        let mut vcx = VisualTestContext::from_window(w, cx);
        let row = vcx.debug_bounds("menu:About").expect("the About row");
        vcx.simulate_click(row.center(), Modifiers::default());
        draw(cx, w);
        assert_about(cx, w);
    }

    /// The macOS app menu's About noshape (its action): the same modal in the active window.
    #[gpui::test]
    fn the_app_menus_about_opens_the_about_modal(cx: &mut TestAppContext) {
        let w = host(cx);
        cx.update_window(w, |_, window, _| window.activate_window()).unwrap();
        cx.update(|cx| cx.dispatch_action(&ShowAbout));
        draw(cx, w);
        assert_about(cx, w);
    }
}
