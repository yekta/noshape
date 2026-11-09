//! Help › System check: a page of its own, in its own window (live's opens its check page in a new browser tab). The
//! system, its memory, the documents folder's disk and the graphics adapter, a warning beside what makes Noshape slow (a
//! software renderer, little memory or disk). Memory and disk are read off the UI thread, with a timeout (a network
//! home can hang).

use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_scene::system_info as si;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);
const GB: u64 = 1 << 30;

/// What the background read found: memory, the disk; None while it runs, Err when it didn't answer in time.
type Found = Option<Result<(Option<(u64, Option<u64>)>, Option<(u64, u64)>), String>>;

/// The open page's window: a second System check brings it to the front and checks again.
struct SystemCheckWindow {
    handle: AnyWindowHandle,
    view: Entity<SystemCheckPage>,
}

impl Global for SystemCheckWindow {}

struct SystemCheckPage {
    adapter: Option<String>,
    found: Found,
    _read: Option<Task<()>>,
}

impl SystemCheckPage {
    fn check(&mut self, cx: &mut Context<Self>) {
        self.adapter = cx.try_global::<crate::gpu::GpuGlobal>().and_then(|g| g.0.adapter_name());
        self.found = None;
        let folder = noshape_scene::documents_dir();
        let ex = cx.background_executor().clone();
        self._read = Some(cx.spawn(async move |this, cx| {
            let read = ex.spawn(async move { (si::memory(), si::disk(&folder).or_else(|| folder.parent().and_then(si::disk))) });
            let r = read.with_timeout(TIMEOUT, &ex).await.map_err(|_| format!("didn't answer in {} s", TIMEOUT.as_secs()));
            let _ = this.update(cx, |this, cx| {
                this.found = Some(r);
                cx.notify();
            });
        }));
        cx.notify();
    }
}

pub fn open(window: &mut Window, cx: &mut App) {
    if let Some(g) = cx.try_global::<SystemCheckWindow>() {
        let (handle, view) = (g.handle, g.view.clone());
        if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
            view.update(cx, |v, cx| v.check(cx));
            return;
        }
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, crate::shortcuts_page::page_size(Some(window.viewport_size())), cx))),
        window_min_size: Some(size(px(480.), px(320.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..gpui_kit::component::TitleBar::window_options()
    };
    match crate::os_menu::open_window(options, cx, |window, cx| {
        window.set_window_title("System check — Noshape");
        cx.new(|cx| {
            let mut page = SystemCheckPage { adapter: None, found: None, _read: None };
            page.check(cx);
            page
        })
    }) {
        Ok((handle, view)) => cx.set_global(SystemCheckWindow { handle, view }),
        Err(e) => {
            let message = format!("Could not open System check: {e:#}");
            window.open_dialog(cx, move |dialog, _, cx| {
                crate::os_ui::modal(dialog, "System check", 460.)
                    .child(crate::os_ui::modal_body().child(crate::os_ui::info_box(message.clone())))
                    .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("close", "Close", cx))))
            });
        }
    }
}

impl Render for SystemCheckPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let software = self.adapter.as_deref().is_some_and(si::is_software_adapter);
        let mut rows = vec![
            row("System", format!("{} ({})", os_name(), std::env::consts::ARCH), None, cx),
            row("Processor", format!("{} threads", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)), None, cx),
            row("Graphics", self.adapter.clone().unwrap_or_else(|| "Starting…".into()), software.then_some("A software renderer: drawing is slow. Install your graphics card's driver."), cx),
        ];
        match &self.found {
            None => rows.push(row("Memory and disk", "Checking…".into(), None, cx)),
            Some(Err(e)) => rows.push(row("Memory and disk", format!("couldn't check ({e})"), None, cx)),
            Some(Ok((memory, disk))) => {
                rows.push(match memory {
                    Some((total, Some(available))) => row("Memory", format!("{} ({} available)", gib(*total), gib(*available)), (*total < 4 * GB).then_some("Less than 4 GB: large models may be slow."), cx),
                    Some((total, None)) => row("Memory", gib(*total), (*total < 4 * GB).then_some("Less than 4 GB: large models may be slow."), cx),
                    None => row("Memory", "unknown".into(), None, cx),
                });
                let shown = noshape_scene::documents_dir();
                rows.push(match disk {
                    Some((free, total)) => row("Disk", format!("{} free of {} ({})", gib(*free), gib(*total), noshape_app_core::file_dialogs::display_path(&shown)), (*free < GB).then_some("Less than 1 GB free: documents may not save."), cx),
                    None => row("Disk", "unknown".into(), None, cx),
                });
            }
        }
        rows.push(row("Version", crate::VERSION.into(), None, cx));
        v_flex()
            .size_full()
            .bg(gpui_kit::rgb(0xffffff))
            .text_color(pal.text)
            .child(gpui_kit::component::TitleBar::new().bg(pal.panel_inactive).border_b_1().border_color(pal.border).child(div().pl(px(8.)).text_size(px(12.)).text_color(pal.text_muted).child("System check — Noshape")))
            .child(
                div().id("system-check").flex_1().min_h_0().overflow_y_scroll().child(
                    v_flex().max_w(px(720.)).px(px(32.)).py(px(24.)).gap(px(4.)).child(div().pb(px(12.)).text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child("System check")).children(rows),
                ),
            )
    }
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    }
}
