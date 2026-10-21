//! Onshape's workspace protection (help "Workspace Protections"): a
//! protected workspace takes no direct model edits (geometry, drawings,
//! tabs); changes are made on a branch and merged in. Versions, branches
//! and merges stay allowed. The check is `Session::is_protected` (ops); here
//! the Versions and history row's "Workspace protection…" dialog and the
//! banner shown while the protected workspace is active.
//!
//! The dialog follows live wp-902; the banner follows the help text (its
//! live capture is still open, REQUESTS ui2 10-20 23:30).

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_store::BranchId;
use std::cell::Cell;
use std::rc::Rc;

/// Workspace row › Workspace protection… (live wp-902,
/// dom/ui2-workspace-protection-dialog.html): a 600 px modal "Workspace
/// protection for [name]" (the name in a yellow-outlined chip), ☐ Enable
/// workspace protection, the blue "Workspace protection restrictions" box
/// and [Apply] (enabled once changed) [Cancel]. Live's yellow "These tabs
/// should be updated: …" notice (which disables the checkbox) needs a check
/// for out-of-date drawings we don't have yet.
pub fn open_dialog(doc: Entity<DocModel>, branch: BranchId, window: &mut Window, cx: &mut App) {
    let saved = doc.read(cx).session.branch_protected(branch);
    let name = doc.read(cx).session.store.branch(branch).map(|b| b.name).unwrap_or_default();
    let on = Rc::new(Cell::new(saved));
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (doc, on2) = (doc.clone(), on.clone());
        let changed = on.get() != saved;
        let info = rgb(0x094174);
        let bullets = |items: &[&'static str]| v_flex().pl(px(28.)).children(items.iter().map(|t| h_flex().gap(px(10.)).child(div().child("•")).child(div().child(*t))));
        let title = h_flex()
            .w(px(598.))
            .h(px(48.))
            .pl(px(10.))
            .pr(px(12.))
            .gap(px(6.))
            .border_b_1()
            .border_color(rgb(os::MODAL_DIVIDER))
            .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Workspace protection for"))
            .child(div().px(px(4.)).rounded(px(3.)).border_1().border_color(rgb(0xfab601)).bg(rgb(0xfff4d6)).text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(name.clone()))
            .child(div().flex_1())
            .child(div().flex_none().child(gpui_kit::component::dialog::DialogClose::new().child(div().id("modal-close").size(px(22.)).flex().items_center().justify_center().cursor_pointer().text_color(pal.text_faint).child(gpui_kit::component::Icon::empty().data(os::GLYPH_CLOSE).size(px(16.))))));
        os::modal(dialog, "", 600.)
            .title(title)
            .child(
                v_flex()
                    .px(px(11.))
                    .pt(px(4.))
                    .pb(px(20.))
                    .gap(px(10.))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child({
                        let on = on.clone();
                        h_flex()
                            .id("ws-protect")
                            .h(px(26.))
                            .gap(px(6.))
                            .cursor_pointer()
                            .child(div().size(px(13.)).flex_none().flex().items_center().justify_center().rounded(px(2.)).border_1().border_color(if on.get() { rgb(0x0075ff) } else { rgb(0x767676) }).bg(if on.get() { rgb(0x0075ff) } else { rgb(0xffffff) }).when(on.get(), |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(rgb(0xffffff)))))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Enable workspace protection"))
                            .on_click(move |_, window, _| {
                                on.set(!on.get());
                                window.refresh();
                            })
                    })
                    .child(
                        v_flex()
                            .px(px(15.))
                            .py(px(14.))
                            .rounded(px(3.))
                            .bg(pal.info_fill)
                            .border_1()
                            .border_color(pal.info_outline)
                            .line_height(px(21.))
                            .text_color(info)
                            .child(div().font_weight(FontWeight::BOLD).child("Workspace protection restrictions"))
                            .child(div().child("Workspace protection applies restrictions to how a workspace can be edited. This option currently restricts the following actions in a workspace:"))
                            .child(bullets(&["Restrict geometry changes to merges only", "Restrict drawings changes to merges only", "Restrict metadata changes to merges only", "Restrict adding new tabs to merges only"]))
                            .child(div().mt(px(14.)).child("Actions that are allowed in this workspace:"))
                            .child(bullets(&["Release management", "Refresh drawing updates", "Version creation, branching and merging"]))
                            .child(
                                div()
                                    .id("ws-protect-learn")
                                    .mt(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .underline()
                                    .cursor_pointer()
                                    .child("Learn more about workspace protection")
                                    .on_click(|_, _, cx| cx.open_url("https://github.com/yekta/noshape#readme")),
                            ),
                    ),
            )
            // Live: Apply stays pale until the box changes; Enter does the same.
            .footer(os::modal_footer().child(os::modal_ok(div().when(!changed, |d| d.opacity(0.4)).child(os::primary_button("ok", "Apply")))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                let v = on2.get();
                if v == saved {
                    return false;
                }
                let r = doc.update(cx, |d, _| d.session.set_branch_protected(branch, v));
                doc.update(cx, |d, cx| match r {
                    Ok(()) => d.reloaded(cx),
                    Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
                });
                true
            })
    });
}

/// The message on a protected workspace's drawings (live wp-928).
const BANNER_MESSAGE: &str = "Workspace protection enabled.  Changes must be merged in from another workspace.";

/// While the protected workspace is active, Onshape's view-only-mode bar in
/// the toolbar row (live wp-928 / 930: white, 32 px, rounded below, 14 px
/// `#333` / 600): a Part Studio shows the read-only document's "This
/// document is view only. [Make a copy to edit] (?)", a drawing (and the
/// other tabs) "Workspace protection enabled.  Changes must be merged in
/// from another workspace." (501 px, truncated) and (?). workspace.rs puts
/// it over the toolbar row.
pub fn render_banner(doc: &Entity<DocModel>, part_studio: bool, cx: &App) -> Option<AnyElement> {
    let s = &doc.read(cx).session;
    if !s.is_protected() {
        return None;
    }
    let pal = p();
    let help = div().size(px(18.)).flex_none().rounded_full().bg(rgb(0x999999)).flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(rgb(0xffffff)).child("?");
    let bar = h_flex().id("protected-banner").h(px(32.)).px(px(20.)).gap(px(10.)).bg(rgb(0xffffff)).border_1().border_t_0().border_color(rgb(0xcccccc)).rounded_b(px(6.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x333333));
    Some(if part_studio {
        let d = doc.clone();
        bar.child("This document is view only.")
            .child(
                div()
                    .id("protected-copy")
                    .h(px(24.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(3.))
                    .bg(pal.accent)
                    .cursor_pointer()
                    .text_size(px(12.))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(rgb(0xffffff))
                    .child("Make a copy to edit")
                    .on_click(move |_, window, cx| crate::doc_dialogs::copy_workspace(d.clone(), window, cx)),
            )
            .child(help)
            .into_any_element()
    } else {
        bar.max_w(px(501.)).child(div().min_w_0().truncate().child(BANNER_MESSAGE)).child(help).into_any_element()
    })
}
