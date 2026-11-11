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
    let path = pattern_document(cx, name);
    open_assembly(cx, &path)
}

/// [`pattern_assembly`]'s document.
fn pattern_document(cx: &mut TestAppContext, name: &str) -> std::path::PathBuf {
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
    path
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

/// The workspace's drawing of an assembly tab: the view, and its front layer (the left dialogs) over it.
struct FrontHost {
    view: Entity<AssemblyView>,
    front: gpui_kit::AnyView,
}

impl gpui_kit::Render for FrontHost {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div().relative().size_full().child(self.view.clone()).child(gpui_kit::div().absolute().top(px(crate::ui::TOOLBAR_H)).left_0().right_0().bottom_0().child(self.front.clone()))
    }
}

/// [`open_assembly`] with the front layer drawn, so the left dialogs can be clicked.
fn open_assembly_with_front(cx: &mut TestAppContext, path: &std::path::Path) -> (AnyWindowHandle, Entity<AssemblyView>) {
    let session = crate::session::Session::open(path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::Assembly(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
            let front = v.update(cx, |v, cx| v.front_host(cx).0);
            view = Some(v.clone());
            cx.new(|_| FrontHost { view: v, front })
        })
        .unwrap();
        (w, view.unwrap())
    });
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
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

/// Live ver-linked-selective-update (behaviours "Change to version…" 295–296): Selective update's ⎇ swaps the panel
/// for the document's Version graph (Main, V2, V1, Start, the latest picked); a pick ticks the row and goes back;
/// Update selected moves the instance and says nothing (Update all's toast is its own).
#[gpui::test]
fn selective_updates_version_graph_picks_and_ticks_the_row(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-version-graph-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (src, top) = (dir.join("src.noshape"), dir.join("top.noshape"));
    for p in [&src, &top] {
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", p.display()));
        }
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": src.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"].clone()], "depth": "10 mm", "operationType": "NEW" } }));
        call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
        call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }));
        call(&mut ctx, "history_create_version", json!({ "name": "V2" }));
        call(&mut ctx, "document_create", json!({ "path": top.to_str().unwrap() }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1", "document": src.to_str().unwrap(), "version": "V1" }));
    }
    let (window, view) = open_assembly_with_front(cx, &top);
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.open_reference_manager(None, cx);
            if let Some(m) = &mut v.ui.ref_manager {
                m.tab = 1;
            }
        })
    });
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        let m = v.ui.ref_manager.as_ref().unwrap();
        assert_eq!(m.docs.iter().map(|d| (d.name.clone(), d.current.clone(), d.newer)).collect::<Vec<_>>(), [("src".to_string(), "V1".to_string(), true)]);
    });
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let graph = vcx.debug_bounds("rm-graph-0").expect("the row's Version graph button");
    vcx.simulate_click(graph.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    let list = crate::ui_script::open_list().expect("the Version graph's rows");
    assert_eq!(list["kind"], "list", "{list}");
    assert_eq!(list["rows"], json!(["Main", "V2", "V1", "Start"]), "{list}");
    assert_eq!(list["highlighted"], 1, "the latest version picked: {list}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let v2 = vcx.debug_bounds("rm-picker-1").expect("the V2 row");
    vcx.simulate_click(v2.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    assert!(crate::ui_script::open_list().is_none(), "back to the list");
    cx.update(|cx| {
        let v = view.read(cx);
        let m = v.ui.ref_manager.as_ref().unwrap();
        assert!(m.picking.is_none() && m.checked.contains(&0), "the pick ticked the row");
        assert_eq!(m.docs[0].pick_label(m.chosen[&0]), "V2");
    });
    cx.update(|cx| view.update(cx, |v, cx| v.apply_reference_manager(cx)));
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.message.is_none(), "Update selected says nothing: {:?}", v.message);
        let m = v.ui.ref_manager.as_ref().unwrap();
        assert_eq!((m.docs[0].current.as_str(), m.docs[0].newer), ("V2", false), "the row stays, V2 ⇨ V2");
    });
}

/// Live asm-pattern-field-configure: in a configured assembly, a right-click on a new Linear pattern's Instance count
/// opens only Configure (not the text field's Cut / Copy / Paste menu, which kept the next clicks), and Configure says
/// "Linear pattern 1 Instance count added to Configuration" and closes the menu.
#[gpui::test]
fn a_pattern_fields_configure_takes_the_click(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-pattern-configure-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("asm.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"].clone()], "depth": "10 mm", "operationType": "NEW" } }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "list", "name": "Configuration" }));
    }
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_pattern(super::panels::PatternDialogKind::Linear, w, cx))).unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let count = vcx.debug_bounds("pat-cfg-count").expect("Instance count");
    // On the field's text, at its right (where the input sits): the input's own Cut / Copy / Paste menu stays shut.
    let at = gpui_kit::point(count.right() - px(20.), count.center().y);
    vcx.simulate_mouse_down(at, gpui_kit::MouseButton::Right, gpui_kit::Modifiers::default());
    vcx.simulate_mouse_up(at, gpui_kit::MouseButton::Right, gpui_kit::Modifiers::default());
    draw(cx, window);
    assert_eq!(cx.update(|cx| crate::os_menu::open_labels(cx)), ["Configure"], "only the field's Configure menu");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let row = vcx.debug_bounds("menu:Configure").expect("Configure");
    vcx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    assert!(!cx.update(|cx| crate::os_menu::is_open(cx)), "the menu closed");
    cx.update(|cx| assert_eq!(view.read(cx).ui.cfg_toast.as_deref(), Some("Linear pattern 1 Instance count added to Configuration")));
    // Escape cancels the pattern, its Configure and the toast with it (live).
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.on_cancel(&crate::actions::Cancel, w, cx))).unwrap();
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.pattern.is_none() && v.ui.cfg_toast.is_none(), "the pattern and its toast gone");
    });
}

/// Live asm-standard-content-edit-instance: an inserted instance that snapped on its way (turned onto a face) and then
/// moves off every part turns back to how it was inserted, and its bounds' bottom centre follows the pointer.
#[gpui::test]
fn placing_off_a_snap_turns_the_instance_back(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "unturn");
    let turned = [0.0, -std::f64::consts::FRAC_1_SQRT_2, 0.0, std::f64::consts::FRAC_1_SQRT_2];
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            v.start_placing(vec![pin], cx);
            let start = v.ui.place_rotations.clone();
            assert_eq!(start.len(), 1);
            let mut solved = v.result.solved.clone();
            for (id, pl) in &mut solved.placements {
                if *id == pin {
                    pl.rotation = turned;
                }
            }
            v.apply_solved(solved, cx);
            v.ui.place_moved = true;
            // A corner of the graphics area: off every part.
            v.pointer = glam::DVec2::new(30.0, 30.0);
            assert!(v.update_placing(cx), "placed at the pointer");
            let now = v.result.solved.placement(pin).unwrap().rotation;
            assert_eq!(now, start[0].1, "turned back from the snap's {turned:?}");
        })
    });
    draw(cx, window);
}

/// Live asm-menu-dynamic-suppression-edit- / -remove-expression: Undo is off while the Dynamic suppression panel is
/// open, Escape closes it with nothing selected, and Remove expression says "Dynamic suppression removed.".
#[gpui::test]
fn dynamic_suppressions_panel_escape_and_remove(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "dyn-suppression");
    let target = cx.update(|cx| super::props::DynTarget::Instance(instance(view.read(cx), "Part 2 <1>", cx)));
    cx.update(|cx| view.update(cx, |v, cx| v.set_dynamic_suppression(target, Some("1 > 2".into()), cx)));
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).history_state(cx).0), "the expression is an undo step");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selected = vec![instance(v, "Part 2 <1>", cx)];
            v.open_dynamic_suppression(target, w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).history_state(cx)), (false, false), "Undo off while the panel is open");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.on_cancel(&crate::actions::Cancel, w, cx))).unwrap();
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.dynamic_suppression.is_none() && v.selected.is_empty(), "closed, nothing selected");
        assert!(v.history_state(cx).0, "Undo back");
    });
    let remove = super::AssemblyView::dynamic_suppression_entries(target, true);
    let Some(super::behaviors::MenuEntry::Sub { children, .. }) = remove.iter().find(|e| matches!(e, super::behaviors::MenuEntry::Sub { .. })) else { panic!("Edit dynamic suppression") };
    let Some(super::behaviors::MenuEntry::Item { act, .. }) = children.iter().find(|e| matches!(e, super::behaviors::MenuEntry::Item { label, .. } if label == "Remove expression")) else { panic!("Remove expression") };
    let act = act.clone();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| act(v, w, cx))).unwrap();
    cx.update(|cx| assert_eq!(view.read(cx).message.as_ref().map(|m| m.1.as_str()), Some("Dynamic suppression removed.")));
}

/// User P0 (lead 11-10), in an assembly: the readout's and the Measure panel's measurements run off the UI thread.
/// The frame that shows a selection draws without them (none yet), and they arrive with the background work.
#[gpui::test]
fn an_assembly_measurement_doesnt_hold_up_the_frame(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "measure-off-thread");
    let picks: Vec<(Vec<InstanceId>, Reference)> = cx.update(|cx| {
        let v = view.read(cx);
        ["Part 1 <1>", "Part 2 <1>"]
            .iter()
            .map(|n| {
                let id = instance(v, n, cx);
                let occ = v.result.occurrences.iter().find(|o| o.top() == id).unwrap();
                (vec![id], occ.face_refs[0].clone())
            })
            .collect()
    });
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.ui.entity_sel = picks.clone();
            v.update_entity_measure();
            cx.notify();
        })
    });
    // One frame with the background threads held: the readout's measurement is under way, not done in the frame.
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.entity_measure_for.as_ref() == Some(&picks) && v.ui.entity_measure.is_empty(), "measured in the frame");
    });
    draw(cx, window);
    assert!(cx.update(|cx| !view.read(cx).ui.entity_measure.is_empty()), "the readout's rows arrived");
    // The Measure panel (`[`) takes the selected faces: its rows come the same way.
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_measure(cx)));
    assert!(cx.update(|cx| view.read(cx).ui.measure_result.is_empty()), "not measured on the click");
    draw(cx, window);
    assert!(cx.update(|cx| !view.read(cx).ui.measure_result.is_empty()), "the panel's rows arrived");
}

/// The Mass and section properties dialog's values are worked out off the UI thread, once per selection: the frame
/// that opens it draws with them pending.
#[gpui::test]
fn assembly_mass_properties_dont_hold_up_the_frame(cx: &mut TestAppContext) {
    let path = pattern_document(cx, "mass-off-thread");
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selected = vec![instance(v, "Part 1 <1>", cx)];
            v.toggle_mass(w, cx);
        })
    })
    .unwrap();
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.update(|cx| assert!(view.read(cx).ui.mass_job.as_ref().is_some_and(|j| j.value.is_none()), "worked out in the frame"));
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).ui.mass_job.as_ref().and_then(|j| j.value.clone()).and_then(|(m, _)| m).is_some(), "the values arrived"));
}

/// Live asm-menu-switch-to-part-studio: an instance's Switch to Part Studio opens that Part Studio with its part
/// selected (the event carries the part).
#[gpui::test]
fn switch_to_part_studio_carries_the_part(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "switch-part");
    let got: std::rc::Rc<std::cell::RefCell<Vec<String>>> = Default::default();
    let g = got.clone();
    let _sub = cx.update(|cx| {
        cx.subscribe(&view, move |_, ev: &super::AssemblyViewEvent, _| {
            g.borrow_mut().push(match ev {
                super::AssemblyViewEvent::ShowPart(_, body) => format!("show {body:?}"),
                super::AssemblyViewEvent::ActivateTab(_) => "activate".into(),
                super::AssemblyViewEvent::EditInContext(..) | super::AssemblyViewEvent::EditInContextOther(..) => "context".into(),
            })
        })
    });
    let body = cx.update(|cx| {
        let v = view.read(cx);
        let id = instance(v, "Part 2 <1>", cx);
        match &v.assembly(cx).unwrap().instance(id).unwrap().source {
            noshape_model::assembly::InstanceSource::Part { body, .. } => body.clone(),
            s => panic!("{s:?}"),
        }
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let id = instance(v, "Part 2 <1>", cx);
            let menu = v.instance_menu(id, false, cx);
            let Some(super::behaviors::MenuEntry::Item { act, .. }) = menu.iter().find(|e| matches!(e, super::behaviors::MenuEntry::Item { label, .. } if label == "Switch to Part Studio 1")) else { panic!("Switch to Part Studio 1") };
            let act = act.clone();
            act(v, w, cx);
        })
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*got.borrow(), [format!("show {body:?}")]);
}

/// Show center of mass in an assembly (lead 11-10, the synchronous calls list): each part's own center is worked out
/// off the UI thread once; the marker comes when known, and a re-evaluation (a drag moves parts) weighs the moved
/// centers again without measuring the parts again.
#[gpui::test]
fn assembly_center_of_mass_is_worked_out_off_the_click(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "com-off-thread");
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_center_of_mass(Vec::new(), cx)));
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.com.is_none() && v.ui.com_task.is_some() && v.ui.com_ids.is_some(), "worked out on the click");
    });
    draw(cx, window);
    let parts = cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.com.is_some(), "the marker");
        v.ui.com_parts.len()
    });
    // Part 1 and Part 2 (its pattern copy shares Part 2's model): each measured once.
    assert_eq!(parts, 2, "each part's own center remembered");
    // The same result again (as after a drag's solve): the marker at once, nothing measured again.
    cx.update(|cx| view.update(cx, |v, cx| v.refresh_center_of_mass(cx)));
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.com.is_some() && v.ui.com_task.is_none(), "weighed again without measuring");
    });
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_center_of_mass(Vec::new(), cx)));
    cx.update(|cx| assert!(view.read(cx).ui.com.is_none() && view.read(cx).ui.com_ids.is_none(), "hidden"));
}

/// Live 2464: another document's part has Update linked document…, Open linked document, Pin reference and Create
/// Drawing of <part>… in its menu; pinned, the menu says Unpin reference and the Reference manager leaves it on V1.
#[gpui::test]
fn another_documents_part_pins_its_reference(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-pin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (src, top) = (dir.join("src.noshape"), dir.join("top.noshape"));
    for p in [&src, &top] {
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", p.display()));
        }
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": src.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"].clone()], "depth": "10 mm", "operationType": "NEW" } }));
        call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
        call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }));
        call(&mut ctx, "history_create_version", json!({ "name": "V2" }));
        call(&mut ctx, "document_create", json!({ "path": top.to_str().unwrap() }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1", "document": src.to_str().unwrap(), "version": "V1" }));
    }
    let (_window, view) = open_assembly(cx, &top);
    let p1 = cx.update(|cx| instance(view.read(cx), "Part 1 <1>", cx));
    let menu = cx.update(|cx| labels(&view.read(cx).instance_menu(p1, false, cx)));
    let at = |l: &str| menu.iter().position(|x| x == l).unwrap_or_else(|| panic!("{l} in {menu:?}"));
    assert!(at("Update linked document…") < at("Open linked document") && at("Open linked document") < at("Pin reference"), "{menu:?}");
    assert!(menu.iter().any(|l| l == "Create Drawing of Part 1…"), "{menu:?}");
    assert!(at("Edit in context") < at("Add mate connector to instance origin…"), "{menu:?}");
    assert!(at("Export…") < at("Release…"), "{menu:?}");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.edit("Pin reference", cx, |a| noshape_ops::assembly::set_reference_pinned(a, p1, true));
        })
    });
    let menu = cx.update(|cx| labels(&view.read(cx).instance_menu(p1, false, cx)));
    assert!(menu.iter().any(|l| l == "Unpin reference") && !menu.iter().any(|l| l == "Pin reference" || l == "Edit in context"), "{menu:?}");
    cx.update(|cx| view.update(cx, |v, cx| v.open_reference_manager(None, cx)));
    cx.update(|cx| assert!(view.read(cx).ui.ref_manager.as_ref().unwrap().docs.is_empty(), "a pinned reference isn't offered an update"));
}
