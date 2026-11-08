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
    open_assembly(cx, &path)
}

/// Assembly 1 of the document at `path` in a test window, evaluated.
fn open_assembly(cx: &mut TestAppContext, path: &std::path::Path) -> (AnyWindowHandle, Entity<AssemblyView>) {
    let session = crate::session::Session::open(path).unwrap();
    let asm = |t: &&noshape_model::document::Tab| matches!(t.content, noshape_model::TabContent::Assembly(_));
    let tab = session.doc.tabs.iter().filter(asm).find(|t| t.name == "Assembly 1").or_else(|| session.doc.tabs.iter().find(asm)).unwrap().id;
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

/// A block (Part 1) and a pin (Part 2) in "Sub", whose instance "Sub <1>"
/// is Assembly 1's only instance.
fn sub_assembly(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<AssemblyView>) {
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
        call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 2" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
    }
    open_assembly(cx, &path)
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
/// cloud).
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
                    "Rename", "Edit…", "Hide", "Hide other instances", "Hide all instances", "Isolate…", "Make transparent…", "Suppress", "Dynamic suppression ▸", sep,
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

/// Live 1723: Ctrl+click puts a second mate in the selection; the two have
/// their own menu (Rename, Edit… and Animate… drop out, Clear selection
/// comes in), and Delete takes both in one step.
#[gpui::test]
fn two_selected_mates_have_lives_menu(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "two-mates");
    let sep = "—";
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (a, b) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
            let (c, d) = (face_connector(v, "Part 1 <1>", false, cx), face_connector(v, "Part 2 <1>", true, cx));
            v.edit("Insert : mates", cx, |asm| {
                asm.mates.push(noshape_model::assembly::Mate::new(noshape_model::assembly::MateId(901), "Fastened 1".into(), MateType::Fastened, a, b));
                asm.mates.push(noshape_model::assembly::Mate::new(noshape_model::assembly::MateId(902), "Fastened 2".into(), MateType::Fastened, c, d));
            });
            let ids = vec![noshape_model::assembly::MateId(901), noshape_model::assembly::MateId(902)];
            assert_eq!(
                labels(&v.mates_menu(ids.clone(), cx)),
                [
                    "Show", "Show all mates", "Isolate…", "Make transparent…", "Suppress", sep,
                    "Clear selection", sep,
                    "Add selection to folder…", "Expand ▸", "Collapse ▸", sep,
                    "Delete",
                ]
            );
            v.delete_mates(ids, cx);
            assert!(v.assembly(cx).unwrap().mates.is_empty());
        });
    })
    .unwrap();
}

/// Live 1723: a part's row inside an expanded subassembly has its own menu
/// (less "Add comment"); Move to new subassembly makes a new subassembly in
/// "Sub" around it;
/// its Hide and Fix are kept on Assembly 1's occurrence.
#[gpui::test]
fn a_nested_rows_menu_is_lives(cx: &mut TestAppContext) {
    let (window, view) = sub_assembly(cx, "nested-menu");
    let sep = "—";
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let sub = instance(v, "Sub <1>", cx);
            let inner = v.node_assembly(&[sub], cx).unwrap().instances.iter().find(|i| i.name == "Part 1 <1>").unwrap().id;
            let path = vec![sub, inner];
            assert_eq!(
                labels(&v.nested_instance_menu(path.clone(), cx)),
                [
                    "Properties…", sep,
                    "Hide", "Hide other instances", "Hide all instances", "Isolate…", "Make transparent…", "Fix", "Use best available tessellation", sep,
                    "Edit in context", "Add mate connector to instance origin…", "Switch to Part Studio 1", sep,
                    "Copy Part 1 <1>", sep,
                    "Move to new subassembly", "Export…", "Release…", sep,
                    "Zoom to selection", sep,
                    "Create Drawing of Part 1…", sep,
                    "Create empty subassembly",
                ]
            );
            v.edit("Hide", cx, |a| a.set_occurrence(&path, |s| s.hidden = true));
            assert!(v.assembly(cx).unwrap().occurrence_hidden(&path));
            assert_eq!(labels(&v.nested_instance_menu(path.clone(), cx))[2], "Show");
            v.move_occurrence_to_new_subassembly(path.clone(), cx);
            let sub_now = v.node_assembly(&[sub], cx).unwrap();
            assert!(sub_now.instance(inner).is_none() && sub_now.instances.iter().any(|i| matches!(i.source, noshape_model::assembly::InstanceSource::Assembly { .. })), "Part 1 <1> went into a new subassembly in Sub");
            assert!(v.assembly(cx).unwrap().occurrences.is_empty(), "its old occurrence state is dropped");
        });
    })
    .unwrap();
}

/// Enter with the focus in the graphics accepts the open dialog as its ✓ (behaviours "Enter accepts the open
/// dialog"; ui-parity asm-connector-menu): the Mate connector dialog at Part 2's origin, then the Group dialog.
#[gpui::test]
fn enter_in_the_graphics_accepts_the_open_dialog(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "enter-accepts");
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    let enter = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            let focus = view.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_keystroke(gpui_kit::Keystroke::parse("enter").unwrap(), cx);
            w.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            v.mc_at_instance_origin(pin, w, cx);
            assert!(v.mc.is_some());
        });
    })
    .unwrap();
    enter(cx);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.mc.is_none(), "{:?}", v.message);
        assert_eq!(v.assembly(cx).unwrap().connectors.len(), 1);
    });
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (block, pin) = (instance(v, "Part 1 <1>", cx), instance(v, "Part 2 <1>", cx));
            v.selected.clear();
            v.create_group(cx);
            v.toggle_group_member(block, cx);
            v.toggle_group_member(pin, cx);
        });
    })
    .unwrap();
    enter(cx);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.group.is_none(), "{:?}", v.message);
        assert_eq!(v.assembly(cx).unwrap().groups.len(), 1);
    });
}

/// Lead 11-06, the display cache: an assembly's evaluation comes back from the cache with the same occurrences (meshes,
/// placements) and instance tree; shown in its place the view is view-only (a mate asked for waits, the user told at
/// once, "Regenerating…" at the top of the instance list), and the real evaluation replaces it and opens the mate.
#[gpui::test]
fn a_cached_assembly_stands_in_until_the_real_one_lands(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "display-cache");
    // The opened assembly's evaluation (on a background thread) in.
    for _ in 0..600 {
        if cx.update(|cx| view.read(cx).ui.real_result_in) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        cx.run_until_parked();
    }
    assert!(cx.update(|cx| view.read(cx).ui.real_result_in), "the assembly evaluated");
    let real = cx.update(|cx| view.read(cx).result.clone());
    assert_eq!(real.occurrences.len(), 3, "the block, the pin and its copy");
    type Occ = (Vec<InstanceId>, String, Vec<[f32; 3]>, Vec<u32>, glam::DMat4);
    let occs = |r: &noshape_model::assembly::eval::AssemblyResult| -> Vec<Occ> {
        r.occurrences.iter().map(|o| (o.path.clone(), o.instance_name.clone(), o.mesh.positions.clone(), o.mesh.indices.clone(), o.transform)).collect()
    };
    let names = |r: &noshape_model::assembly::eval::AssemblyResult| -> Vec<String> { r.nodes.iter().map(|n| n.name.clone()).collect() };
    let dir = std::env::temp_dir().join(format!("noshape-asm-cache-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cached = super::open::cache_round_trip(&real, &dir).expect("the cached evaluation");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(occs(&cached), occs(&real), "cached as it was");
    assert_eq!(names(&cached), names(&real));
    // Reopened: the cached evaluation stands in.
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.real_result_in = false;
            v.show_cached_result("k".into(), cached, w, cx);
        })
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx.update(|cx| view.read(cx).ui.cached_view && view.read(cx).render_regenerating_row().is_some()), "view-only, Regenerating… shown");
    // A mate waits, with feedback at once.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_mate(MateType::Fastened, w, cx))).unwrap();
    let (opened, said) = cx.update(|cx| (view.read(cx).mate.is_some(), view.read(cx).message.as_ref().map(|(_, m)| m.clone())));
    assert!(!opened, "no mate dialog on the cached model");
    assert_eq!(said.as_deref(), Some(crate::display_cache::WAITING));
    // The real evaluation lands: the same occurrences, the mate dialog opens.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_eval(w, cx))).unwrap();
    for _ in 0..600 {
        if cx.update(|cx| view.read(cx).mate.is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        cx.run_until_parked();
    }
    let after = cx.update(|cx| view.read(cx).result.clone());
    assert!(!cx.update(|cx| view.read(cx).ui.cached_view), "the model's own again");
    assert!(cx.update(|cx| view.read(cx).mate.is_some()), "the mate dialog opened once the real model landed");
    assert_eq!(occs(&after), occs(&real), "the evaluated assembly is the cached one");
}


fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

/// live2 2241: Hide mates on mates already hidden is an undo step of its own: one Undo takes it back and changes
/// nothing (the hide before it stays), and Redo is offered.
#[gpui::test]
fn hiding_hidden_mates_is_an_undo_step(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "hide-hidden");
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    cx.update_window(window, |_, w, cx| {
        w.activate_window();
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            v.mc_at_instance_origin(pin, w, cx);
            let focus = v.focus.clone();
            w.focus(&focus, cx);
        });
        w.dispatch_keystroke(gpui_kit::Keystroke::parse("enter").unwrap(), cx);
    })
    .unwrap();
    draw(cx, window);
    let ids = cx.update(|cx| view.read(cx).assembly(cx).unwrap().connectors.iter().map(|c| c.id).collect::<Vec<_>>());
    assert_eq!(ids.len(), 1, "the connector made");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.set_mates_hidden(ids.clone(), true, cx);
            v.set_mates_hidden(ids.clone(), true, cx);
        })
    });
    draw(cx, window);
    let doc = cx.update(|cx| view.read(cx).doc.clone());
    cx.update(|cx| doc.update(cx, |d, cx| d.undo(cx)));
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        let a = v.assembly(cx).unwrap();
        assert!(a.connectors.len() == 1 && a.connectors[0].hidden, "the no-change step undone, the hide kept");
        assert!(doc.read(cx).can_redo());
    });
}

/// A new List input opens its italic "Name" row for typing, and the recorded focus is that input (live2 2208).
#[gpui::test]
fn a_new_list_input_types_into_its_name_row(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "cfg-name-row");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.add_asm_config_input("List", w, cx);
            let input = v.assembly(cx).unwrap().configuration.inputs[0].id.clone();
            assert_eq!(v.ui.cfg_edit.as_ref().map(|e| e.key.clone()), Some(format!("opt-new-{input}")));
        });
    })
    .unwrap();
    draw(cx, window);
    let state = cx.update(|cx| view.read(cx).ui_state(cx));
    assert_eq!(state["focus"]["kind"], "other", "{}", state["focus"]);
}

/// The Mate connector dialog's viewport pick is selected and is an undo step of the dialog (live
/// asm-mate-connector-on-face): Undo takes the pick back, the dialog stays.
#[gpui::test]
fn a_mate_connector_pick_is_selected_and_undone_in_the_dialog(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "mc-pick-undo");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let part = instance(v, "Part 2 <1>", cx);
            v.open_mc(None, w, cx);
            let c = face_connector(v, "Part 2 <1>", true, cx);
            v.deliver_connector(c, w, cx);
            assert!(v.mc.as_ref().is_some_and(|m| m.on.as_ref().is_some_and(|o| o.instance == part)));
            let state = v.ui_state(cx);
            assert_eq!(state["selection"]["count"], 1, "{}", state["selection"]);
            assert_eq!(state["focus"]["kind"], "none");
            assert_eq!(v.history_state(cx), (true, false));
            assert!(v.mc_undo(false, w, cx));
            assert!(v.mc.as_ref().is_some_and(|m| m.on.is_none()), "the pick taken back, the dialog open");
            assert_eq!(v.history_state(cx), (false, true));
        });
    })
    .unwrap();
}
