//! Help › System check (live's opens its WebGL check page): noshape's own, in a dialog. The system, its memory, the
//! documents folder's disk and the graphics adapter, a warning beside what makes noshape slow (a software renderer,
//! little memory or disk). Memory and disk are read off the UI thread, with a timeout (a network home can hang).

use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_scene::system_info as si;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);
const GB: u64 = 1 << 30;

/// What the background read found: memory, the disk; None while it runs, Err when it didn't answer in time.
type Found = Option<Result<(Option<(u64, Option<u64>)>, Option<(u64, u64)>), String>>;

pub fn open(window: &mut Window, cx: &mut App) {
    let adapter = cx.try_global::<crate::gpu::GpuGlobal>().and_then(|g| g.0.adapter_name());
    let folder = noshape_scene::documents_dir();
    let found: Rc<RefCell<Found>> = Rc::default();
    let (slot, shown, handle) = (found.clone(), folder.clone(), window.window_handle());
    let ex = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let read = ex.spawn(async move { (si::memory(), si::disk(&folder).or_else(|| folder.parent().and_then(si::disk))) });
        let r = read.with_timeout(TIMEOUT, &ex).await.map_err(|_| format!("didn't answer in {} s", TIMEOUT.as_secs()));
        *slot.borrow_mut() = Some(r);
        let _ = handle.update(cx, |_, window, _| window.refresh());
    })
    .detach();
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let row = |k: &'static str, v: String, warn: Option<&'static str>, cx: &App| {
            h_flex()
                .min_h(px(30.))
                .items_start()
                .gap(px(10.))
                .text_size(px(14.))
                .line_height(px(21.))
                .child(div().w(px(150.)).flex_none().font_weight(FontWeight::SEMIBOLD).child(k))
                .child(v_flex().flex_1().min_w_0().child(v).children(warn.map(|w| h_flex().items_start().gap(px(6.)).text_color(pal.text_muted).child(div().flex_none().pt(px(2.)).child(crate::assets::icon("warning", 16., cx))).child(div().flex_1().min_w_0().child(w)))))
        };
        let gib = |b: u64| format!("{:.1} GB", b as f64 / GB as f64);
        let software = adapter.as_deref().is_some_and(si::is_software_adapter);
        let mut rows = vec![
            row("System", format!("{} ({})", os_name(), std::env::consts::ARCH), None, cx),
            row("Processor", format!("{} threads", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)), None, cx),
            row("Graphics", adapter.clone().unwrap_or_else(|| "Starting…".into()), software.then_some("A software renderer: drawing is slow. Install your graphics card's driver."), cx),
        ];
        match &*found.borrow() {
            None => rows.push(row("Memory and disk", "Checking…".into(), None, cx)),
            Some(Err(e)) => rows.push(row("Memory and disk", format!("couldn't check ({e})"), None, cx)),
            Some(Ok((memory, disk))) => {
                rows.push(match memory {
                    Some((total, Some(available))) => row("Memory", format!("{} ({} available)", gib(*total), gib(*available)), (*total < 4 * GB).then_some("Less than 4 GB: large models may be slow."), cx),
                    Some((total, None)) => row("Memory", gib(*total), (*total < 4 * GB).then_some("Less than 4 GB: large models may be slow."), cx),
                    None => row("Memory", "unknown".into(), None, cx),
                });
                rows.push(match disk {
                    Some((free, total)) => row("Disk", format!("{} free of {} ({})", gib(*free), gib(*total), noshape_app_core::file_dialogs::display_path(&shown)), (*free < GB).then_some("Less than 1 GB free: documents may not save."), cx),
                    None => row("Disk", "unknown".into(), None, cx),
                });
            }
        }
        rows.push(row("Version", crate::VERSION.into(), None, cx));
        os::modal(dialog, "System check", 560.)
            .child(os::modal_body().children(rows))
            .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
    });
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    }
}
