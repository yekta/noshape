//! The ? Help menu, the documents page's and a document's (live psui 1996: Help, Explore Onshape, FeatureScript
//! documentation, Keyboard shortcuts | What's new, Forums, Report a bug | System check, About; the cloud rows are
//! left out), working offline: Help opens noshape's user guide, FeatureScript documentation the standard library it
//! ships, System check its own check. Then noshape's two setup rows.

use crate::os_menu::{OsMenu, OsMenuItem};
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::v_flex;
use gpui_kit::*;

/// Every row, in live's order, then ours after a divider.
pub fn items(menu: OsMenu) -> OsMenu {
    let menu = menu
        .item(OsMenuItem::new("Help").on_click(|_, window, cx| open_guide(window, cx)))
        .item(OsMenuItem::new("FeatureScript documentation").on_click(|_, window, cx| open_featurescript_library(window, cx)))
        .item(OsMenuItem::new("Keyboard shortcuts").on_click(|_, window, cx| window.dispatch_action(Box::new(noshape_app_core::actions::ToggleShortcutHelp), cx)))
        .separator()
        .item(OsMenuItem::new("System check").on_click(|_, window, cx| crate::system_check::open(window, cx)));
    let menu = crate::about::help_item(menu).separator();
    crate::agent_setup::help_items(menu)
}

/// Help › Help: the user guide (ships with the app), in a scrolling modal.
fn open_guide(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, |dialog, _, cx| {
        crate::os_ui::modal(dialog, "Help", 800.)
            .child(div().id("help-guide").w_full().max_h(px(640.)).overflow_y_scroll().text_color(p().text).child(crate::doc_panels::markdown(noshape_ops::about::USER_GUIDE).w_full()))
            .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("close", "Close", cx))))
    });
}

/// Help › FeatureScript documentation: the standard library's source, read-only in a window of its own (its "std"
/// document is made once, off the UI thread). A failure says so.
fn open_featurescript_library(window: &mut Window, cx: &mut App) {
    let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
    let handle = window.window_handle();
    let made = cx.background_spawn(async { crate::std_source::std_document().map_err(|e| format!("{e:#}")) });
    cx.spawn(async move |cx| {
        let made = made.await;
        cx.update(|cx| {
            let opened = made.map_err(anyhow::Error::msg).and_then(|_| crate::std_source::open("common.fs", 1, 1, gpu, cx));
            if let Err(e) = opened {
                let message = format!("Could not open the FeatureScript library: {e:#}");
                let _ = handle.update(cx, |_, window, cx| {
                    window.open_dialog(cx, move |dialog, _, cx| {
                        crate::os_ui::modal(dialog, "FeatureScript documentation", 460.)
                            .child(crate::os_ui::modal_body().child(crate::os_ui::info_box(message.clone())))
                            .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("close", "Close", cx))))
                    })
                });
            }
        });
    })
    .detach();
}

/// The guide window (one; a later link shows its section in it).
struct GuideWindow {
    handle: AnyWindowHandle,
    view: Entity<GuideView>,
}

impl Global for GuideWindow {}

/// A section of the user guide in a window of its own (a dialog's ? or Learn more; live opens its help in a browser
/// tab), "Whole guide" for the rest.
struct GuideView {
    section: Option<String>,
}

impl Render for GuideView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let text = self.section.as_deref().and_then(noshape_ops::about::guide_section).unwrap_or(noshape_ops::about::USER_GUIDE);
        let whole = self.section.is_some().then(|| {
            div()
                .id("guide-whole")
                .px(px(16.))
                .pb(px(12.))
                .text_size(px(14.))
                .text_color(pal.link)
                .underline()
                .cursor_pointer()
                .child("Whole guide")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.section = None;
                    cx.notify();
                }))
        });
        v_flex()
            .size_full()
            .bg(pal.panel)
            .text_color(pal.text)
            .child(gpui_kit::component::TitleBar::new().bg(pal.panel_inactive).border_b_1().border_color(pal.border).child(div().pl(px(8.)).text_size(px(12.)).text_color(pal.text_muted).child("Help — Noshape")))
            .child(div().id("guide-text").flex_1().min_h_0().overflow_y_scroll().p(px(16.)).child(crate::doc_panels::markdown(text).w_full()))
            .children(whole)
    }
}

/// The user guide at `section` (noshape_ops::about::guide_section; the whole guide when there is none), in its window.
pub fn open_section(section: &str, cx: &mut App) {
    let section = noshape_ops::about::guide_section(section).is_some().then(|| section.to_string());
    if let Some(g) = cx.try_global::<GuideWindow>() {
        let (handle, view) = (g.handle, g.view.clone());
        if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
            view.update(cx, |v, cx| {
                v.section = section;
                cx.notify();
            });
            return;
        }
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(720.), px(640.)), cx))),
        window_min_size: Some(size(px(360.), px(240.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..gpui_kit::component::TitleBar::window_options()
    };
    let mut made = None;
    match crate::os_menu::open_window(options, cx, |window, cx| {
        window.set_window_title("Help — Noshape");
        let view = cx.new(|_| GuideView { section });
        made = Some(view.clone());
        view
    }) {
        Ok((handle, _)) => {
            if let Some(view) = made {
                cx.set_global(GuideWindow { handle, view });
            }
        }
        Err(e) => log::error!("could not open the help window: {e:#}"),
    }
}
