//! The ? Help menu, the documents page's and a document's (live psui 1996: Help, Explore Onshape, FeatureScript
//! documentation, Keyboard shortcuts | What's new, Forums, Report a bug | System check, About; the cloud rows are
//! left out), working offline: Help opens noshape's user guide, FeatureScript documentation the standard library it
//! ships, System check its own check. Then noshape's two setup rows.

use crate::os_menu::{OsMenu, OsMenuItem};
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
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
