//! The update banner: nothing while there's nothing to say, then each phase's text in its slot, and a list of
//! documents to reopen read once.

use super::{Phase, Updates, banner, reopen_list};
use gpui_kit::{AppContext as _, Bounds, IntoElement, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use noshape_update::Release;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

struct Slot;

impl Render for Slot {
    fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().children(banner(cx))
    }
}

fn set(cx: &mut TestAppContext, phase: Phase) {
    cx.update(|cx| {
        let release = Release { tag: "v0.4.21".into(), notes: "Faster fillets.".into(), page: "https://example.invalid/r".into(), assets: Vec::new() };
        cx.set_global(Updates { phase, release: Some(release), staged: None, progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))), failed: Vec::new() });
    });
}

fn shows(cx: &mut TestAppContext) -> bool {
    cx.update(|cx| banner(cx).is_some())
}

#[gpui::test]
fn the_banner_shows_each_phase(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    set(cx, Phase::Idle);
    assert!(!shows(cx), "idle: an empty slot");
    let bounds = Bounds { origin: Point::default(), size: size(px(900.), px(200.)) };
    let w = cx.update(|cx| gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| Slot)).unwrap().0);
    for phase in [Phase::Downloading, Phase::Ready, Phase::Manual, Phase::Failed("the signature doesn't match the file".into())] {
        set(cx, phase.clone());
        if phase == Phase::Downloading {
            cx.update(|cx| {
                let u = cx.global::<Updates>();
                u.progress.0.store(42, Ordering::Relaxed);
                u.progress.1.store(100, Ordering::Relaxed);
            });
        }
        assert!(shows(cx), "{phase:?}");
        // Drawn in a window (layout and paint of its parts).
        cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    }
}

#[test]
fn the_reopen_list_is_read_once() {
    let dir = std::env::temp_dir().join(format!("noshape-reopen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("a.noshape");
    std::fs::write(&doc, "").unwrap();
    let list = dir.join("reopen.json");
    std::fs::write(&list, serde_json::to_vec(&[doc.display().to_string(), dir.join("gone.noshape").display().to_string()]).unwrap()).unwrap();
    assert_eq!(reopen_list(&list), vec![doc.clone()], "documents that still exist");
    assert!(!list.exists(), "read once");
    let _ = std::fs::remove_dir_all(dir);
}
