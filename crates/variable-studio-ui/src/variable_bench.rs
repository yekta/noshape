//! T2 bench: a Variable Studio of 500 variables (docs/TESTS.md "UI benches"): its frames, scrolling, the arrow keys
//! through its cells, typing a value and committing it.

use super::VariableStudioView;
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Keystroke, ParentElement as _, Point, Render, ScrollDelta, ScrollWheelEvent, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, point, px, size};
use noshape_ui_test_support::bench;
use serde_json::json;

const CASE: &str = "Variable Studio (500 variables)";

struct Host(Entity<VariableStudioView>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().flex().flex_col().child(self.0.clone())
    }
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
}

fn key(cx: &mut TestAppContext, window: AnyWindowHandle, key: &str) {
    cx.update_window(window, |_, w, cx| {
        let mut ks = Keystroke::parse(key).unwrap();
        if key.len() == 1 {
            ks.key_char = Some(key.into());
        }
        w.dispatch_keystroke(ks, cx);
        w.draw(cx).clear(cx);
    })
    .unwrap();
}

fn studio(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<VariableStudioView>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| gpui_kit::component::Theme::global_mut(cx).font_family = "Roboto".into());
    let dir = std::env::temp_dir().join(format!("noshape-variables-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("variables.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = noshape_ops::Ctx::new();
        let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call("document_create", json!({ "path": path.to_str().unwrap() }));
        let tab = call("tab_create", json!({ "kind": "variablestudio" }));
        let tab = tab["tab"]["name"].as_str().unwrap().to_string();
        call("variable_create", json!({ "tab": tab, "name": "v1", "value": "10 mm", "description": "Base length" }));
        for n in 2..=500 {
            call("variable_create", json!({ "tab": tab, "name": format!("v{n}"), "value": format!("#v{} + 1 mm", n - 1), "description": format!("Offset {n}") }));
        }
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::VariableStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| VariableStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Host(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    cx.run_until_parked();
    draw(cx, window);
    (window, view)
}

#[test]
#[ignore = "T2: bench, a Variable Studio of 500 variables (prints BENCH lines)"]
fn variable_studio_at_scale() {
    bench::run(|cx| {
        let (window, view) = studio(cx);
        assert_eq!(cx.update(|cx| view.read(cx).studio(cx).map(|s| s.variables.len())), Some(500));
        let mut fits = Vec::new();
        let mut step = |name: &str, t: bench::Times| fits.push((name.to_string(), bench::report(CASE, name, &t)));
        step("a frame of the table", bench::time(20, || {
            cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
            draw(cx, window);
        }));
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let over = point(px(800.), px(500.));
        step("scroll (120 px a step)", bench::time(40, || {
            vcx.simulate_event(ScrollWheelEvent { position: over, delta: ScrollDelta::Pixels(point(px(0.), px(-120.))), ..Default::default() });
            vcx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
        }));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.select_cell("e2", w, cx))).unwrap();
        draw(cx, window);
        step("↓ to the next value cell", bench::time(30, || key(cx, window, "down")));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_cell_edit("e2", Some("2".into()), w, cx))).unwrap();
        draw(cx, window);
        let keys = ["0", " ", "m", "m", "backspace", "backspace", "backspace"];
        let mut k = keys.iter().cycle();
        step("a key typed in a value", bench::time(21, || key(cx, window, k.next().unwrap())));
        // Enter in an open editor: the key (the document's edit, every later value following) and the frame after
        // it. (The editor opens with the first typed key: its own step above.)
        let (mut opens, mut enters) = (Vec::new(), Vec::new());
        for n in 1..=10 {
            let t = std::time::Instant::now();
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_cell_edit("e1", Some(format!("{} mm", 10 + n)), w, cx))).unwrap();
            draw(cx, window);
            opens.push(t.elapsed().as_secs_f64() * 1e3);
            let t = std::time::Instant::now();
            cx.update_window(window, |_, w, cx| w.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx)).unwrap();
            draw(cx, window);
            enters.push(t.elapsed().as_secs_f64() * 1e3);
        }
        step("a value's editor opened by a key", bench::Times::from_ms(opens));
        step("Enter commits a value (every later one follows)", bench::Times::from_ms(enters));
        let slow: Vec<&str> = fits.iter().filter(|(_, ok)| !ok).map(|(n, _)| n.as_str()).collect();
        assert!(slow.is_empty(), "over a frame: {slow:?}");
    });
}
