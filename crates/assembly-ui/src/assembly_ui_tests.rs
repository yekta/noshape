//! UI tests of the Assembly view on GPUI's test platform (docs/TESTS.md "UI
//! keystroke tests"): the real view in a test window over a document made
//! through the agent API, the GPU a no-op stand-in.

use super::AssemblyView;
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_model::assembly::InstanceId;
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

/// A block (Part 1 <1>), a pin (Part 2 <1>) and a Linear pattern 1 of the
/// pin (its copy Part 2 <2>), in Assembly 1 of a test window.
fn pattern_assembly(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<AssemblyView>) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("asm.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "circle", "points": [[70, 10], [75, 10]] }] }));
        let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
        let e = call(&mut ctx, "assembly_entities", json!({ "instance": "Part 1 <1>", "kind": "edges", "direction": [1, 0, 0] }))["edges"][0]["id"].clone();
        call(&mut ctx, "assembly_pattern", json!({ "kind": "linear", "seeds": ["Part 2 <1>"], "direction": { "instance": "Part 1 <1>", "entity": e }, "distance": "30 mm" }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::Assembly(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            v
        })
        .unwrap();
        (w, view.unwrap())
    });
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
    (window, view)
}

fn instance(v: &AssemblyView, name: &str, cx: &gpui_kit::App) -> InstanceId {
    v.assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == name)).unwrap_or_else(|| panic!("no {name}")).id
}

/// Live 1681: in the Group dialog a pick on a pattern's copy is taken as its
/// pattern ("Linear pattern 1", no message); a second pick on it takes the
/// pattern out again; ✓ with only the pattern makes Group 1 of it, and
/// editing it shows the pattern again.
#[gpui::test]
fn a_group_takes_a_pattern_copy_as_its_pattern(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "group-pattern");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (seed, copy) = (instance(v, "Part 2 <1>", cx), instance(v, "Part 2 <2>", cx));
            let pat = v.assembly(cx).unwrap().patterns[0].id;
            v.selected.clear();
            v.create_group(cx);
            v.toggle_group_member(seed, cx);
            v.toggle_group_member(copy, cx);
            assert_eq!(v.ui.group.as_ref().map(|g| g.1.clone()), Some(vec![seed]));
            assert_eq!(v.ui.group_patterns, vec![pat]);
            v.toggle_group_member(copy, cx);
            assert!(v.ui.group_patterns.is_empty());
            v.toggle_group_member(copy, cx);
            v.toggle_group_member(seed, cx);
            v.accept_group(cx);
            assert!(v.ui.group.is_none() && v.message.is_none(), "{:?}", v.message);
            let g = v.assembly(cx).unwrap().groups[0].clone();
            assert_eq!((g.name.as_str(), g.instances.len(), g.patterns.clone()), ("Group 1", 0, vec![pat]));
            v.open_group(Some(g.id), g.instances, g.patterns, cx);
            assert_eq!(v.ui.group_patterns, vec![pat]);
        });
    })
    .unwrap();
}
