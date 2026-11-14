//! T2 bench: the Feature Studio editor on a 10,000-line file (docs/TESTS.md "UI benches"): opening it, its frames,
//! scrolling, typing in the middle, std's completion list as a name is typed, and the error squiggles.

use super::FeatureStudioView;
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Keystroke, ParentElement as _, Point, Render, ScrollDelta, ScrollWheelEvent, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, point, px, size};
use noshape_ui_test_support::bench;
use serde_json::json;
use std::time::{Duration, Instant};

const CASE: &str = "Feature Studio (10,000 lines)";

struct Host(Entity<FeatureStudioView>);

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

/// A FeatureScript file of 10,000 lines: std's geometry import, a feature, then helper functions.
fn source() -> String {
    let mut s = String::from("FeatureScript 2581;\nimport(path : \"onshape/std/geometry.fs\", version : \"2581.0\");\n\n");
    s += "annotation { \"Feature Type Name\" : \"Bench feature\" }\nexport const benchFeature = defineFeature(function(context is Context, id is Id, definition is map)\n    precondition\n    {\n        annotation { \"Name\" : \"Length\" }\n        isLength(definition.length, LENGTH_BOUNDS);\n    }\n    {\n        var total = helper0(3, 4);\n        println(total);\n    });\n\n";
    let mut i = 0;
    while s.lines().count() < 10_000 - 14 {
        s += &format!("/**\n * Sums {i} over a grid.\n */\nfunction helper{i}(a is number, b is number) returns number\n{{\n    var total = 0;\n    for (var k = 0; k < a; k += 1)\n    {{\n        total += k * b + {i};\n    }}\n    return total > 100 ? total / 2 : total;\n}}\n\n");
        i += 1;
    }
    while s.lines().count() < 10_000 {
        s += "// end\n";
    }
    s
}

fn studio(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<FeatureStudioView>, f64) {
    cx.update(gpui_kit::init);
    cx.update(|cx| gpui_kit::component::Theme::global_mut(cx).font_family = "Roboto".into());
    let dir = std::env::temp_dir().join(format!("noshape-fs-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fs.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = noshape_ops::Ctx::new();
        let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call("document_create", json!({ "path": path.to_str().unwrap() }));
        call("tab_create", json!({ "kind": "featurestudio" }));
        call("featurestudio_edit", json!({ "source": source() }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::FeatureStudio(_))).unwrap().id;
    let t = Instant::now();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Host(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    let open = t.elapsed().as_secs_f64() * 1e3;
    (window, view, open)
}

#[test]
#[ignore = "T2: bench, the Feature Studio on a 10,000-line file (prints BENCH lines)"]
fn feature_studio_at_scale() {
    bench::run(|cx| {
        let (window, view, open) = studio(cx);
        let mut fits = Vec::new();
        let mut step = |name: &str, t: bench::Times| fits.push((name.to_string(), bench::report(CASE, name, &t)));
        println!("BENCH {CASE} | open (the view and its first frame): {open:.1} ms");
        let t = Instant::now();
        cx.run_until_parked();
        println!("BENCH {CASE} | its compile (background): {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        step("a frame of the code", bench::time(20, || {
            cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
            draw(cx, window);
        }));
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let over = point(px(800.), px(500.));
        step("scroll (120 px a step)", bench::time(40, || {
            vcx.simulate_event(ScrollWheelEvent { position: over, delta: ScrollDelta::Pixels(point(px(0.), px(-120.))), ..Default::default() });
            vcx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
        }));
        // The caret in the middle (line 5,000, in a function's body), then typing.
        let editor = cx.update(|cx| view.read(cx).editor.clone());
        let line = cx.update(|cx| editor.read(cx).value().split('\n').enumerate().skip(5000).find(|(_, l)| l.trim_start().starts_with("var total")).map(|(i, _)| i as u32).unwrap());
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.set_cursor_position(gpui_kit::base::input::Position { line: line + 1, character: 0 }, w, cx))).unwrap();
        draw(cx, window);
        let keys = ["enter", "v", "a", "r", "space", "x", "space", "=", "space", "1", ";"];
        let mut k = keys.iter().cycle();
        step("a key typed at line 5,000", bench::time(33, || key(cx, window, k.next().unwrap())));
        // std's list as a name is typed: each key until the list shows it.
        key(cx, window, "enter");
        cx.run_until_parked();
        let mut word = "opExtrude".chars().map(|c| c.to_string()).cycle();
        let (mut typed, mut rows_made) = (Vec::new(), Vec::new());
        for n in 1..=18 {
            if n % 9 == 0 {
                for _ in 0..8 {
                    key(cx, window, "backspace");
                }
                cx.run_until_parked();
            }
            let t = Instant::now();
            key(cx, window, &word.next().unwrap());
            typed.push(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            cx.executor().advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
            draw(cx, window);
            rows_made.push(t.elapsed().as_secs_f64() * 1e3);
        }
        step("completion: a key typed (its rows made off the UI thread)", bench::Times::from_ms(typed));
        rows_made.sort_by(f64::total_cmp);
        println!("BENCH {CASE} | completion: the list's rows made (background) and drawn: median {:.1} ms", rows_made[rows_made.len() / 2]);
        key(cx, window, "escape");
        // An error: a stray brace; the squiggles come with the compile after the save.
        key(cx, window, "}");
        let t = Instant::now();
        cx.executor().advance_clock(Duration::from_millis(800));
        cx.run_until_parked();
        draw(cx, window);
        println!("BENCH {CASE} | save and compile with the error (background): {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        let errors = cx.update(|cx| view.read(cx).notices.iter().filter(|n| n.error).count());
        assert!(errors > 0, "the stray brace is reported");
        step("a frame with the squiggles", bench::time(20, || {
            cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
            draw(cx, window);
        }));
        let mut m = 0;
        step("a key typed with the squiggles", bench::time(20, || {
            m += 1;
            key(cx, window, if m % 2 == 1 { "a" } else { "backspace" });
        }));
        let slow: Vec<&str> = fits.iter().filter(|(_, ok)| !ok).map(|(n, _)| n.as_str()).collect();
        assert!(slow.is_empty(), "over a frame: {slow:?}");
    });
}
