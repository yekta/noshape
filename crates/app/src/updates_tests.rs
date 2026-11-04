//! The update card: nothing while there's nothing to say, then each phase floating at the window's bottom centre
//! (wide and narrow windows), hidden for the session by its ✕; and a list of documents to reopen read once.

use super::{Phase, Updates, card, reopen_list};
use gpui_kit::{AppContext as _, Bounds, IntoElement, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use noshape_update::Release;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

struct Slot;

impl Render for Slot {
    fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().relative().children(card(window, cx))
    }
}

fn set(cx: &mut TestAppContext, phase: Phase) {
    cx.update(|cx| {
        let release = Release { tag: "v0.4.21".into(), notes: "Faster fillets.".into(), page: "https://example.invalid/r".into(), assets: Vec::new() };
        cx.set_global(Updates { phase, release: Some(release), staged: None, progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))), failed: Vec::new(), dismissed: false });
    });
}

fn shows(cx: &mut TestAppContext, w: gpui_kit::AnyWindowHandle) -> bool {
    cx.update_window(w, |_, window, cx| card(window, cx).is_some()).unwrap()
}

#[gpui::test]
fn the_card_shows_each_phase_until_closed(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let open = |cx: &mut TestAppContext, width: f32| {
        let bounds = Bounds { origin: Point::default(), size: size(px(width), px(400.)) };
        cx.update(|cx| gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| Slot)).unwrap().0)
    };
    let (wide, narrow) = (open(cx, 1280.), open(cx, 360.));
    set(cx, Phase::Idle);
    assert!(!shows(cx, wide), "idle: no card");
    for phase in [Phase::Downloading, Phase::Ready, Phase::Manual, Phase::Failed("the signature doesn't match the file".into())] {
        set(cx, phase.clone());
        if phase == Phase::Downloading {
            cx.update(|cx| {
                let u = cx.global::<Updates>();
                u.progress.0.store(42, Ordering::Relaxed);
                u.progress.1.store(100, Ordering::Relaxed);
            });
        }
        for w in [wide, narrow] {
            assert!(shows(cx, w), "{phase:?}");
            // Drawn in the window (layout and paint of its parts).
            cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        }
    }
    // Its ✕: hidden until the next launch, whatever comes next.
    cx.update(|cx| cx.global_mut::<Updates>().dismissed = true);
    assert!(!shows(cx, wide));
    cx.update(|cx| cx.global_mut::<Updates>().phase = Phase::Ready);
    assert!(!shows(cx, narrow));
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

/// The settings the UI tests read are their own: another run's or an app's panel widths, columns and preferences on
/// the same machine made simulated clicks miss.
#[test]
fn ui_tests_have_their_own_settings_directory() {
    let dir = noshape_app_core::paths::config_dir();
    assert!(noshape_ui_test_support::is_not_the_users(&dir), "{dir:?} is the user's");
}
