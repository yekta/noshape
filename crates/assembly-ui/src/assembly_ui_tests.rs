//! UI tests of the Assembly view on GPUI's test platform (docs/TESTS.md "UI
//! keystroke tests"): the real view in a test window over a document made
//! through the agent API, the GPU a no-op stand-in.

use super::AssemblyView;
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_model::assembly::{InstanceId, MateConnectorRef, MateType};
use noshape_model::reference::{Inference, Reference};
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
    // The evaluation runs in the background after a short timer.
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
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

/// An implicit connector at the centroid of the top (+Z) or bottom (−Z)
/// face of an instance's part.
fn face_connector(v: &AssemblyView, name: &str, up: bool, cx: &gpui_kit::App) -> MateConnectorRef {
    let id = instance(v, name, cx);
    let occ = v.result.occurrences.iter().find(|o| o.top() == id).unwrap_or_else(|| panic!("{name} not evaluated"));
    let body = occ.context.body(&occ.body).unwrap();
    let side = if up { 1.0 } else { -1.0 };
    let k = body.shape.faces().iter().position(|f| f.face_plane().ok().flatten().is_some_and(|p| p.unit_normal().z * side > 0.99)).unwrap();
    MateConnectorRef::on(id, Vec::new(), Reference::Implicit { on: Box::new(occ.face_refs[k].clone()), inference: Inference::Centroid })
}

/// A new mate's second pick fills the dialog and puts up the heads-up ✓
/// (it stopped when the last field started keeping the focus, 10-31: the
/// dialog then still awaits picks, so "complete" is its own test).
#[gpui::test]
fn a_mate_gets_its_heads_up_check_after_the_second_pick(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "mate-hud");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Fastened, w, cx);
            let (a, b) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
            v.deliver_connector(a, w, cx);
            assert!(v.mate.as_ref().is_some_and(|s| s.hud.is_none() && !s.dialog.read(cx).connectors_complete()));
            v.deliver_connector(b, w, cx);
            let s = v.mate.as_ref().unwrap();
            assert!(s.dialog.read(cx).connectors_complete() && s.hud.is_some());
        });
    })
    .unwrap();
}

/// A menu as its labels: "—" for a separator, "Label ▸" for a submenu;
/// separators collapse as `fill_menu` draws them.
fn labels(entries: &[super::MenuEntry]) -> Vec<String> {
    use super::MenuEntry;
    let mut out: Vec<String> = Vec::new();
    for e in entries {
        let l = match e {
            MenuEntry::Item { label, .. } => label.clone(),
            MenuEntry::Sub { label, .. } => format!("{label} ▸"),
            MenuEntry::Sep if out.last().is_none_or(|l| l == "—") => continue,
            MenuEntry::Sep => "—".into(),
        };
        out.push(l);
    }
    while out.last().is_some_and(|l| l == "—") {
        out.pop();
    }
    out
}

/// Live's menus (dom/context-menus-assembly.txt, behaviours 1222) for a
/// part instance in the list and in the graphics and for the top row, less
/// "Add comment" (collaboration, cloud) and the separator it sits between.
#[gpui::test]
fn the_instance_and_top_row_menus_are_lives(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "menus");
    let sep = "—";
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let p1 = instance(v, "Part 1 <1>", cx);
            v.selected.clear();
            // rmb-asm-list-instance.
            assert_eq!(
                labels(&v.instance_menu(p1, false, cx)),
                [
                    "Properties…", "Instance properties…", sep,
                    "Hide", "Hide other instances", "Hide all instances", "Show center of mass", "Isolate…", "Make transparent…", "Suppress", "Dynamic suppression ▸", "Fix", "Use best available tessellation", sep,
                    "Edit in context", "Add mate connector to instance origin…", "Replace instances…", "Switch to Part Studio 1", sep,
                    "Copy Part 1 <1>", sep,
                    "Move to new subassembly", "Change to version…", "Export…", "Release…", sep,
                    "Zoom to selection", sep,
                    "Create Drawing of Part 1…", sep,
                    "Add selection to folder…", "Create empty subassembly", sep,
                    "Delete",
                ]
            );
            // rmb-asm-two-instances-selected (one instance, nothing selected
            // before; "View normal to" needs a face under the pointer).
            assert_eq!(
                labels(&v.instance_menu(p1, true, cx)),
                [
                    "Hide", "Hide other instances", "Hide all instances", "Show center of mass", "Isolate…", "Make transparent…", "Section view…", "Suppress Part 1 <1>", "Fix", "Use best available tessellation", sep,
                    "Copy Part 1 <1>", sep,
                    "Create Drawing of Part 1…", "Create Drawing of Assembly 1…", "Export…", sep,
                    "Edit in context", "Find in instance list", "Add mate connector to instance origin…", "Replace Part 1 <1>…", "Switch to Part Studio 1", sep,
                    "Select other…", sep,
                    "Zoom to fit", "Zoom to selection", sep,
                    "Delete Part 1 <1>",
                ]
            );
            // rmb-asm-list-top-assembly.
            assert_eq!(
                labels(&v.assembly_menu(cx)),
                [
                    "Properties…", "Show instance list properties", sep,
                    "Rename", "Show center of mass", "Check interference…", sep,
                    "Create Drawing of Assembly 1…", "Add exploded view…", sep,
                    "Export…", "Release…", sep,
                    "Expand all", "Collapse all",
                ]
            );
        })
    })
    .unwrap();
}

/// Live 1723: a Linear pattern's row and its copy's row (less "Add comment",
/// cloud, and the pattern's Dynamic suppression, which patterns don't have
/// here yet).
#[gpui::test]
fn the_pattern_and_copy_row_menus_are_lives(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "pattern-menus");
    let sep = "—";
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let pat = v.assembly(cx).unwrap().patterns[0].id;
            assert_eq!(
                labels(&v.pattern_menu(pat, cx)),
                [
                    "Rename", "Edit…", "Hide", "Hide other instances", "Hide all instances", "Isolate…", "Make transparent…", "Suppress", sep,
                    "Move to new subassembly", "Release…", sep,
                    "Zoom to selection", sep,
                    "Add selection to folder…", "Expand ▸", "Collapse ▸", sep,
                    "Delete",
                ]
            );
            let copy = instance(v, "Part 2 <2>", cx);
            v.selected.clear();
            assert_eq!(
                labels(&v.instance_menu(copy, false, cx)),
                [
                    "Properties…", "Instance properties…", sep,
                    "Hide", "Hide other instances", "Hide all instances", "Isolate…", "Make transparent…", "Suppress", "Fix", "Use best available tessellation", sep,
                    "Add mate connector to instance origin…", "Switch to Part Studio 1", sep,
                    "Copy Part 2 <2>", sep,
                    "Export…", "Release…", sep,
                    "Zoom to selection", sep,
                    "Create Drawing of Part 2…",
                ]
            );
        })
    })
    .unwrap();
}
