//! T2 bench: the Versions and history panel with 500 versions in 21 workspaces (docs/TESTS.md "UI benches"): its
//! frames, scrolling, hovering rows, opening a version and searching.

use super::HistoryPanel;
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, ParentElement as _, Point, Render, ScrollDelta, ScrollWheelEvent, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, point, px, size};
use gpui_kit::Focusable as _;
use noshape_ui_test_support::bench;
use serde_json::json;

const CASE: &str = "version graph (500 versions, 20 branches)";

struct Host(Entity<HistoryPanel>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().flex().child(div().w(px(420.)).h_full().flex().flex_col().child(self.0.clone())).child(div().flex_1())
    }
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
}

/// A document of 500 versions, each after a change: 300 in Main, then 20 workspaces branched from Main's versions,
/// 10 versions each.
fn document(path: &std::path::Path) {
    let mut ctx = noshape_ops::Ctx::new();
    let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
    call("document_create", json!({ "path": path.to_str().unwrap() }));
    let mut n = 0;
    let mut change_and_version = |call: &mut dyn FnMut(&str, serde_json::Value) -> serde_json::Value, prefix: &str, k: usize| {
        n += 1;
        call("variable_create", json!({ "name": format!("v{n}"), "value": format!("{n} mm") }));
        call("history_create_version", json!({ "name": format!("{prefix}{k}"), "description": format!("Release candidate {k}") }));
    };
    for k in 1..=300 {
        change_and_version(&mut call, "V", k);
    }
    for b in 1..=20 {
        call("history_create_branch", json!({ "name": format!("Feature {b}"), "version": format!("V{}", b * 15) }));
        for k in 1..=10 {
            change_and_version(&mut call, &format!("F{b}."), k);
        }
    }
}

fn panel(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<HistoryPanel>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| gpui_kit::component::Theme::global_mut(cx).font_family = "Roboto".into());
    let dir = std::env::temp_dir().join(format!("noshape-history-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("graph.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let t = std::time::Instant::now();
    document(&path);
    println!("BENCH {CASE} | (document made in {:.1} s)", t.elapsed().as_secs_f64());
    let session = crate::session::Session::open(&path).unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| HistoryPanel::new(doc, window, cx));
            view = Some(v.clone());
            cx.new(|_| Host(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    cx.run_until_parked();
    (window, view)
}

#[test]
#[ignore = "T2: bench, the version graph at 500 versions (prints BENCH lines)"]
fn version_graph_at_scale() {
    bench::run(|cx| {
        let (window, view) = panel(cx);
        let mut fits = Vec::new();
        let mut step = |name: &str, t: bench::Times| fits.push((name.to_string(), bench::report(CASE, name, &t)));
        step("a frame of the panel", bench::time(20, || {
            cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
            draw(cx, window);
        }));
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let over = point(px(200.), px(500.));
        step("scroll (120 px a step)", bench::time(40, || {
            vcx.simulate_event(ScrollWheelEvent { position: over, delta: ScrollDelta::Pixels(point(px(0.), px(-120.))), ..Default::default() });
            vcx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
        }));
        let mut y = 200.;
        step("hover across rows", bench::time(40, || {
            y = if y > 850. { 200. } else { y + 25. };
            vcx.simulate_mouse_move(point(px(200.), px(y)), None, Modifiers::default());
            vcx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
        }));
        // A click on a version row: its handler and frame (the window's own events, so the version's read, a
        // background task, isn't run in the step), then the read and the version shown.
        let (mut y, mut opened) = (250., Vec::new());
        let click = bench::time(10, || {
            y = if y > 800. { 250. } else { y + 50. };
            let at = point(px(200.), px(y));
            cx.update_window(window, |_, w, cx| {
                w.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }), cx);
                w.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1 }), cx);
                w.draw(cx).clear(cx);
            })
            .unwrap();
            let t = std::time::Instant::now();
            cx.run_until_parked();
            draw(cx, window);
            opened.push(t.elapsed().as_secs_f64() * 1e3);
        });
        step("a click on a version row (it opens)", click);
        opened.sort_by(f64::total_cmp);
        println!("BENCH {CASE} | the version read (background) and shown: median {:.1} ms", opened[opened.len() / 2]);
        cx.update_window(window, |_, w, cx| {
            let fh = view.read(cx).search.focus_handle(cx);
            fh.focus(w, cx);
        })
        .unwrap();
        let keys = ["r", "e", "l", "enter", "backspace", "backspace", "backspace", "enter"];
        let mut k = keys.iter().cycle();
        step("search: a key typed (Enter searches)", bench::time(16, || {
            let key = *k.next().unwrap();
            cx.update_window(window, |_, w, cx| {
                let mut ks = Keystroke::parse(key).unwrap();
                if key.len() == 1 {
                    ks.key_char = Some(key.into());
                }
                w.dispatch_keystroke(ks, cx);
                w.draw(cx).clear(cx);
            })
            .unwrap();
        }));
        let slow: Vec<&str> = fits.iter().filter(|(_, ok)| !ok).map(|(n, _)| n.as_str()).collect();
        assert!(slow.is_empty(), "over a frame: {slow:?}");
    });
}
