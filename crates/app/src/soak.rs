//! Memory over repeated opens (`NOSHAPE_SOAK=<cycles>`, lead 11-16): the document on the command line is opened,
//! shown until its Part Studio has drawn and settled, closed, and opened again, `cycles` times, then the app quits.
//! Each cycle logs `SOAK <n> rss <MB>` and whether the closed workspace, document model, Part Studio view and its
//! viewport are freed, to stderr after the close.

use crate::gpu::Gpu;
use gpui_kit::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn cycles() -> Option<usize> {
    std::env::var("NOSHAPE_SOAK").ok()?.parse().ok()
}

/// This process's resident memory, MB (`ps`, as Activity Monitor's and top's RSS).
fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output();
    out.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok()).map_or(0.0, |kb| kb / 1024.0)
}

pub fn start(path: PathBuf, cycles: usize, gpu: Gpu, cx: &mut App) {
    cx.spawn(async move |cx| {
        let wait = async |cx: &mut AsyncApp, ms: u64| cx.background_executor().timer(Duration::from_millis(ms)).await;
        eprintln!("SOAK 0 rss {:.1}", rss_mb());
        for n in 1..=cycles {
            let session = match cx.background_spawn({
                let p = path.clone();
                async move { crate::session::Session::open(&p) }
            })
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("SOAK FAIL: {e:#}");
                    noshape_kernel::exit_now(2);
                }
            };
            let Ok((window, ws)) = cx.update(|cx| crate::files::open_session_window(session, gpu.clone(), cx)) else {
                eprintln!("SOAK FAIL: no window");
                noshape_kernel::exit_now(2);
            };
            // Shown: a frame presented and no regeneration or scene build left, for half a second.
            let t0 = Instant::now();
            let mut settled = 0;
            while settled < 10 && t0.elapsed() < Duration::from_secs(60) {
                wait(cx, 50).await;
                let ready = cx.update(|cx| {
                    ws.read(cx).active_part_studio().is_some_and(|ps| {
                        let v = ps.read(cx);
                        !v.ui_busy() && v.viewport_entity().read(cx).frame_counts().0 > 0
                    })
                });
                settled = if ready { settled + 1 } else { 0 };
            }
            let shown_ms = t0.elapsed().as_millis();
            let (weak_ws, weak_doc, weak_ps, weak_vp) = cx.update(|cx| {
                let ps = ws.read(cx).active_part_studio();
                let vp = ps.as_ref().map(|p| p.read(cx).viewport_entity().downgrade());
                (ws.downgrade(), ws.read(cx).doc.downgrade(), ps.map(|p| p.downgrade()), vp)
            });
            drop(ws);
            let _ = cx.update_window(window, |_, window, _| window.remove_window());
            wait(cx, 1000).await;
            let state = |alive: bool| if alive { "alive" } else { "freed" };
            let (ws_alive, doc_alive) = (weak_ws.upgrade().is_some(), weak_doc.upgrade().is_some());
            let ps_alive = weak_ps.is_some_and(|w| w.upgrade().is_some());
            let vp_alive = weak_vp.is_some_and(|w| w.upgrade().is_some());
            eprintln!("SOAK {n} rss {:.1} workspace {} document {} part studio {} viewport {} shown {shown_ms} ms", rss_mb(), state(ws_alive), state(doc_alive), state(ps_alive), state(vp_alive));
            // (A moment between cycles to look at the process from outside: vmmap, heap.)
            wait(cx, 2000).await;
        }
        noshape_kernel::exit_now(0);
    })
    .detach();
}
