//! UI tests of the Assembly view on GPUI's test platform (docs/TESTS.md "UI
//! keystroke tests"): the real view in a test window over a document made
//! through the agent API, the GPU a no-op stand-in.

use super::AssemblyView;
use crate::assembly_scene::{Highlights, ManipHandle};
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
    open_tab_with_front(cx, path, "Assembly 1")
}

/// [`open_assembly_with_front`] on the assembly `name` (else the first).
fn open_tab_with_front(cx: &mut TestAppContext, path: &std::path::Path, name: &str) -> (AnyWindowHandle, Entity<AssemblyView>) {
    let session = crate::session::Session::open(path).unwrap();
    let asm = |t: &&noshape_model::document::Tab| matches!(t.content, noshape_model::TabContent::Assembly(_));
    let tab = session.doc.tabs.iter().filter(asm).find(|t| t.name == name).or_else(|| session.doc.tabs.iter().find(asm)).unwrap().id;
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
                    "Move to new subassembly", "Change to version…", "Export…", "Where used…", sep,
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
                    "Edit in context", "Find in instance list", "Add mate connector to instance origin…", "Replace Part 1 <1>…", "Switch to Part Studio 1", "Where used for Part 1…", sep,
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
                    "Export…", sep,
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
                    "Move to new subassembly", sep,
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
                    "Export…", "Where used…", sep,
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
/// (less "Add comment"; live Pro asm-pro-menus-1119 b: Where used…, no
/// Release…); Move to new subassembly makes a new subassembly in "Sub"
/// around it;
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
                    "Move to new subassembly", "Export…", "Where used…", sep,
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

/// TICKETS P0 HEAVY-ASM-SOFTWARE-FRAME (1): a refresh that changes nothing asks for no 3D frame, though it builds the
/// mate glyphs anew (they were compared by pointer: every refresh of an assembly with mates drew a frame, ~47 s each
/// on a software renderer for a big one).
#[gpui::test]
fn a_refresh_with_the_same_mate_glyphs_submits_no_frame(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "same-glyphs");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (a, b) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
            v.edit("Insert : mates", cx, |asm| {
                asm.mates.push(noshape_model::assembly::Mate::new(noshape_model::assembly::MateId(901), "Fastened 1".into(), MateType::Fastened, a, b));
            });
        })
    })
    .unwrap();
    // Frames come back from the render thread in real time.
    let frames_for = |cx: &mut TestAppContext, ms: u64| {
        let t = std::time::Instant::now();
        while t.elapsed() < std::time::Duration::from_millis(ms) {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            draw(cx, window);
        }
    };
    frames_for(cx, 600);
    // Its glyph drawn: the mate selected.
    view.update(cx, |v, cx| {
        v.selected_mate = Some(noshape_model::assembly::MateId(901));
        v.refresh_scene(false, cx);
    });
    frames_for(cx, 400);
    // What each refresh itself asks for (other work landing meanwhile on a loaded host doesn't count).
    let refresh = |cx: &mut TestAppContext, mate: Option<noshape_model::assembly::MateId>| {
        let asked = view.update(cx, |v, cx| {
            let before = v.viewport.read(cx).frames_requested();
            v.selected_mate = mate;
            v.refresh_scene(false, cx);
            v.viewport.read(cx).frames_requested() - before
        });
        frames_for(cx, 60);
        asked
    };
    let same: Vec<u64> = (0..5).map(|_| refresh(cx, Some(noshape_model::assembly::MateId(901)))).collect();
    // A real change still draws: the mate no longer selected.
    let deselected = refresh(cx, None);
    assert_eq!((same, deselected > 0), (vec![0; 5], true), "(frames each of 5 unchanged refreshes asked for, one for the deselection)");
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

/// Saving and deleting a named view are no undo steps (live ui-parity named-views-add: Undo stays off), as in a Part
/// Studio.
#[gpui::test]
fn a_named_view_is_no_undo_step(cx: &mut TestAppContext) {
    use crate::named_views_dialog::Action;
    let (window, view) = pattern_assembly(cx, "named-view-undo");
    let doc = cx.update(|cx| view.read(cx).doc.clone());
    let undo_before = cx.update(|cx| doc.read(cx).can_undo());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.toggle_named_views(w, cx);
            v.on_named_views_action(Action::Add, w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update(|cx| {
        assert_eq!(view.read(cx).assembly(cx).unwrap().named_views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["View 1"]);
        assert_eq!(doc.read(cx).can_undo(), undo_before, "saving a view is no undo step");
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.named_view_picked = Some("View 1".into());
            v.on_named_views_action(Action::Remove, w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update(|cx| {
        assert!(view.read(cx).assembly(cx).unwrap().named_views.is_empty());
        assert_eq!(doc.read(cx).can_undo(), undo_before, "deleting a view is no undo step");
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

/// Live 2509: an inserted part placed on empty space keeps its rotation and has its bounding-box centre on the
/// pointer's view ray, at the assembly origin's depth (the plane through the origin across the view).
#[gpui::test]
fn placing_on_empty_space_puts_the_box_centre_on_the_ray_at_the_origins_depth(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "empty-place");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            v.start_placing(vec![pin], cx);
            let start = v.result.solved.placement(pin).unwrap().rotation;
            v.ui.place_moved = true;
            v.hover = None;
            v.pointer = glam::DVec2::new(40.0, 50.0);
            assert!(v.update_placing(cx), "placed at the pointer");
            assert_eq!(v.result.solved.placement(pin).unwrap().rotation, start, "its rotation kept");
            let (lo, hi) = v.result.instance_bounds(pin).unwrap();
            let c = (lo + hi) * 0.5;
            let vp = v.viewport.read(cx);
            let (o, d) = vp.ray(v.pointer);
            assert!((c - o).cross(d.normalize()).length() < 1e-9, "the box centre on the ray: {c:?}");
            assert!(c.dot(vp.camera.forward()).abs() < 1e-9, "at the origin's depth: {c:?}");
        })
    });
    draw(cx, window);
}

/// Live asm-menu-dynamic-suppression-edit- / -remove-expression: Undo is off while the Dynamic suppression panel is
/// open, Escape closes it with nothing selected, and Remove expression says "Dynamic suppression removed.".
#[gpui::test]
fn dynamic_suppressions_panel_escape_and_remove(cx: &mut TestAppContext) {
    // (The panel is on the front layer the workspace draws.)
    let path = pattern_document(cx, "dyn-suppression");
    let (window, view) = open_assembly_with_front(cx, &path);
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
    // Escape from its expression field (which has the keyboard).
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    cx.update_window(window, |_, w, cx| {
        use gpui_kit::Focusable as _;
        let st = view.read(cx).ui.dynamic_suppression.as_ref().map(|d| d.1.clone()).unwrap();
        assert!(st.read(cx).focus_handle(cx).is_focused(w), "the expression has the keyboard");
        w.dispatch_keystroke(gpui_kit::Keystroke::parse("escape").unwrap(), cx);
    })
    .unwrap();
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

/// Live2 asm-dialogs-1119 q-meas: the Measure panel hangs 279 px into the graphics area (under the toolbar), 13 px from
/// its right, whatever its rows (it grows downward; ours was docked at the bottom).
#[gpui::test]
fn the_measure_panel_hangs_from_a_fixed_top(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "measure-place", |_| {});
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let id = instance(v, "Part 1 <1>", cx);
            let occ = v.result.occurrences.iter().find(|o| o.top() == id).unwrap();
            v.ui.entity_sel = vec![(vec![id], occ.face_refs[0].clone())];
            v.toggle_measure(cx);
        })
    });
    draw(cx, window);
    let window_w = cx.update_window(window, |_, w, _| f32::from(w.viewport_size().width)).unwrap();
    let b = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("measure-panel").expect("the Measure panel");
    let (top, right) = (f32::from(b.top()), window_w - f32::from(b.right()));
    assert!((top - (crate::ui::TOOLBAR_H + super::MEASURE_TOP)).abs() < 0.5 && (right - super::MEASURE_RIGHT).abs() < 0.5, "top {top}, {right} from the right");
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
                super::AssemblyViewEvent::WhereUsed(_, body) => format!("where used {body:?}"),
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
            // Live Pro (asm-menu-release 11-19): the list's Where used… right after Export…, no Release….
            let labels: Vec<&str> = menu.iter().filter_map(|e| match e {
                super::behaviors::MenuEntry::Item { label, .. } => Some(label.as_str()),
                _ => None,
            }).collect();
            let at = labels.iter().position(|l| *l == "Export…").expect("Export…");
            assert_eq!(labels.get(at + 1), Some(&"Where used…"), "{labels:?}");
            assert!(!labels.contains(&"Release…"), "{labels:?}");
            for name in ["Switch to Part Studio 1", "Where used…"] {
                let Some(super::behaviors::MenuEntry::Item { act, .. }) = menu.iter().find(|e| matches!(e, super::behaviors::MenuEntry::Item { label, .. } if label == name)) else { panic!("{name}") };
                let act = act.clone();
                act(v, w, cx);
            }
        })
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*got.borrow(), [format!("show {body:?}"), format!("where used {body:?}")]);
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
    // (Live2 context-create-version-update, Professional: Where used… right after Export….)
    assert_eq!(menu.get(at("Export…") + 1).map(String::as_str), Some("Where used…"), "{menu:?}");
    assert!(!menu.iter().any(|l| l == "Release…"), "no Release… with a new company's managed workflows off: {menu:?}");
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

/// Live2 config.json (tableKeys), the assembly's Configurations table as the Part Studio's: a framed value cell edits
/// on Enter, and Enter after typing commits it and frames the cell below.
#[gpui::test]
fn enter_in_a_configured_value_moves_down_a_row(cx: &mut TestAppContext) {
    use noshape_model::assembly::config::ConfigTarget;
    use noshape_model::ParamValue;
    let path = pattern_document(cx, "cfg-enter");
    let (window, view) = open_assembly_with_front(cx, &path);
    let (input, rows) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let pattern = v.assembly(cx).unwrap().patterns[0].id;
            let mut out = (String::new(), Vec::new());
            v.edit("Configure", cx, |a| {
                let input = noshape_ops::asm_config::add_input(a, "List", Some("Configuration"));
                noshape_ops::asm_config::add_option(a, &input, "A");
                let rows: Vec<String> = match &a.configuration.inputs.iter().find(|i| i.id == input).unwrap().kind {
                    noshape_model::configuration::ConfigInputKind::List { options, .. } => options.iter().map(|o| o.id.clone()).collect(),
                    _ => Vec::new(),
                };
                assert!(noshape_ops::asm_config::configure(a, &input, ConfigTarget::Pattern(pattern), noshape_model::assembly::config::param::DISTANCE));
                out = (input, rows);
            });
            v.panel = Some(super::SidePanel::Configurations);
            cx.notify();
            out
        })
    });
    assert_eq!(rows.len(), 2, "Default and A in {input}");
    draw(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.frame_cfg_cell(format!("cv-0-{}", rows[0]), w, cx))).unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.cfg_framed.is_some()), "framed");
    assert!(cx.update(|cx| !view.read(cx).ui.cfg_enter_next.is_empty()), "the value cells drawn");
    let focused = cx.update_window(window, |_, w, cx| view.read(cx).ui.cfg_framed.as_ref().is_some_and(|(_, f, _)| f.is_focused(w))).unwrap();
    assert!(focused, "the framed cell has the keyboard");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_keystrokes("enter");
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.cfg_edit.is_some()), "Enter edits the framed cell");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_keystrokes("5 space m m enter");
    draw(cx, window);
    let (value, framed) = cx.update(|cx| {
        let v = view.read(cx);
        let value = v.assembly(cx).unwrap().configuration.configured[0].values.get(&rows[0]).cloned();
        (value, v.ui.cfg_framed.as_ref().map(|(k, ..)| k.clone()))
    });
    assert!(matches!(&value, Some(ParamValue::Expr(e)) if e.text == "5 mm"), "{value:?}");
    assert_eq!(framed, Some(format!("cv-0-{}", rows[1])), "the cell below framed");
}

/// Live2 feedback.json (documentLoad): an opening assembly says "Loading studio data…" until its first evaluation is
/// in (after a moment), and nothing once it is.
#[gpui::test]
fn an_opening_assembly_says_it_is_loading(cx: &mut TestAppContext) {
    let path = pattern_document(cx, "loading");
    let (window, view) = open_assembly_with_front(cx, &path);
    let t = std::time::Instant::now();
    while !cx.update(|cx| view.read(cx).ui.real_result_in) && t.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    assert_eq!(cx.update(|cx| view.read(cx).loading_message()), None, "evaluated: nothing");
    view.update(cx, |v, _| {
        v.ui.real_result_in = false;
        v.ui.opened_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    });
    assert_eq!(cx.update(|cx| view.read(cx).loading_message()), Some("Loading studio data…"));
}

/// Live2 fields.json contextMenus.twoInstances (rules dropped; "Add comment" is cloud): Dynamic suppression ▸,
/// Check interference… before the mate items, "Replace instances…", Clear selection before Delete.
#[gpui::test]
fn two_selected_instances_have_lives_list_menu(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "two-menu");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (p1, p2) = (instance(v, "Part 1 <1>", cx), instance(v, "Part 2 <1>", cx));
            v.selected = vec![p1, p2];
            let got: Vec<String> = labels(&v.instance_menu(p1, false, cx)).into_iter().filter(|l| l != "—").collect();
            let live = [
                "Hide", "Hide other instances", "Hide all instances", "Show center of mass", "Isolate…", "Make transparent…", "Suppress", "Dynamic suppression ▸", "Fix",
                "Use best available tessellation", "Check interference…", "Replace instances…", "Copy 2 items", "Move to new subassembly", "Change to version…", "Export…",
                "Zoom to selection", "Add selection to folder…", "Create empty subassembly", "Clear selection", "Delete",
            ];
            assert_eq!(got, live);
        })
    })
    .unwrap();
}

/// Live asm/feel-1117 (reach-corner-strip) and asm-feel3-1118: over a planar face the preview connector jumps to a dot
/// within a 47.5 px square around the pointer (a corner 3 px away; kept at (25, 46) and (47, 40), lost at (26, 48) and
/// (49, 42)), else it sits at the face's centroid; with Shift locking the face's points the nearest is taken at any
/// distance.
#[gpui::test]
fn a_faces_connector_takes_a_dot_within_a_48_px_square_else_its_centroid(cx: &mut TestAppContext) {
    use noshape_scene::{asm_scene::AsmPickTarget, builder::PickKind};
    let (window, view) = pattern_assembly(cx, "face-centroid");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Fastened, w, cx);
            let top = face_connector(v, "Part 1 <1>", true, cx);
            let Reference::Implicit { on, .. } = &top.reference else { panic!("an implicit connector") };
            let id = instance(v, "Part 1 <1>", cx);
            let occ = v.result.occurrences.iter().position(|o| o.top() == id).unwrap();
            v.hover = Some(AsmPickTarget { path: vec![id], reference: (**on).clone(), kind: PickKind::Face, occ, anchor: None });
            v.update_inference(cx);
            let (_, _, cands) = v.infer.clone().expect("the face's points");
            let tf = v.result.occurrences[occ].transform;
            let at = |v: &AssemblyView, p: glam::DVec3, cx: &gpui_kit::App| v.viewport.read(cx).project(tf.transform_point3(p)).unwrap();
            let corner = cands.iter().find(|c| matches!(c.reference, Reference::Implicit { inference: Inference::Vertex, .. })).expect("a corner").frame.origin;
            v.pointer = at(v, corner, cx) + glam::DVec2::new(3.0, 3.0);
            v.update_inference(cx);
            assert!(matches!(cands[v.infer_choice.expect("a choice")].reference, Reference::Implicit { inference: Inference::Vertex, .. }), "the corner 3 px away");
            // Entering the face there: the centroid first, the corner on the next move (live2 place-1118 21:05).
            v.hover = None;
            v.update_inference(cx);
            v.hover = Some(AsmPickTarget { path: vec![id], reference: (**on).clone(), kind: PickKind::Face, occ, anchor: None });
            v.update_inference(cx);
            assert_eq!(v.infer_choice, Some(0), "entering: the centroid");
            v.update_inference(cx);
            assert!(matches!(cands[v.infer_choice.expect("a choice")].reference, Reference::Implicit { inference: Inference::Vertex, .. }), "the next move: the corner");
            let corner_ix = cands.iter().position(|c| c.frame.origin == corner).unwrap();
            for (off, kept) in [((25.0, 46.0), true), ((47.0, 40.0), true), ((26.0, 48.0), false), ((49.0, 42.0), false)] {
                v.pointer = at(v, corner, cx) + glam::DVec2::from(off);
                v.update_inference(cx);
                assert_eq!(v.infer_choice != Some(corner_ix), !kept, "the corner at {off:?}: kept {kept}");
            }
            // From the corner straight toward the centroid until every dot is out of the square.
            let dots: Vec<glam::DVec2> = cands.iter().skip(1).map(|c| at(v, c.frame.origin, cx)).collect();
            let (c0, c1) = (at(v, corner, cx), at(v, cands[0].frame.origin, cx));
            let far = (0..=100).map(|i| c0 + (c1 - c0) * (i as f64 / 100.0)).find(|p| dots.iter().all(|d| (*d - *p).abs().max_element() > 48.0));
            let Some(far) = far else { panic!("the face is too small on screen for a point 48 px from every dot") };
            v.pointer = far;
            v.update_inference(cx);
            assert_eq!(v.infer_choice, Some(0), "the centroid");
            v.pointer = at(v, corner, cx) + glam::DVec2::new(3.0, 3.0);
            v.infer_locked = true;
            v.update_inference(cx);
            assert_ne!(v.infer_choice, Some(0), "Shift: the corner by the pointer");
        })
    })
    .unwrap();
}

/// Live asm/feel3-1118 r1–r4: a new Revolute's typed min "30" + Enter reads "30 deg" and turns the preview from 0° to
/// 30° (clamped into the range; nothing while typing); a max of 60 then leaves it at 30°.
#[gpui::test]
fn a_typed_limit_turns_the_new_mates_preview_into_range(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "limit-preview");
    let angle = |cx: &mut TestAppContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        cx.update(|cx| {
            let v = view.read(cx);
            let id = v.mate.as_ref().unwrap().dialog.read(cx).to_mate(cx).unwrap().id;
            let m = v.result.mates.iter().find(|m| m.id == id).expect("the previewed mate");
            assert!(m.ok, "{:?}", m.message);
            m.values[0].to_degrees()
        })
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
        });
    })
    .unwrap();
    assert!(angle(cx).abs() < 1e-6, "the connectors meet at 0°");
    let limit = |max: bool, text: &'static str, enter: bool, cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            let dialog = view.read(cx).mate.as_ref().unwrap().dialog.clone();
            dialog.update(cx, |d, cx| d.type_limit("rz", max, text, enter, w, cx));
        })
        .unwrap();
    };
    limit(false, "30", false, cx);
    assert!(angle(cx).abs() < 1e-6, "no solve while typing");
    limit(false, "30", true, cx);
    assert!((angle(cx) - 30.0).abs() < 1e-6, "turned to the minimum on Enter");
    assert_eq!(cx.update(|cx| view.read(cx).mate.as_ref().unwrap().dialog.read(cx).to_mate(cx).unwrap().limits[0].1.min.as_ref().unwrap().text.clone()), "30 deg");
    limit(true, "60", true, cx);
    assert!((angle(cx) - 30.0).abs() < 1e-6, "stays at 30°");
}

/// qa P1 (ASM-LIMIT-TYPED-NO-PREVIEW), live mate limits r1-r4 (11-18): in an edited Revolute a typed limit turns the
/// preview into its range at once (min 30 / max 35 → 30°, then min 0 / max 5 → 5°), before Solve or ✓ (ours held the
/// edited mate's angle where it was).
#[gpui::test]
fn a_typed_limit_turns_the_edited_mates_preview_into_range(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "edit-limit-preview");
    let settle = |cx: &mut TestAppContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.accept_mate(w, cx);
        });
    })
    .unwrap();
    settle(cx);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let id = v.assembly(cx).unwrap().mates.iter().find(|m| m.name == "Revolute 1").expect("Revolute 1").id;
            v.edit_mate(id, w, cx);
        })
    })
    .unwrap();
    settle(cx);
    let angle = |cx: &mut TestAppContext| {
        settle(cx);
        cx.update(|cx| {
            let v = view.read(cx);
            let id = v.mate.as_ref().unwrap().dialog.read(cx).to_mate(cx).unwrap().id;
            let m = v.result.mates.iter().find(|m| m.id == id).expect("the previewed mate");
            assert!(m.ok, "{:?}", m.message);
            m.values[0].to_degrees()
        })
    };
    let start = angle(cx);
    assert!(!(29.0..=36.0).contains(&start), "outside the first range before the limits: {start}°");
    let limit = |max: bool, text: &'static str, cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            let dialog = view.read(cx).mate.as_ref().unwrap().dialog.clone();
            dialog.update(cx, |d, cx| d.type_limit("rz", max, text, true, w, cx));
        })
        .unwrap();
    };
    limit(false, "30", cx);
    limit(true, "35", cx);
    assert!((angle(cx) - 30.0).abs() < 1e-6, "turned to 30° in the preview");
    limit(false, "0", cx);
    limit(true, "5", cx);
    assert!((angle(cx) - 5.0).abs() < 1e-6, "turned back to 5° in the preview");
    assert!(view.read_with(cx, |v, _| v.mate.is_some()), "still editing (no Solve, no ✓)");
}

/// Live mate-keys-1118: Q in an edited Revolute turns the free part a quarter turn about the mate's axis, +90° about the
/// connectors' Z (pin-block: −90° about world Z, the connectors' Z pointing down); its free rotation doesn't absorb it.
#[gpui::test]
fn q_in_an_edited_revolute_turns_the_free_part_a_quarter(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "edit-q");
    let settle = |cx: &mut TestAppContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.accept_mate(w, cx);
        });
    })
    .unwrap();
    settle(cx);
    let turn = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let p = v.result.solved.placement(instance(v, "Part 2 <1>", cx)).unwrap();
            glam::DQuat::from_xyzw(p.rotation[0], p.rotation[1], p.rotation[2], p.rotation[3])
        })
    };
    let before = turn(cx);
    let axis = cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let m = v.assembly(cx).unwrap().mates.iter().find(|m| m.name == "Revolute 1").expect("Revolute 1").clone();
            v.edit_mate(m.id, w, cx);
            v.result.connector_world(&m.b, &v.units(cx)).unwrap().z()
        })
    })
    .unwrap();
    settle(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.flip_or_reorient(false, w, cx))).unwrap();
    settle(cx);
    let d = turn(cx) * before.inverse();
    let want = glam::DQuat::from_axis_angle(axis, std::f64::consts::FRAC_PI_2);
    assert!(d.dot(want).abs() > 1.0 - 1e-9, "a quarter turn about the connectors' Z: {d:?} vs {want:?}");
}

/// Live mate-keys-1118: A in a freshly edited Revolute turns the free part 180° about the connector's Y, diag(−1, 1, −1)
/// on pin-block (not about X: a flip alone).
#[gpui::test]
fn a_in_an_edited_revolute_turns_the_free_part_over_about_y(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "edit-a");
    let settle = |cx: &mut TestAppContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.accept_mate(w, cx);
        });
    })
    .unwrap();
    settle(cx);
    let turn = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let p = v.result.solved.placement(instance(v, "Part 2 <1>", cx)).unwrap();
            glam::DQuat::from_xyzw(p.rotation[0], p.rotation[1], p.rotation[2], p.rotation[3])
        })
    };
    let before = turn(cx);
    let y = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let m = v.assembly(cx).unwrap().mates.iter().find(|m| m.name == "Revolute 1").expect("Revolute 1").clone();
                v.edit_mate(m.id, w, cx);
                v.result.connector_world(&m.b, &v.units(cx)).unwrap().y()
            })
        })
        .unwrap();
    settle(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.flip_or_reorient(true, w, cx))).unwrap();
    settle(cx);
    let d = turn(cx) * before.inverse();
    let want = glam::DQuat::from_axis_angle(y, std::f64::consts::PI);
    assert!(d.dot(want).abs() > 1.0 - 1e-6, "A: 180° about the connector's Y {y:?}: {d:?}");
}

/// Live extras-1118 n0/n1: Edit named position driver mate… with no position makes "Named position 1" and edits it, no
/// undo step yet (n0, asm-mate-menu-edit-driver-mate); ticking a DOF applies at once, with the blue bubble "Revolute 1
/// Rz added to named positions"; closing the selector makes the position an undo step (live: Undo on, no banner).
#[gpui::test]
fn ticking_a_driver_mate_makes_the_first_named_position_at_once(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "np-driver");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.accept_mate(w, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let m = v.assembly(cx).unwrap().mates.iter().find(|m| m.name == "Revolute 1").expect("Revolute 1").id;
            let undo_before = v.doc.read(cx).session.can_undo();
            v.open_np_driver(m, cx);
            assert_eq!(v.assembly(cx).unwrap().named_positions.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Named position 1"], "opening the selector makes the first position");
            assert_eq!(v.ui.np_editing, Some(0), "being edited");
            assert_eq!(v.doc.read(cx).session.can_undo(), undo_before, "no undo step yet");
            v.toggle_np_driver(m, 0, cx);
            let a = v.assembly(cx).unwrap();
            assert_eq!(a.position_drivers, vec![(m, 0)]);
            assert_eq!(a.named_positions.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Named position 1"]);
            assert_eq!(v.ui.np_editing, Some(0));
            assert_eq!(v.ui.np_toast.as_deref(), Some("Revolute 1 Rz added to named positions"));
            assert!(v.ui.np_toast_info, "the blue bubble");
            v.accept_np_driver(cx);
            assert!(v.ui.np_driver.is_none());
            assert_eq!(v.assembly(cx).unwrap().position_drivers, vec![(m, 0)]);
            assert_eq!(v.assembly(cx).unwrap().named_positions.len(), 1, "still one position");
            assert!(v.ui.np_editing.is_none() && v.doc.read(cx).session.can_undo(), "closed: no banner, Undo on");
        });
    });
}

/// Live asm-views2-1118: a step a drag just made (its dialog open): Enter keeps it, editing goes on; Esc removes it and
/// also ends "Editing Explode 1" (one Esc).
#[gpui::test]
fn esc_on_a_new_explode_step_removes_it_and_ends_the_editing(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "explode-esc");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            let step: noshape_model::assembly::ExplodeStep = serde_json::from_value(json!({ "instances": [pin], "translation": [0.0, 0.0, 0.0], "rotation_axis": null, "rotation_angle": 0.0 })).unwrap();
            v.edit("Insert : Explode 1", cx, |a| a.exploded_views.push(noshape_model::assembly::ExplodedView { name: "Explode 1".into(), steps: vec![] }));
            v.activate_explode_from_menu(0, cx);
            let steps = |v: &AssemblyView, cx: &gpui_kit::App| v.assembly(cx).unwrap().exploded_views[0].steps.len();
            for (enter, left) in [(true, 1), (false, 1)] {
                v.push_explode_step(step.clone(), cx);
                v.open_step_edit(steps(v, cx) - 1, true, w, cx);
                if enter {
                    v.on_accept(&crate::actions::Accept, w, cx);
                    assert!(v.step_edit.is_none() && v.explode_edit, "Enter: kept, still editing");
                } else {
                    v.on_cancel(&crate::actions::Cancel, w, cx);
                    assert!(v.step_edit.is_none() && !v.explode_edit && v.explode.is_none(), "Esc: editing ended");
                }
                assert_eq!(steps(v, cx), left);
            }
        });
    })
    .unwrap();
}

/// Live asm-views2-1118: "Activate Explode 1" jumps to the exploded state and the camera to fit the exploded parts in
/// one frame (no playback, no animation).
#[gpui::test]
fn activating_an_exploded_view_jumps_to_it_and_to_fit(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "explode-activate");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            let step: noshape_model::assembly::ExplodeStep = serde_json::from_value(json!({ "instances": [pin], "translation": [0.0, 0.0, 0.5], "rotation_axis": null, "rotation_angle": 0.0 })).unwrap();
            v.edit("Insert : Explode 1", cx, |a| a.exploded_views.push(noshape_model::assembly::ExplodedView { name: "Explode 1".into(), steps: vec![step] }));
            let before = v.result.instance_bounds(pin).unwrap();
            let distance = v.viewport.read(cx).camera.distance;
            v.activate_explode_from_menu(0, cx);
            assert_eq!(v.explode, Some(0));
            assert!(v.explode_progress.is_none(), "no playback");
            let after = v.result.instance_bounds(pin).unwrap();
            assert!((after.0.z - before.0.z - 0.5).abs() < 1e-6, "exploded at once: {before:?} -> {after:?}");
            assert!(v.viewport.read(cx).animation().is_none(), "no animation");
            assert!(v.viewport.read(cx).camera.distance > distance, "zoomed out to the exploded pin at once");
        });
    })
    .unwrap();
}

/// Live extras-1118: opening Linear pattern adds its red row at once, counted in "Instances (N)".
#[gpui::test]
fn an_opened_linear_pattern_counts_its_row(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "pattern-count");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let before = v.instance_count(cx);
            v.open_pattern(super::PatternDialogKind::Linear, w, cx);
            assert!(v.pending_pattern_row(cx).is_some(), "the red row");
            assert_eq!(v.instance_count(cx), before + 1);
        });
    })
    .unwrap();
}

/// Live snapmode-1118 / snap-diag-1118: Esc during a snap-mode drag doesn't cancel it; off every part the connector
/// stays put and a release there moves nothing (no message); snap mode stays on.
#[gpui::test]
fn a_snap_mode_drag_keeps_its_move_through_esc_and_stays_in_snap_mode(cx: &mut TestAppContext) {
    use noshape_scene::{asm_scene::AsmPickTarget, builder::PickKind};
    let (window, view) = pattern_assembly(cx, "snap-drag");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            let before = v.result.instance_bounds(pin).unwrap();
            v.toggle_snap(w, cx);
            let top = face_connector(v, "Part 2 <1>", true, cx);
            let Reference::Implicit { on, .. } = &top.reference else { panic!("an implicit connector") };
            let occ = v.result.occurrences.iter().position(|o| o.top() == pin).unwrap();
            v.hover = Some(AsmPickTarget { path: vec![pin], reference: (**on).clone(), kind: PickKind::Face, occ, anchor: None });
            let at = v.viewport.read(cx).project(v.result.occurrences[occ].transform.transform_point3(glam::DVec3::ZERO)).unwrap();
            v.pointer = at;
            v.start_snap_drag(at, cx);
            // Far from everything: off every part, the pin stays where it was.
            v.update_snap_drag(at + glam::DVec2::new(0.0, -300.0), cx);
            assert!(v.snap_drag.as_ref().is_some_and(|s| s.off_parts));
            assert!(v.result.instance_bounds(pin).unwrap().0.distance(before.0) < 1e-9, "doesn't follow off the parts");
            v.on_cancel(&crate::actions::Cancel, w, cx);
            assert!(v.snap_drag.is_some(), "Esc doesn't cancel the drag");
            v.finish_snap_drag(w, cx);
            assert!(v.message.is_none(), "{:?}", v.message);
            assert!(v.snap.is_some(), "snap mode stays on");
        });
    })
    .unwrap();
}

/// Live snapmode-1118: a snap-mode drag of the pin's top-face connector onto the block's top-face centre lands it there
/// with the frames aligned (the tops flush, the pin sunk in), opens an uncommitted Fastened dialog with both
/// connectors (Ctrl+Z in it does nothing), and Esc closes it with no mate, the pin staying where it snapped.
#[gpui::test]
fn a_snap_mode_drag_lands_aligned_and_opens_an_uncommitted_fastened(cx: &mut TestAppContext) {
    use noshape_scene::{asm_scene::AsmPickTarget, builder::PickKind};
    let (window, view) = pattern_assembly(cx, "snap-onto");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            let mates = v.assembly(cx).unwrap().mates.len();
            v.toggle_snap(w, cx);
            let top = face_connector(v, "Part 2 <1>", true, cx);
            let Reference::Implicit { on, .. } = &top.reference else { panic!("an implicit connector") };
            let occ = v.result.occurrences.iter().position(|o| o.top() == pin).unwrap();
            v.hover = Some(AsmPickTarget { path: vec![pin], reference: (**on).clone(), kind: PickKind::Face, occ, anchor: None });
            let units = v.units(cx);
            let from = v.viewport.read(cx).project(v.result.connector_world(&top, &units).unwrap().origin).unwrap();
            v.pointer = from;
            v.start_snap_drag(from, cx);
            let target = face_connector(v, "Part 1 <1>", true, cx);
            let onto = v.viewport.read(cx).project(v.result.connector_world(&target, &units).unwrap().origin).unwrap();
            v.update_snap_drag(onto + glam::DVec2::new(5.0, 5.0), cx);
            v.finish_snap_drag(w, cx);
            let (a, b) = (v.result.connector_world(&top, &units).unwrap(), v.result.connector_world(&target, &units).unwrap());
            assert!(a.origin.distance(b.origin) < 1e-9 && a.z().dot(b.z()) > 0.999, "aligned onto the centre: {a:?} {b:?}");
            let s = v.mate.as_ref().expect("the Fastened dialog");
            let d = s.dialog.read(cx);
            assert!(d.mate_type() == MateType::Fastened && d.connectors_complete());
            assert_eq!(v.assembly(cx).unwrap().mates.len(), mates, "nothing committed");
            assert!(v.snap.is_some(), "snap mode stays on");
            let slots = v.mate.as_ref().unwrap().dialog.read(cx).slots();
            v.mate_undo(false, cx);
            assert_eq!(v.mate.as_ref().unwrap().dialog.read(cx).slots(), slots, "Ctrl+Z does nothing");
            v.on_cancel(&crate::actions::Cancel, w, cx);
            assert!(v.mate.is_none());
            assert_eq!(v.assembly(cx).unwrap().mates.len(), mates, "no mate");
        });
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| {
        let v = view.read(cx);
        let units = v.units(cx);
        let (a, b) = (face_connector(v, "Part 2 <1>", true, cx), face_connector(v, "Part 1 <1>", true, cx));
        assert!(v.result.connector_world(&a, &units).unwrap().origin.distance(v.result.connector_world(&b, &units).unwrap().origin) < 1e-9, "stays where it snapped");
    });
}

/// Live asm-feel3-1118: Shift+N on a selected mate row does nothing (its menu's Rename renames it); on a selected
/// instance it renames.
#[gpui::test]
fn shift_n_renames_a_selected_instance_but_not_a_mate(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "shift-n");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.accept_mate(w, cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selected_mate = v.assembly(cx).unwrap().mates.first().map(|m| m.id);
            v.rename_selected(w, cx);
            assert!(v.ui.rename.is_none(), "a mate: nothing");
            v.selected_mate = None;
            v.selected = vec![instance(v, "Part 1 <1>", cx)];
            v.rename_selected(w, cx);
            assert!(v.ui.rename.is_some(), "an instance: renamed");
        })
    })
    .unwrap();
}

/// Live asm-feel3-1118: a dragged instance row lands above the row under the pointer, or below it past the row's
/// middle (the drop line switches there); the drag starts past 7 px (8 drags, 6 doesn't).
#[test]
fn a_dragged_instance_row_lands_by_the_rows_middle() {
    use super::behaviors::{LIST_DRAG_PX, list_drop_before};
    let [a, b, c] = [1, 2, 3].map(InstanceId);
    let order = [a, b, c];
    assert_eq!(list_drop_before(&order, c, a, false), Some(a), "upper half of A: above A");
    assert_eq!(list_drop_before(&order, c, a, true), Some(b), "lower half of A: between A and B");
    assert_eq!(list_drop_before(&order, b, a, true), Some(c), "B onto A's lower half: skips itself");
    assert_eq!(list_drop_before(&order, a, c, true), None, "lower half of the last row: the end");
    const { assert!(6.0 <= LIST_DRAG_PX && LIST_DRAG_PX < 8.0) };
}

/// Live manip-feel-1118 m9: an arrow drag keeps where the arrow was grabbed: at the press point it reads 0 mm (ours jumped
/// the instance's origin to the pointer, 4.3 mm after 6 px).
#[gpui::test]
fn a_triad_arrow_drag_keeps_its_grab_point(cx: &mut TestAppContext) {
    use gpui_kit::Modifiers;
    use crate::viewport::ViewportEvent;
    let (window, view) = pattern_assembly(cx, "arrow-grab");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selected = vec![instance(v, "Part 2 <1>", cx)];
            let f = v.manipulator(cx).expect("a triad");
            let shaft = v.viewport.read(cx).project(f.origin + f.x() * v.manip_size(cx) * 0.6).unwrap();
            v.manip_hover = Some(ManipHandle::Axis(0));
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: shaft, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.drag.as_mut().expect("an arrow drag").started = true;
            v.on_viewport_event(&vp, &ViewportEvent::ToolDrag { position: shaft }, w, cx);
            let value = v.drag.as_ref().unwrap().value;
            assert!(value.abs() < 1e-9, "0 at the grab point: {value}");
        })
    })
    .unwrap();
}

/// Live asm-dstates-1118: Display states opens with its "New display state name" box focused; Esc there closes the
/// panel, nothing focused (qa asm-display-states-open-escape).
#[gpui::test]
fn display_states_opens_with_its_name_box_focused(cx: &mut TestAppContext) {
    use gpui_kit::Focusable as _;
    let path = pattern_document(cx, "ds-focus");
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.toggle_display_states(w, cx);
            let st = v.ui.ds_name.clone().expect("the name box");
            assert!(st.read(cx).focus_handle(cx).is_focused(w), "focused");
        })
    })
    .unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_keystrokes("escape");
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        let v = view.read(cx);
        assert!(!v.ui.display_states, "Esc closed the panel");
        assert!(!v.ui.ds_name.as_ref().unwrap().read(cx).focus_handle(cx).is_focused(w), "the name box lost the keyboard");
    })
    .unwrap();
}

/// Live snap-diag-1118 (retaken): a dot of the face under the pointer takes a snap drag's connector within 51 px in x
/// and in y (45° up-left at (51, 51), straight up at 50); 52 px either way doesn't.
#[test]
fn a_snap_drags_reach_is_51_px_in_x_and_y() {
    use crate::assembly_view::panels::in_snap_reach;
    let at = glam::DVec2::new(400.0, 300.0);
    for (d, reach) in [((51.0, 51.0), true), ((0.0, -50.0), true), ((26.0, 49.0), true), ((0.0, -52.0), false), ((52.0, 10.0), false)] {
        assert_eq!(in_snap_reach(at + glam::DVec2::from(d), at), reach, "{d:?}");
    }
}

/// Live asm/feel-1117 c-a-rowhover: hovering an instance's row outlines the part in the graphics, in hover orange, with
/// no tint on its faces (was: the accent fill).
#[gpui::test]
fn a_hovered_instance_row_outlines_the_part_without_a_tint(cx: &mut TestAppContext) {
    use noshape_render::scene::state;
    let (window, view) = pattern_assembly(cx, "row-hover");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            v.ui.list_hover = Some(instance(v, "Part 1 <1>", cx));
            v.refresh_scene(false, cx);
            let vp = v.viewport.read(cx);
            let scene = vp.scene();
            assert!(scene.hover_outline, "the hover outline pass");
            assert!(scene.states.iter().any(|s| s & (state::HOVER | state::OUTLINE) == state::HOVER | state::OUTLINE), "its faces outlined");
            assert!(scene.states.iter().all(|s| s & state::ACCENT == 0), "no tint");
        })
    })
    .unwrap();
}

/// A started triad drag of `handle` on the selected instance's triad, `value` along / about it.
fn triad_drag(v: &AssemblyView, handle: ManipHandle, value: f64, cx: &gpui_kit::App) -> super::DragState {
    let f = v.manipulator(cx).expect("a triad");
    let k = match handle {
        ManipHandle::Axis(k) => k,
        ManipHandle::Ring(k) => ManipHandle::ring_axis(k),
        ManipHandle::Plane(k) => k,
        ManipHandle::Center => 2,
    };
    let top = v.selected[0];
    super::DragState {
        top,
        press: glam::DVec2::ZERO,
        pressed_at: std::time::Instant::now(),
        started: true,
        local: glam::DVec3::ZERO,
        plane_point: f.origin,
        plane_normal: [f.x(), f.y(), f.z()][k],
        pending: None,
        busy: true,
        handle: Some(handle),
        ids: vec![top],
        start: Default::default(),
        ring_v0: [f.x(), f.y(), f.z()][(k + 1) % 3],
        value,
        grab0: f.origin,
        offset: glam::DVec3::ZERO,
        triad_preview: None,
        explode: false,
        nested: None,
        solved: None,
        origin: Default::default(),
        landed: true,
        released: false,
        no_box: false,
    }
}

/// Live manip-feel-1118 (m9): an arrow / ring drag snaps (0.1 mm steps here; 5° near a multiple) but not with Shift
/// held (3.827 mm, 28.352 deg).
#[test]
fn shift_frees_a_triad_drag_from_its_snap() {
    assert!((super::triad_arrow_value(0.003827, Some(1e-4), false) - 0.0038).abs() < 1e-12);
    assert_eq!(super::triad_arrow_value(0.003827, Some(1e-4), true), 0.003827);
    let a = 28.352f64.to_radians();
    assert!((super::triad_ring_value(a, 100.0, false) - 30f64.to_radians()).abs() < 1e-12, "within 2° of 30: snapped");
    assert_eq!(super::triad_ring_value(a, 100.0, true), a);
    let free = 22.82f64.to_radians();
    assert_eq!(super::triad_ring_value(free, 100.0, false), free, "between multiples: free");
}

/// Live manip-feel-1118 (m9-grid): during an arrow drag the other handles fade and a line runs from a tick at the
/// arrow's start to where it is; a ring drag draws an arc from a tick through its angle. Esc mid-drag removes the
/// value box, the drag going on (the release keeps the move, with no box).
#[gpui::test]
fn a_triad_drag_fades_the_other_handles_draws_its_dimension_and_esc_drops_the_box(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "triad-drag");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            v.selected = vec![instance(v, "Part 1 <1>", cx)];
            v.drag = Some(triad_drag(v, ManipHandle::Axis(0), 0.004, cx));
            let mut h = Highlights { manipulator: v.manipulator(cx).map(|f| (f, v.manip_size(cx), Some(ManipHandle::Axis(0)))), ..Default::default() };
            v.triad_drag_marks(&mut h, cx);
            assert!(h.manip_dragging, "the other handles fade");
            let Some(crate::assembly_scene::DragDim::Line { start, end, tick }) = h.drag_dim else { panic!("a line: {:?}", h.drag_dim) };
            assert!(((end - start).length() - 0.004).abs() < 1e-9 && tick.length() > 0.0 && tick.dot(end - start).abs() < 1e-12);
            v.drag = Some(triad_drag(v, ManipHandle::Ring(0), 0.5, cx));
            let mut h = Highlights { manipulator: h.manipulator, ..Default::default() };
            v.triad_drag_marks(&mut h, cx);
            assert!(matches!(h.drag_dim, Some(crate::assembly_scene::DragDim::Arc { sweep, .. }) if (sweep - 0.5).abs() < 1e-12), "an arc: {:?}", h.drag_dim);
        })
    })
    .unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.on_cancel(&crate::actions::Cancel, w, cx))).unwrap();
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            assert!(v.drag.as_ref().is_some_and(|d| d.no_box), "the drag goes on, its box gone");
            assert!(v.render_manip_value(cx).is_none(), "no value box");
        })
    })
    .unwrap();
}

/// Live2 fields.json contextMenus.mixedSelection: instances, the Origin and a mate selected give a reduced menu
/// (Hide… Isolate… … Clear selection); no Suppress, Fix, Copy, Export… or Delete.
#[gpui::test]
fn instances_with_a_mate_selected_have_lives_reduced_menu(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "mixed-menu");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (a, b) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
            v.edit("Insert : mate", cx, |asm| {
                asm.mates.push(noshape_model::assembly::Mate::new(noshape_model::assembly::MateId(901), "Fastened 1".into(), MateType::Fastened, a, b));
            });
            let (p1, p2) = (instance(v, "Part 1 <1>", cx), instance(v, "Part 2 <1>", cx));
            v.selected = vec![p1, p2];
            v.selected_mate = Some(noshape_model::assembly::MateId(901));
            let got: Vec<String> = labels(&v.instance_menu(p1, false, cx)).into_iter().filter(|l| l != "—").collect();
            let live = [
                "Hide", "Hide other instances", "Hide all instances", "Isolate…", "Make transparent…", "Use best available tessellation", "Check interference…",
                "Show mates", "Hide mates", "Replace instances…", "Move to new subassembly", "Change to version…", "Zoom to selection",
                "Create empty subassembly", "Clear selection",
            ];
            assert_eq!(got, live);
        })
    })
    .unwrap();
}

/// Live asm-insert-esc-pending: Esc drops the instance on the pointer (Redo offered, the dialog stays); a second Esc
/// closes the dialog and leaves no Redo.
#[gpui::test]
fn closing_the_insert_dialog_after_esc_leaves_no_redo(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "insert-esc");
    let esc = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.on_cancel(&crate::actions::Cancel, w, cx))).unwrap();
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.toggle_insert(w, cx);
            let studio = v.doc.read(cx).session.doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
            let id = v.insert_part(studio, None, cx).unwrap();
            v.start_placing(vec![id], cx);
            assert!(v.placing.is_some(), "on the pointer");
        })
    })
    .unwrap();
    esc(cx);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.insert_open && v.placing.is_none(), "the instance dropped, the dialog open");
        assert!(v.doc.read(cx).can_redo(), "Redo offered");
    });
    esc(cx);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(!v.insert_open, "closed");
        assert!(!v.doc.read(cx).can_redo(), "no Redo left");
    });
    draw(cx, window);
}

/// Live2 asm-feel (1): a mate row's flyout heads with the mate's name and state, then shows its parameters read-only:
/// only the type while they are at their defaults, every one with Limits ticked ("No minimum" / "No maximum" for a
/// limit left open).
#[gpui::test]
fn a_mate_rows_flyout_shows_the_type_alone_until_a_parameter_is_set(cx: &mut TestAppContext) {
    use crate::assembly_flyout::FlyRow;
    use noshape_model::assembly::{Limits, Mate, MateId};
    let (_window, view) = pattern_assembly(cx, "mate-flyout");
    cx.update(|cx| {
        let v = view.read(cx);
        let (a, b) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
        // A Fastened lists its two checkboxes at their defaults (live2 errors-1118 tips).
        let f = Mate::new(MateId(901), "Fastened 1".into(), MateType::Fastened, a.clone(), b.clone());
        assert_eq!(v.mate_flyout(&f).1, vec![FlyRow::Text("Fastened".into()), FlyRow::Check("Offset".into(), false)]);
        let mut m = Mate::new(MateId(900), "Revolute 1".into(), MateType::Revolute, a, b);
        let (head, rows) = v.mate_flyout(&m);
        assert!(head.starts_with("Revolute 1"), "{head}");
        assert_eq!(rows, vec![FlyRow::Text("Revolute".into())], "defaults: the type alone");
        m.limits = vec![("rz".into(), Limits { min: None, max: None })];
        let (_, rows) = v.mate_flyout(&m);
        assert_eq!(rows, vec![FlyRow::Text("Revolute".into()), FlyRow::Check("Offset".into(), false), FlyRow::Check("Limits".into(), true), FlyRow::Text("No minimum".into()), FlyRow::Text("No maximum".into())], "Limits ticked: every parameter");
    });
}

/// live2 asm-feel2 (a): every instance row shows its name as its tooltip, whether it fits or not, a moment after the
/// pointer comes to rest on it (~0.1 s); the next row's replaces it at once.
#[gpui::test]
fn every_instance_row_tooltips_its_name(cx: &mut TestAppContext) {
    let (window, _view) = pattern_assembly(cx, "row-tips");
    draw(cx, window);
    for (name, sel) in [("Part 1 <1>", "inst-row Part 1 <1>"), ("Part 2 <1>", "inst-row Part 2 <1>")] {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let b = vcx.debug_bounds(sel).unwrap_or_else(|| panic!("{name}'s row"));
        crate::ui_script::clear_tooltip();
        vcx.simulate_mouse_move(b.center(), None, gpui_kit::Modifiers::default());
        cx.executor().advance_clock(std::time::Duration::from_millis(40));
        draw(cx, window);
        // The first only after a pause; the next row's at once in its place (live2 13:05 (1): ~17 ms).
        if name == "Part 1 <1>" {
            assert_eq!(crate::ui_script::shown_tooltip(), None, "{name}: not at once");
        } else {
            assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some(name), "{name}: replaces the last at once");
        }
        cx.executor().advance_clock(std::time::Duration::from_millis(400));
        draw(cx, window);
        assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some(name), "{name}: its name");
    }
}

/// Live asm-export-1150: an assembly's Export shows "<name> is being prepared for export…" while its file is written
/// off the UI thread; the bell's "Export completed" and (Options › Store file in a new tab) the file's tab right after
/// the assembly's come when it is done.
#[gpui::test]
fn an_assembly_export_is_written_off_the_ui_thread(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "export-off-thread");
    let s = noshape_ops::export::ExportSettings { format: noshape_ops::export::PartFormat::Step, options: 2, ..Default::default() };
    cx.update(|cx| view.update(cx, |v, cx| v.run_asm_export(s, "l2 exp asm".into(), None, None, cx)));
    let bell = |cx: &gpui_kit::App| cx.try_global::<crate::notifications::Notifications>().map(|n| n.items.iter().map(|i| (i.title.clone(), i.body.clone())).collect::<Vec<_>>()).unwrap_or_default();
    cx.update(|cx| {
        let shown = view.read(cx).message.clone().map(|m| m.1);
        assert_eq!(shown.as_deref(), Some("l2 exp asm is being prepared for export. You will receive a notification with its status."));
        assert!(bell(cx).is_empty(), "written on the UI thread: {:?}", bell(cx));
    });
    draw(cx, window);
    assert_eq!(cx.update(|cx| bell(cx)), vec![("Export completed".to_string(), "l2 exp asm was exported successfully.".to_string())]);
    let tabs: Vec<String> = cx.update(|cx| {
        let v = view.read(cx);
        let d = &v.doc.read(cx).session.doc;
        let ordered = d.ordered_tabs();
        let at = ordered.iter().position(|t| t.id == v.tab).expect("the assembly's tab");
        ordered[at..].iter().take(2).map(|t| t.name.clone()).collect()
    });
    assert_eq!(tabs, ["Assembly 1", "l2 exp asm.step"]);
}

/// Live asm-fixed-drag-1118 / icons2 10-23 (b): a press-drag on a fixed instance shows the no-drop cursor and moves
/// nothing; the release brings the default cursor back at once (ours kept ⊘ until a later move onto the part).
#[gpui::test]
fn a_fixed_instances_drag_drops_the_no_drop_cursor_at_the_release(cx: &mut TestAppContext) {
    use crate::cursor::AppCursor;
    use crate::viewport::ViewportEvent;
    use gpui_kit::Modifiers;
    let (window, view) = pattern_assembly(cx, "fixed-drag");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let id = instance(v, "Part 1 <1>", cx);
            v.edit("Fix", cx, |a| {
                if let Some(i) = a.instances.iter_mut().find(|i| i.id == id) {
                    i.fixed = true;
                }
            });
        })
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    draw(cx, window);
    let cursor = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).tool_cursor);
    let (at, before) = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let id = instance(v, "Part 1 <1>", cx);
                assert!(v.assembly(cx).unwrap().instance(id).unwrap().fixed, "fixed");
                let (lo, hi) = v.result.instance_bounds(id).unwrap();
                let at = v.viewport.read(cx).project((lo + hi) * 0.5).unwrap();
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                // (A drag starts past 2 px and 50 ms.)
                std::thread::sleep(std::time::Duration::from_millis(60));
                v.on_viewport_event(&vp, &ViewportEvent::ToolDrag { position: at + glam::DVec2::new(20.0, 0.0) }, w, cx);
                (at, v.result.solved.placement(id))
            })
        })
        .unwrap();
    assert_eq!(cursor(cx), Some(AppCursor::NoDrop), "⊘ while dragging it");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at + glam::DVec2::new(20.0, 0.0), modifiers: Modifiers::default() }, w, cx);
        })
    })
    .unwrap();
    assert_eq!(cursor(cx), None, "the default cursor at the release");
    cx.update(|cx| {
        let v = view.read(cx);
        assert_eq!(v.result.solved.placement(instance(v, "Part 1 <1>", cx)), before, "nothing moved");
    });
}

/// Live icons2 10-23 (a), asm-fixed-drag-1118: a free instance's press-drag keeps the default cursor and, Snap mode
/// off, carries the part with the pointer in the plane of the screen (its centre the pointer's 20 px away), not
/// turning it (the point drag turned it 1–2°) and no jump onto a snap target.
#[gpui::test]
fn a_free_instances_drag_keeps_the_default_cursor_and_follows_the_pointer(cx: &mut TestAppContext) {
    use crate::viewport::ViewportEvent;
    use gpui_kit::Modifiers;
    let (window, view) = pattern_assembly(cx, "free-drag");
    draw(cx, window);
    let cursor = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).tool_cursor);
    let centre = |v: &AssemblyView, cx: &gpui_kit::App| {
        let id = instance(v, "Part 1 <1>", cx);
        let (lo, hi) = v.result.instance_bounds(id).unwrap();
        v.viewport.read(cx).project((lo + hi) * 0.5).unwrap()
    };
    let to = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                assert!(!v.assembly(cx).unwrap().instance(instance(v, "Part 1 <1>", cx)).unwrap().fixed, "free");
                assert!(v.snap.is_none(), "Snap mode off");
                let at = centre(v, cx);
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                // (A drag starts past 2 px and 50 ms.)
                std::thread::sleep(std::time::Duration::from_millis(60));
                let to = at + glam::DVec2::new(20.0, 0.0);
                v.on_viewport_event(&vp, &ViewportEvent::ToolDrag { position: to }, w, cx);
                to
            })
        })
        .unwrap();
    assert_eq!(cursor(cx), None, "the default cursor while dragging it");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: to, modifiers: Modifiers::default() }, w, cx);
        })
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(cursor(cx), None, "the default cursor at the release");
    cx.update(|cx| {
        let v = view.read(cx);
        let now = centre(v, cx);
        assert!(now.distance(to) < 1.0, "the part's centre with the pointer: {now:?}, pointer {to:?}");
        let p = v.result.solved.placement(instance(v, "Part 1 <1>", cx)).unwrap();
        assert_eq!(p.rotation, [0.0, 0.0, 0.0, 1.0], "moved, not turned");
    });
}

/// Live drag-delay-1120 (feel cov-1120 A1): a press on a free instance moved 2 px is a click (selected, not moved); its
/// first move past 2 px and 50 ms drags, taking the whole move from the press, and after the release nothing is selected.
#[gpui::test]
fn a_free_instance_drags_past_2_px_and_takes_the_whole_move(cx: &mut TestAppContext) {
    use crate::viewport::ViewportEvent;
    use gpui_kit::Modifiers;
    let (window, view) = pattern_assembly(cx, "drag-start");
    draw(cx, window);
    let centre = |v: &AssemblyView, cx: &gpui_kit::App| {
        let (lo, hi) = v.result.instance_bounds(instance(v, "Part 1 <1>", cx)).unwrap();
        v.viewport.read(cx).project((lo + hi) * 0.5).unwrap()
    };
    let press_move_release = |cx: &mut TestAppContext, by: f64| {
        let to = cx
            .update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let at = centre(v, cx);
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    let to = at + glam::DVec2::new(by, 0.0);
                    v.on_viewport_event(&vp, &ViewportEvent::ToolDrag { position: to }, w, cx);
                    v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: to, modifiers: Modifiers::default() }, w, cx);
                    to
                })
            })
            .unwrap();
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        to
    };
    let before = cx.update(|cx| view.read(cx).result.solved.placement(instance(view.read(cx), "Part 1 <1>", cx)));
    press_move_release(cx, 2.0);
    cx.update(|cx| {
        let v = view.read(cx);
        let id = instance(v, "Part 1 <1>", cx);
        assert_eq!(v.result.solved.placement(id), before, "2 px: not moved");
        assert!(v.selected.contains(&id), "2 px: a click, selected");
    });
    cx.update(|cx| view.update(cx, |v, _| v.clear_selection()));
    let to = press_move_release(cx, 5.0);
    cx.update(|cx| {
        let v = view.read(cx);
        let now = centre(v, cx);
        assert!(now.distance(to) < 1.0, "5 px: the part's centre with the pointer, the whole move: {now:?}, pointer {to:?}");
        assert!(v.selected.is_empty(), "nothing selected after the drag");
    });
}

/// Live insert-sort-1118: Insert › Other documents lists Documents newest first by default; a Sort menu pick re-sorts
/// them (names case-insensitive, Modified by by author) and is remembered across a restart; Esc closes the menu, not the
/// dialog.
#[gpui::test]
fn other_documents_sort_by_their_menu_and_remember_it(cx: &mut TestAppContext) {
    use noshape_ops::prefs::{DocumentSort, FolderSort, InsertMemory};
    let doc = |name: &str, modified: i64, by: &str| crate::library::DocEntry {
        path: std::path::PathBuf::from(format!("/nowhere/{name}.noshape")),
        name: name.into(),
        modified,
        modified_by: by.into(),
        owner: by.into(),
        description: String::new(),
        workspace: "Main".into(),
        thumbnail: None,
        recent_rank: None,
        broken: false,
        trashed: false,
    };
    let docs = vec![doc("bracket", 2, "Zoe"), doc("Axle", 1, "amir"), doc("cover", 3, "Bea")];
    let order = |sort: DocumentSort| {
        let mut d = docs.clone();
        super::panels::sort_documents(&mut d, sort);
        d.iter().map(|d| d.name.clone()).collect::<Vec<_>>()
    };
    assert_eq!(DocumentSort::default(), DocumentSort::ModifiedNewest, "live's default");
    assert_eq!(order(DocumentSort::ModifiedNewest), ["cover", "bracket", "Axle"]);
    assert_eq!(order(DocumentSort::ModifiedOldest), ["Axle", "bracket", "cover"]);
    assert_eq!(order(DocumentSort::NameAsc), ["Axle", "bracket", "cover"]);
    assert_eq!(order(DocumentSort::NameDesc), ["cover", "bracket", "Axle"]);
    assert_eq!(order(DocumentSort::ModifiedByAsc), ["Axle", "cover", "bracket"]);
    assert_eq!(order(DocumentSort::ModifiedByDesc), ["bracket", "cover", "Axle"]);
    let mut dirs = vec![std::path::PathBuf::from("/x/beta"), std::path::PathBuf::from("/x/Alpha")];
    super::panels::sort_folders(&mut dirs, FolderSort::NameAsc);
    assert_eq!(dirs[0].file_name().unwrap(), "Alpha");
    assert_eq!(FolderSort::ALL.map(|s| s.label()), ["Name: A → Z", "Name: Z → A", "Last modified"]);

    let (window, view) = pattern_assembly(cx, "insert-docs-sort");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.toggle_insert(w, cx);
            v.ui.insert_source = 1;
            v.ui.insert_memory = Some(InsertMemory::default());
            v.ui.insert_other_cat = Some(2);
            v.ui.insert_other_docs = Some(docs.clone());
            v.ui.insert_sort_menu = Some(1);
            let _ = v.render_insert_other(cx);
            v.pick_insert_sort(1, 0);
            assert_eq!(v.ui.insert_sort_menu, None, "a pick closes the menu");
            assert_eq!(v.insert_memory().documents, DocumentSort::NameAsc);
            assert_eq!(InsertMemory::load().documents, DocumentSort::NameAsc, "remembered across a restart");
            assert_eq!(InsertMemory::load().category, Some(2), "and the location");
            v.ui.insert_sort_menu = Some(1);
            v.on_cancel(&crate::actions::Cancel, w, cx);
            assert!(v.ui.insert_sort_menu.is_none() && v.insert_open, "Esc: the menu closes, the dialog stays");
        })
    })
    .unwrap();
    draw(cx, window);
}

/// qa session-asm-1119 (2): with Fastened 1 in place, a new mate opens as "Fastened 2"; switching its type names it
/// by the new type's own count, "Revolute 1", then "Slider 1", as live.
#[gpui::test]
fn a_new_mates_type_switch_takes_that_types_next_name(cx: &mut TestAppContext) {
    use noshape_model::assembly::{Mate, MateId};
    let (window, view) = pattern_assembly(cx, "mate-type-names");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let (a, b) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
            v.edit("Fastened", cx, |asm| {
                let id = MateId(asm.next_id());
                asm.mates.push(Mate::new(id, "Fastened 1".into(), MateType::Fastened, a, b));
            });
            v.start_mate(MateType::Fastened, w, cx);
        })
    })
    .unwrap();
    let name = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).mate.as_ref().unwrap().dialog.read(cx).name.clone());
    assert_eq!(name(cx), "Fastened 2");
    let switch = |cx: &mut TestAppContext, t: MateType| {
        cx.update(|cx| {
            let d = view.read(cx).mate.as_ref().unwrap().dialog.clone();
            d.update(cx, |d, cx| d.set_type(t, cx));
        })
    };
    switch(cx, MateType::Revolute);
    assert_eq!(name(cx), "Revolute 1");
    switch(cx, MateType::Slider);
    assert_eq!(name(cx), "Slider 1");
    switch(cx, MateType::Fastened);
    assert_eq!(name(cx), "Fastened 2");
}

/// qa session-asm-1119 (2b): ✕ / Esc in the Insert dialog takes its instances back and shows the blue "Assembly
/// insertion has been cancelled. Restore" bubble; Restore brings them back.
#[gpui::test]
fn cancelling_an_insertion_offers_restore(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "insert-restore");
    let count = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).assembly(cx).unwrap().instances.len());
    let before = count(cx);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.toggle_insert(w, cx);
            let studio = v.doc.read(cx).session.doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
            let id = v.insert_part(studio, None, cx).unwrap();
            v.start_placing(vec![id], cx);
            v.finish_placing(true, w, cx);
        })
    })
    .unwrap();
    assert_eq!(count(cx), before + 1, "placed in the dialog");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_insert(w, cx))).unwrap();
    assert_eq!(count(cx), before, "taken back");
    assert!(cx.update(|cx| view.read(cx).ui.insert_restore.is_some()), "the cancelled bubble with Restore");
    draw(cx, window);
    cx.update(|cx| view.update(cx, |v, cx| v.restore_insert(cx)));
    assert_eq!(count(cx), before + 1, "Restore brings it back");
    assert!(cx.update(|cx| view.read(cx).ui.insert_restore.is_none()), "the bubble gone");
}

/// Live 1704 (skasm/snapinsert-1704): in Snap mode a pin placed with its explicit mate connector on the bottom rim (Z
/// out of the pin) snaps, near the block's top face centre, onto that face's centroid connector with the frames
/// coincident: turned 180° about X, at (−50, 20, 10) mm, hanging in the block; the click adds Fastened 1 between them.
#[gpui::test]
fn a_snap_insert_onto_a_plain_face_takes_its_centroid_and_turns_about_x(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-snap-insert-face-{}", std::process::id()));
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
        let rim = call(&mut ctx, "query_entities", json!({ "kind": "edges", "at": [[75, 10, 0]] }))["edges"][0]["id"].clone();
        call(&mut ctx, "feature_add", json!({ "type": "mateConnector", "params": { "originQuery": [rim], "entityInferenceType": "CENTER" } }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>"], "fixed": true }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    }
    let (window, view) = open_assembly(cx, &path);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let pin = instance(v, "Part 2 <1>", cx);
            let mates = v.assembly(cx).unwrap().mates.len();
            v.toggle_snap(w, cx);
            v.start_placing(vec![pin], cx);
            v.ui.place_moved = true;
            let target = face_connector(v, "Part 1 <1>", true, cx);
            let units = v.units(cx);
            let centre = v.viewport.read(cx).project(v.result.connector_world(&target, &units).unwrap().origin).unwrap();
            v.pointer = centre + glam::DVec2::new(6.0, 3.0);
            assert!(v.update_placing(cx), "snapped");
            assert_eq!(v.ui.place_snap.as_ref().map(|(_, d)| d), Some(&target), "onto the face's centroid connector");
            let p = v.result.solved.placement(pin).unwrap();
            let r = glam::DQuat::from_array(p.rotation);
            let near = |a: glam::DVec3, b: glam::DVec3| a.distance(b) < 1e-9;
            assert!(near(r * glam::DVec3::X, glam::DVec3::X) && near(r * glam::DVec3::Z, -glam::DVec3::Z), "180° about X: {r:?}");
            assert!(near(glam::DVec3::from(p.translation), glam::DVec3::new(-0.05, 0.02, 0.01)), "{:?}", p.translation);
            v.finish_placing(true, w, cx);
            let a = v.assembly(cx).unwrap();
            assert_eq!(a.mates.len(), mates + 1);
            let m = a.mates.last().unwrap();
            assert_eq!((m.name.as_str(), m.mate_type), ("Fastened 1", MateType::Fastened));
            assert_eq!(m.b, target);
        })
    })
    .unwrap();
}

/// A block (Part 1) and a pin (Part 2) inserted into Assembly 1, then `make`.
fn two_parts_document(cx: &mut TestAppContext, name: &str, make: impl FnOnce(&mut Ctx)) -> std::path::PathBuf {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("asm.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "circle", "points": [[70, 10], [75, 10]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    make(&mut ctx);
    path
}

/// Live2 errors-1119 a2: with two parts fixed, the amber warning sits on each fixed icon and on the root row's (its name
/// cut short so the icon shows), with no second warning beside; the rows span the list, so a row's flyout starts at the
/// list's right edge.
#[gpui::test]
fn two_fixed_parts_warn_on_their_fixed_icons_and_the_root(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "fixed-warn", |ctx| {
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>", "Part 2 <1>"], "fixed": true }));
        call(ctx, "tab_rename", json!({ "tab": "Assembly 1", "name": "start pin-block-asm-drawing assembly with a long name" }));
    });
    let (window, view) = open_assembly(cx, &path);
    draw(cx, window);
    let list_w = cx.update(|cx| crate::ui::left_panel_w(view.read(cx).tab));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for part in ["Part 1 <1>", "Part 2 <1>"] {
        let icon = vcx.debug_bounds(format!("fixed {part}").leak()).unwrap_or_else(|| panic!("{part}'s fixed icon"));
        let badge = vcx.debug_bounds(format!("fixed-warning {part}").leak()).unwrap_or_else(|| panic!("{part}'s warning on its fixed icon"));
        assert!(icon.contains(&badge.origin), "{part}: the warning on the icon ({icon:?}, {badge:?})");
        assert!(vcx.debug_bounds(format!("warn {part}").leak()).is_none(), "{part}: no second warning beside");
        let row = vcx.debug_bounds(format!("inst-row {part}").leak()).unwrap();
        eprintln!("row {row:?} list_w {list_w}");
        assert!((f32::from(row.right()) - list_w).abs() <= 1., "{part}'s row spans the list ({row:?}, list {list_w})");
    }
    assert!(vcx.debug_bounds("asm-grounded-warning").is_some(), "the root's grounded icon carries the warning");
    let grounded = vcx.debug_bounds("asm-grounded").expect("the root's grounded icon");
    assert!(f32::from(grounded.right()) <= list_w, "the root's name is cut short so its icon shows ({grounded:?}, list {list_w})");
}

/// Live2 recheck-1119 (5) / (7): a mate row spans the list, so its flyout and tip bar start right of the list (live x
/// 243), not over it (ours started at 190, right of the row's text).
#[gpui::test]
fn a_mate_row_spans_the_list(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "mate-row-width", |ctx| {
        let top = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
        let (a, b) = (top(ctx, "Part 1 <1>"), top(ctx, "Part 2 <1>"));
        call(ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 2 <1>", "entity": b } }));
    });
    let (window, view) = open_assembly(cx, &path);
    draw(cx, window);
    let list_w = cx.update(|cx| crate::ui::left_panel_w(view.read(cx).tab));
    let row = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mate-row Revolute 1").expect("Revolute 1's row");
    assert!((f32::from(row.right()) - list_w).abs() <= 1., "the mate row spans the list ({row:?}, list {list_w})");
}

/// Live2 errors-1119 a5: a Revolute's minimum above its maximum disables ✓ and Enter (the dialog stays, nothing made).
#[gpui::test]
fn crossed_limits_keep_the_mate_dialog_open(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "crossed-limits");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            let dialog = v.mate.as_ref().unwrap().dialog.clone();
            dialog.update(cx, |d, cx| {
                d.type_limit("rz", false, "90", true, w, cx);
                d.type_limit("rz", true, "10", true, w, cx);
            });
        });
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            assert!(!v.mate.as_ref().unwrap().dialog.read(cx).can_accept(cx), "✓ disabled");
            v.accept_mate(w, cx);
            assert!(v.mate.is_some(), "Enter / ✓ leaves the dialog open");
            assert!(v.assembly(cx).unwrap().mates.is_empty(), "no mate made");
        });
    })
    .unwrap();
}

/// Live2 errors-1119 q3: the Insert dialog leaves out a Part Studio with no parts, its name searched for too, and a
/// search matching nothing says "No Part Studios match current search.".
#[gpui::test]
fn the_insert_dialog_leaves_out_an_empty_part_studio(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "insert-empty", |ctx| {
        call(ctx, "tab_create", json!({ "kind": "partstudio", "name": "start empty" }));
    });
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_insert(w, cx))).unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds("ins-row Part Studio 1").is_some(), "Part Studio 1 listed");
    assert!(vcx.debug_bounds("ins-row start empty").is_none(), "the empty studio left out");
    for (query, none) in [("empty", true), ("zzz", true), ("part", false)] {
        cx.update(|cx| view.update(cx, |v, cx| {
            v.ui.insert_query = query.into();
            cx.notify();
        }));
        draw(cx, window);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        assert!(vcx.debug_bounds("ins-row start empty").is_none(), "{query}: the empty studio left out");
        assert_eq!(vcx.debug_bounds("ins-no-match").is_some(), none, "{query}: the no-match line");
    }
}

/// Live2 asm-dialogs-1119 q-ins / q-ins-sort: Insert's Part Studios are listed by name A → Z by default, the Sort
/// button right of the search box turns it to Z → A, kept when the dialog is closed and reopened; the filters are
/// Parts, Sketches and Flat patterns, also with no sketches or sheet metal in the document.
#[gpui::test]
fn the_insert_dialogs_studios_sort_by_name_and_keep_the_sort(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "insert-sort", |ctx| {
        let dup = call(ctx, "tab_duplicate", json!({ "tab": "Part Studio 1" }))["tab"]["name"].as_str().unwrap().to_string();
        call(ctx, "tab_rename", json!({ "tab": dup, "name": "Alpha studio" }));
    });
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.insert_memory = Some(noshape_ops::prefs::InsertMemory::default());
            v.toggle_insert(w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    let first = |cx: &mut TestAppContext| {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let y = |vcx: &mut gpui_kit::VisualTestContext, n: &str| vcx.debug_bounds(format!("ins-row {n}").leak()).unwrap_or_else(|| panic!("{n} listed")).origin.y;
        if y(&mut vcx, "Alpha studio") < y(&mut vcx, "Part Studio 1") { "Alpha studio" } else { "Part Studio 1" }
    };
    assert_eq!(first(cx), "Alpha studio", "A → Z by default");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for id in ["ins-cur-sort", "ins-f-parts", "ins-f-sketch", "ins-f-flat"] {
        assert!(vcx.debug_bounds(id).is_some(), "{id} shown");
    }
    cx.update(|cx| view.update(cx, |v, cx| {
        v.pick_insert_sort(2, 1);
        v.insert_search_changed(cx);
        cx.notify();
    }));
    draw(cx, window);
    assert_eq!(first(cx), "Part Studio 1", "Z → A");
    for _ in 0..2 {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_insert(w, cx))).unwrap();
        draw(cx, window);
    }
    assert_eq!(first(cx), "Part Studio 1", "Z → A kept on reopening");
}

/// Onshape 1.223 (live rn-1222-1223 ins-other): Insert › Other documents › an opened document's Part Studios list has
/// the same Sort button beside its search box as Current document, A → Z by default, Z → A from its menu.
#[gpui::test]
fn an_other_documents_studios_sort_by_name_as_the_current_documents(cx: &mut TestAppContext) {
    let src = two_parts_document(cx, "ins-other-sort-src", |ctx| {
        let dup = call(ctx, "tab_duplicate", json!({ "tab": "Part Studio 1" }))["tab"]["name"].as_str().unwrap().to_string();
        call(ctx, "tab_rename", json!({ "tab": dup, "name": "Alpha studio" }));
        call(ctx, "history_create_version", json!({ "name": "V1" }));
    });
    let path = two_parts_document(cx, "ins-other-sort", |_| {});
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.insert_memory = Some(noshape_ops::prefs::InsertMemory::default());
            v.toggle_insert(w, cx);
            v.ui.insert_source = 1;
            v.pick_other_document(src.clone(), cx);
        })
    })
    .unwrap();
    draw(cx, window);
    let first = |cx: &mut TestAppContext| {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let y = |vcx: &mut gpui_kit::VisualTestContext, n: &str| vcx.debug_bounds(format!("ins-other-row {n}").leak()).unwrap_or_else(|| panic!("{n} listed")).origin.y;
        if y(&mut vcx, "Alpha studio") < y(&mut vcx, "Part Studio 1") { "Alpha studio" } else { "Part Studio 1" }
    };
    assert!(gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("ins-cur-sort").is_some(), "the Sort button beside the search box");
    assert_eq!(first(cx), "Alpha studio", "A → Z by default");
    // (Z → A as the menu's pick sets it, without writing the remembered file other tests read.)
    cx.update(|cx| view.update(cx, |v, cx| {
        v.ui.insert_memory.as_mut().unwrap().current_desc = true;
        cx.notify();
    }));
    draw(cx, window);
    assert_eq!(first(cx), "Part Studio 1", "Z → A");
}

/// Live2 asm-dialogs-1119 grp / gok: Group on a fixed part and a part Revolute-mated to it shows the mate's error as the
/// dialog opens (not only after ✓), and ✓ clears the selection.
#[gpui::test]
fn the_group_dialog_shows_its_errors_as_it_opens_and_ok_clears_the_selection(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "group-preview", |ctx| {
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>"], "fixed": true }));
        let top = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
        let (a, b) = (top(ctx, "Part 1 <1>"), top(ctx, "Part 2 <1>"));
        call(ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 2 <1>", "entity": b } }));
    });
    let (window, view) = open_assembly_with_front(cx, &path);
    let settle = |cx: &mut TestAppContext| {
        draw(cx, window);
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        draw(cx, window);
    };
    let mate_error = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).result.mates.first().and_then(|m| m.message.clone()));
    assert_eq!(mate_error(cx), None, "Revolute 1 solves");
    cx.update(|cx| view.update(cx, |v, cx| {
        let ids = vec![instance(v, "Part 1 <1>", cx), instance(v, "Part 2 <1>", cx)];
        v.selected = ids.clone();
        v.open_group(None, ids, vec![], cx);
    }));
    settle(cx);
    assert_eq!(mate_error(cx).as_deref(), Some("Mate cannot be added between member of same group."), "the error as the dialog opens");
    // Live2 recheck-1119 (4): the new group's own row red bold with its "!" while the dialog is open.
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds("group-new error").is_some(), "Group 1's row in error (grey: {})", vcx.debug_bounds("group-new").is_some());
    cx.update(|cx| view.update(cx, |v, cx| v.accept_group(cx)));
    settle(cx);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.selected.is_empty(), "✓ clears the selection");
        assert_eq!(v.assembly(cx).unwrap().groups.len(), 1, "Group 1 made");
    });
    assert_eq!(mate_error(cx).as_deref(), Some("Mate cannot be added between member of same group."), "the error stays");
}

/// Live2 asm-dialogs-1119 ins-flow: an instance placed with Insert's dialog still open lights nothing under the pointer
/// until the pointer moves.
#[gpui::test]
fn a_placed_instance_doesnt_light_until_the_pointer_moves(cx: &mut TestAppContext) {
    use noshape_scene::{asm_scene::AsmPickTarget, builder::PickKind};
    let (window, view) = pattern_assembly(cx, "placed-quiet");
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).scene().hover_outline);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.toggle_insert(w, cx);
            let studio = v.doc.read(cx).session.doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
            let id = v.insert_part(studio, None, cx).unwrap();
            v.start_placing(vec![id], cx);
            v.finish_placing(true, w, cx);
        })
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    let hover = |cx: &mut TestAppContext| {
        cx.update(|cx| view.update(cx, |v, cx| {
            let id = instance(v, "Part 1 <1>", cx);
            let top = face_connector(v, "Part 1 <1>", true, cx);
            let Reference::Implicit { on, .. } = &top.reference else { panic!("an implicit connector") };
            let occ = v.result.occurrences.iter().position(|o| o.top() == id).unwrap();
            v.hover = Some(AsmPickTarget { path: vec![id], reference: (**on).clone(), kind: PickKind::Face, occ, anchor: None });
            v.refresh_scene(false, cx);
        }));
    };
    assert!(cx.update(|cx| view.read(cx).insert_open && view.read(cx).ui.placed_at.is_some()), "placed, the dialog open");
    hover(cx);
    assert!(!lit(cx), "nothing lit where it was placed");
    cx.update(|cx| view.update(cx, |v, _| v.pointer += glam::DVec2::new(4.0, 0.0)));
    hover(cx);
    assert!(lit(cx), "the face lights once the pointer moved");
}

/// Live2 asm-dialogs-1119 chip: hovering a connector chip in the mate dialog lights its face yellow-green.
#[gpui::test]
fn a_hovered_connector_chip_lights_its_face(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "chip-hover");
    let query_lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).scene().states.iter().any(|s| s & noshape_render::state::QUERY != 0));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Fastened, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    assert!(!query_lit(cx), "nothing yellow-green at rest");
    cx.update(|cx| view.update(cx, |v, cx| {
        let d = v.mate.as_ref().unwrap().dialog.clone();
        d.update(cx, |d, _| d.hovered_field = Some(0));
        v.refresh_scene(false, cx);
    }));
    assert!(query_lit(cx), "the first chip's face lit");
}

/// Live2 audit-1119 (m2–m7): Mirror strategy on a subassembly: Derived reads "Create new instance", and Update makes
/// the assembly "Sub-Mirrored" (its "Derived mirror" node of Sub's instances) and the row's reference.
#[gpui::test]
fn mirror_strategy_update_on_a_subassembly_makes_its_mirrored_assembly(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-derived-mirror-{}", std::process::id()));
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
        call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
    }
    let (window, view) = open_assembly_with_front(cx, &path);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_pattern(super::panels::PatternDialogKind::Mirror, w, cx))).unwrap();

    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let sub = instance(v, "Sub <1>", cx);
            let p = v.pattern.as_mut().unwrap();
            p.seeds = vec![sub];
            p.strategy_open = true;
            p.mirror_rows.entry(sub).or_default().derived = true;
            cx.notify();
        })
    });
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let update = vcx.debug_bounds("ms-update").expect("Update");
    vcx.simulate_click(update.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        let doc = &v.doc.read(cx).session.doc;
        let sub = doc.tabs.iter().find(|t| t.name == "Sub").unwrap().id;
        let m = doc.mirrored_assembly(sub).expect("Sub's mirrored assembly");
        assert_eq!(doc.tab(m).unwrap().name, "Sub-Mirrored");
        assert!(v.pattern.as_ref().unwrap().mirror_rows.values().all(|r| r.created), "the row's reference is the new assembly");
    });
    // Live m7: the chevron lists Sub's own parts under its row; Part 1 Derived, Update, ✓: the node's strategy.
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let chev = vcx.debug_bounds("ms-chev-0").expect("the subassembly row's chevron");
    vcx.simulate_click(chev.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let sub = instance(v, "Sub <1>", cx);
            let part = match &v.doc.read(cx).session.doc.tabs.iter().find(|t| t.name == "Sub").unwrap().content {
                noshape_model::TabContent::Assembly(a) => a.instances[0].id,
                _ => unreachable!(),
            };
            let p = v.pattern.as_mut().unwrap();
            assert_eq!(p.mirror_expanded, [sub], "expanded");
            p.mirror_children.entry(sub).or_default().entry(part).or_default().derived = true;
            p.axis = Some(noshape_model::assembly::MateConnectorRef::origin());
            cx.notify();
        })
    });
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let update = vcx.debug_bounds("ms-update").expect("Update");
    vcx.simulate_click(update.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    cx.update(|cx| view.update(cx, |v, cx| v.accept_pattern(cx)));
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        let doc = &v.doc.read(cx).session.doc;
        let ps = doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
        let m = doc.mirrored_studio(ps).expect("Part Studio 1-Mirrored");
        let sub = doc.tabs.iter().find(|t| t.name == "Sub").unwrap().id;
        let noshape_model::TabContent::Assembly(a) = &doc.tab(doc.mirrored_assembly(sub).unwrap()).unwrap().content else { unreachable!() };
        let noshape_model::assembly::InstanceSource::DerivedMirror { derived, mirrored_studios, .. } = &a.instances[0].source else { panic!("the node") };
        assert_eq!((derived.len(), mirrored_studios.clone()), (1, vec![(ps, m)]));
    });
}

/// Live2 audit-1119 m10–m12: a "-Mirrored" assembly's Derived mirror node: its row menu as live's, and Edit… › Mirror
/// strategy… › Derived on its part › Update › ✓ makes "Part Studio 1-Mirrored" and writes the node's strategy.
#[gpui::test]
fn a_derived_mirror_nodes_menu_and_dialog(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-ui-derived-node-{}", std::process::id()));
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
        call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
        call(&mut ctx, "assembly_pattern", json!({ "tab": "Assembly 1", "kind": "mirror", "seeds": ["Sub <1>"], "direction": { "origin": true }, "strategy": [{ "instance": "Sub <1>", "action": "derived" }] }));
    }
    let (window, view) = open_tab_with_front(cx, &path, "Sub-Mirrored");
    let sep = "—";
    cx.update(|cx| {
        let v = view.read(cx);
        let node = instance(v, "Derived mirror", cx);
        assert_eq!(
            labels(&v.instance_menu(node, false, cx)),
            [
                "Rename", "Edit…", "Hide", "Hide other instances", "Hide all instances", "Show center of mass", "Isolate…", "Make transparent…", "Suppress", "Dynamic suppression ▸", sep,
                "Zoom to selection", sep,
                "Add selection to folder…", "Expand ▸", "Collapse ▸", sep,
                "Delete",
            ]
        );
    });
    cx.update(|cx| view.update(cx, |v, cx| {
        let node = instance(v, "Derived mirror", cx);
        v.open_derived_mirror(node, cx);
    }));
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds("dm-strategy").expect("Mirror strategy…");
    vcx.simulate_click(b.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    cx.update(|cx| view.update(cx, |v, cx| {
        let d = v.ui.derived_mirror.as_mut().unwrap();
        assert!(d.strategy_open);
        let part = d.rows.keys().copied().next().expect("the source's part listed");
        d.rows.get_mut(&part).unwrap().derived = true;
        cx.notify();
    }));
    draw(cx, window);
    for id in ["ms-update", "dm-ok"] {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let b = vcx.debug_bounds(id).unwrap_or_else(|| panic!("{id}"));
        vcx.simulate_click(b.center(), gpui_kit::Modifiers::default());
        draw(cx, window);
        // (The table lies over the dialog, as live's: closed before ✓.)
        cx.update(|cx| view.update(cx, |v, cx| {
            if let Some(d) = v.ui.derived_mirror.as_mut() {
                assert!(d.rows.values().all(|r| !r.derived || r.created), "Update made it");
                d.strategy_open = false;
            }
            cx.notify();
        }));
        draw(cx, window);
    }
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.derived_mirror.is_none(), "✓ closed it");
        let doc = &v.doc.read(cx).session.doc;
        let ps = doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
        let m = doc.mirrored_studio(ps).expect("Part Studio 1-Mirrored");
        let node = v.assembly(cx).unwrap().instances.iter().find(|i| i.name == "Derived mirror").unwrap().source.clone();
        let noshape_model::assembly::InstanceSource::DerivedMirror { derived, mirrored_studios, .. } = node else { panic!("the node") };
        assert_eq!((derived.len(), mirrored_studios), (1, vec![(ps, m)]));
    });
}

/// Live asm-views2-1118: a short drag on an explode triad's arrow still makes a step (4 / 8 / 10 px: 0 mm) and opens
/// its dialog; a longer one is the pointer's travel along the arrow at the screen scale, rounded to 0.1 mm.
#[gpui::test]
fn a_short_explode_arrow_drag_makes_a_0_mm_step(cx: &mut TestAppContext) {
    use crate::viewport::ViewportEvent;
    use gpui_kit::Modifiers;
    let (window, view) = pattern_assembly(cx, "explode-short");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.edit("Insert : Explode 1", cx, |a| a.exploded_views.push(noshape_model::assembly::ExplodedView { name: "Explode 1".into(), steps: vec![] }));
            v.activate_explode_from_menu(0, cx);
            assert!(v.explode_edit, "editing Explode 1");
            let pin = instance(v, "Part 2 <1>", cx);
            let steps = |v: &AssemblyView, cx: &gpui_kit::App| v.assembly(cx).unwrap().exploded_views[0].steps.clone();
            // Across the Z arrow (4 px at right angles to it on screen), then 20 px along it.
            for (k, along) in [(1, false), (2, true)] {
                v.selected = vec![pin];
                let f = v.manipulator(cx).expect("a triad");
                let vp = v.viewport.clone();
                let (shaft, tip) = (vp.read(cx).project(f.origin + f.z() * v.manip_size(cx) * 0.6).unwrap(), vp.read(cx).project(f.origin + f.z() * v.manip_size(cx)).unwrap());
                let up = (tip - shaft).normalize();
                let to = if along { shaft + up * 20.0 } else { shaft + up.perp() * 4.0 };
                v.manip_hover = Some(ManipHandle::Axis(2));
                v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: shaft, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                v.drag.as_mut().expect("an arrow drag").started = true;
                v.on_viewport_event(&vp, &ViewportEvent::ToolDrag { position: to }, w, cx);
                v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: to, modifiers: Modifiers::default() }, w, cx);
                let s = steps(v, cx);
                assert_eq!(s.len(), k, "drag {k}: a step made");
                assert!(v.step_edit.is_some(), "drag {k}: its dialog open");
                let d = glam::DVec3::from(s[k - 1].translation).dot(f.z()) * 1e3;
                assert!((d * 10.0 - (d * 10.0).round()).abs() < 1e-6, "drag {k}: {d} mm, in 0.1 mm steps");
                if along {
                    let px_per_mm = (vp.read(cx).project(f.origin + f.z() * 1e-3).unwrap() - vp.read(cx).project(f.origin).unwrap()).length();
                    assert!((d - 20.0 / px_per_mm).abs() <= 0.05 + 1e-9, "drag 2: the travel at the screen scale, {d} mm for 20 px at {px_per_mm} px/mm");
                } else {
                    assert_eq!(d, 0.0, "drag 1: a 0 mm step");
                }
                v.on_accept(&crate::actions::Accept, w, cx);
            }
        })
    })
    .unwrap();
}

/// Live asm-views2-1118 / 156–161: a named position's row menu is Apply / Edit / Duplicate / Update named position,
/// Move down / up (with two or more), Delete <name>, Apply left out on the applied row; Apply moves the parts in one
/// step, no animation, the row marked applied with the "<name> is applied." toast.
#[gpui::test]
fn a_named_positions_menu_and_apply_act_as_live(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "np-apply");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Revolute, w, cx);
            let (a, b) = (face_connector(v, "Part 2 <1>", false, cx), face_connector(v, "Part 1 <1>", true, cx));
            v.deliver_connector(a, w, cx);
            v.deliver_connector(b, w, cx);
            v.accept_mate(w, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let m = v.assembly(cx).unwrap().mates.iter().find(|m| m.name == "Revolute 1").expect("Revolute 1").id;
            v.open_np_driver(m, cx);
            v.toggle_np_driver(m, 0, cx);
            v.accept_np_driver(cx);
            // Named position 2: Revolute 1 turned 90° on from Named position 1.
            v.edit("Named position 2", cx, |a| {
                let mut np = a.named_positions[0].clone();
                np.name = "Named position 2".into();
                for (_, val) in np.mate_values.iter_mut().filter(|(x, _)| *x == m) {
                    val[0] += std::f64::consts::FRAC_PI_2;
                }
                a.named_positions.push(np);
            });
            v.ui.np_count = 2;
            v.apply_np(0, cx);
            let labels = |v: &AssemblyView, i: usize| {
                v.np_row_menu(i, format!("Named position {}", i + 1))
                    .iter()
                    .filter_map(|e| match e {
                        super::behaviors::MenuEntry::Item { label, .. } => Some(label.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(labels(v, 0), ["Edit named position", "Duplicate named position", "Update named position", "Move down", "Delete Named position 1"], "the applied row: no Apply");
            assert_eq!(labels(v, 1), ["Apply named position", "Edit named position", "Duplicate named position", "Update named position", "Move up", "Delete Named position 2"]);
            // (Both blocks are free: the turn is Part 2's relative to Part 1.)
            let relative = |v: &AssemblyView, cx: &gpui_kit::App| {
                let q = |name: &str| glam::DQuat::from_array(v.result.solved.placement(instance(v, name, cx)).unwrap().rotation);
                q("Part 2 <1>") * q("Part 1 <1>").inverse()
            };
            let before = relative(v, cx);
            v.apply_np(1, cx);
            let turned = relative(v, cx) * before.inverse();
            assert!((turned.angle_between(glam::DQuat::IDENTITY) - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "turned 90° at once: {turned:?}");
            assert!(v.viewport.read(cx).animation().is_none(), "no animation");
            assert_eq!(v.ui.np_applied, Some(1), "the row marked applied");
            assert_eq!(v.ui.np_toast.as_deref(), Some("Named position 2 is applied."));
        });
    });
    // A click on a row edits its cell and applies nothing.
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let place = |v: &AssemblyView, cx: &gpui_kit::App| v.result.solved.placement(instance(v, "Part 2 <1>", cx)).unwrap().rotation;
            let before = place(v, cx);
            v.edit_np_cell(0, 0, "Named position 1".into(), w, cx);
            assert!(v.ui.np_cell.as_ref().is_some_and(|c| (c.row, c.col) == (0, 0)), "the click edits the cell");
            v.commit_np_cell(cx);
            assert_eq!(place(v, cx), before, "nothing applied");
            assert_eq!(v.ui.np_applied, Some(1));
            assert_eq!(v.assembly(cx).unwrap().named_positions[0].name, "Named position 1");
        });
    })
    .unwrap();
}

/// Live asm-dstates-1118: Display states is a toolbar tool (its panel on the left), and choosing a state in its
/// select applies it at once: the instance rows and the graphics together, not animated.
#[gpui::test]
fn choosing_a_display_state_applies_it_at_once(cx: &mut TestAppContext) {
    let (window, view) = pattern_assembly(cx, "ds-apply");
    let settle = |cx: &mut TestAppContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
    };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            assert!(noshape_ops::prefs::ASM_TOOLSETS.iter().any(|set| set.contains(&"tb-display-states")), "a toolbar tool");
            v.run_toolbar_tool("tb-display-states", w, cx);
            assert!(v.ui.display_states, "the toolbar tool opens the panel");
            let pin = instance(v, "Part 2 <1>", cx);
            v.set_hidden(vec![pin], true, cx);
            v.edit("Add display state : Pin hidden", cx, |a| a.save_display_state("Pin hidden"));
            v.set_hidden(vec![pin], false, cx);
        })
    })
    .unwrap();
    settle(cx);
    let shown = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let pin = instance(v, "Part 2 <1>", cx);
            let row = v.assembly(cx).unwrap().instance(pin).unwrap().hidden;
            let drawn = v.result.occurrences.iter().filter(|o| o.top() == pin).all(|o| o.hidden);
            (row, drawn)
        })
    };
    assert_eq!(shown(cx), (false, false), "the pin shown");
    cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.choose_display_state(0, cx))).unwrap();
    settle(cx);
    assert_eq!(shown(cx), (true, true), "the row and the graphics both hidden");
    assert!(cx.update(|cx| view.read(cx).viewport.read(cx).animation().is_none()), "no animation");
}

/// qa P1 walk 11-19 (ASM-DRAG-UNFIXED-GROUP), live behaviours 42244 / 42565: with nothing fixed, a drag on a part
/// carries its whole mated group as one rigid set (Part 1 <1> Fastened to <2>, on a Revolute to Part 2; ours moved
/// nothing: the group's 6 DOF took the free-instance path, which held the mates' partners).
#[gpui::test]
fn a_drag_with_nothing_fixed_carries_the_mated_group(cx: &mut TestAppContext) {
    use crate::viewport::ViewportEvent;
    use gpui_kit::Modifiers;
    let path = two_parts_document(cx, "drag-unfixed-group", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        let face = |ctx: &mut Ctx, i: &str, z: i32| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, z] }))["faces"][0]["id"].clone();
        let (a, b) = (face(ctx, "Part 1 <1>", -1), face(ctx, "Part 1 <2>", 1));
        call(ctx, "assembly_mate", json!({ "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 1 <2>", "entity": b } }));
        let (a, b) = (face(ctx, "Part 1 <1>", 1), face(ctx, "Part 2 <1>", -1));
        call(ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 2 <1>", "entity": b } }));
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>", "Part 1 <2>", "Part 2 <1>"], "fixed": false }));
    });
    let (window, view) = open_assembly(cx, &path);
    draw(cx, window);
    let names = ["Part 1 <1>", "Part 1 <2>", "Part 2 <1>"];
    let poses = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        names.map(|n| v.result.solved.placement(instance(v, n, cx)).unwrap())
    });
    let before = poses(cx);
    let to = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let a = v.assembly(cx).unwrap();
                assert!(a.instances.iter().all(|i| !i.fixed), "nothing fixed");
                assert!(a.mates.len() == 2 && v.result.mates.iter().all(|m| m.ok), "both mates solved");
                let id = instance(v, "Part 1 <1>", cx);
                let (lo, hi) = v.result.instance_bounds(id).unwrap();
                let at = v.viewport.read(cx).project((lo + hi) * 0.5).unwrap();
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                std::thread::sleep(std::time::Duration::from_millis(60));
                let to = at + glam::DVec2::new(75.0, 0.0);
                v.on_viewport_event(&vp, &ViewportEvent::ToolDrag { position: to }, w, cx);
                to
            })
        })
        .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: to, modifiers: Modifiers::default() }, w, cx);
        })
    })
    .unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    let after = poses(cx);
    let moved = |i: usize| glam::DVec3::from(after[i].translation) - glam::DVec3::from(before[i].translation);
    assert!(moved(0).length() > 1e-3, "the grabbed part moved: {:?} m", moved(0));
    for i in 0..3 {
        assert!((moved(i) - moved(0)).length() < 1e-6, "{} moved with the group: {:?} vs {:?}", names[i], moved(i), moved(0));
        let turn = glam::DQuat::from_array(after[i].rotation).angle_between(glam::DQuat::from_array(before[i].rotation));
        assert!(turn < 1e-6, "{} not turned: {turn}", names[i]);
    }
}

/// Live feel cov-1120 A2: a new Fastened's previewed part shows its hidden edges through the other part (sunk into it,
/// a thin amber box), as an instance on the pointer from Insert does; nothing before the second pick.
#[gpui::test]
fn a_mate_previews_moving_part_shows_its_edges_through_the_others(cx: &mut TestAppContext) {
    let path = two_parts_document(cx, "preview-xray", |ctx| {
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>", "Part 2 <1>"], "fixed": false }));
    });
    let (window, view) = open_assembly(cx, &path);
    let xray = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).scene_options(cx).xray_tops);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_mate(MateType::Fastened, w, cx);
            let a = face_connector(v, "Part 1 <1>", true, cx);
            v.deliver_connector(a, w, cx);
        });
    })
    .unwrap();
    assert!(xray(cx).is_empty(), "one end picked: nothing moves yet");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let b = face_connector(v, "Part 2 <1>", false, cx);
            v.deliver_connector(b, w, cx);
        });
    })
    .unwrap();
    let moving = cx.update(|cx| instance(view.read(cx), "Part 1 <1>", cx));
    assert_eq!(xray(cx), std::collections::HashSet::from([moving]), "the first pick moves and shows its hidden edges");
    // (A2 FEEL-ASM-MATE-PREVIEW-COLOUR: the face that stays filled the picks' yellow, the moving part not amber.)
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| {
        let v = view.read(cx);
        let (scene, built) = (v.viewport.read(cx).scene(), v.built.as_ref().unwrap());
        let (picked, selected) = (noshape_render::state::SELECTED | noshape_render::state::PICKED, noshape_render::state::SELECTED);
        let faces = |flag: u32, top: InstanceId| (1..scene.states.len() as u32).filter(|id| scene.states[*id as usize] & flag == flag && built.picks.faces_of(top).contains(id)).count();
        let other = instance(v, "Part 2 <1>", cx);
        assert_eq!(faces(picked, other), 1, "the other part's picked face yellow");
        assert_eq!(faces(selected, moving), 0, "the moving part not filled");
        // (qa asm-mate-preview-0bc7: the picked face's boundary #FFC20E (PICKED | OUTLINE), none of the moving part's
        // edges lit; its edges peach in view and through the others.)
        let (lit, outline) = (noshape_render::state::HOVER | noshape_render::state::SELECTED, noshape_render::state::PICKED | noshape_render::state::OUTLINE);
        let edges = |top: InstanceId, flag: u32| built.picks.edges_of(top).into_iter().filter(|id| scene.states[*id as usize] & flag != 0).map(|id| (id, scene.states[id as usize])).collect::<Vec<_>>();
        assert_eq!(edges(moving, lit), vec![], "the moving part's edges unlit");
        let ring = edges(other, noshape_render::state::OUTLINE);
        assert!(!ring.is_empty() && ring.iter().all(|(_, s)| s & outline == outline), "the picked cap's circle outlined #FFC20E: {ring:?}");
        assert_eq!(v.scene_options(cx).preview_tops, std::collections::HashSet::from([moving]));
    });
}

/// Live new-mate-preview-1119 (asm REQUESTS 22:49, qa's ASM-NEW-MATE-PREVIEW-GROUP): in a new Revolute's preview the
/// first pick moves alone (unless it is fixed), splitting it from its Fastened twin; after ✓ the twin follows rigidly.
/// Part 2 picked first: Part 2 moves and the pair stays, before and after ✓.
#[gpui::test]
fn a_new_mates_preview_moves_the_first_pick_alone_and_its_group_after_the_check(cx: &mut TestAppContext) {
    let names = ["Part 1 <1>", "Part 1 <2>", "Part 2 <1>"];
    for part1_first in [true, false] {
        let path = two_parts_document(cx, if part1_first { "preview-group-a" } else { "preview-group-b" }, |ctx| {
            call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
            let face = |ctx: &mut Ctx, i: &str, z: i32| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, z] }))["faces"][0]["id"].clone();
            let (a, b) = (face(ctx, "Part 1 <1>", -1), face(ctx, "Part 1 <2>", 1));
            call(ctx, "assembly_mate", json!({ "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 1 <2>", "entity": b } }));
            call(ctx, "assembly_set_state", json!({ "instances": names, "fixed": false }));
        });
        let (window, view) = open_assembly(cx, &path);
        let poses = |cx: &mut TestAppContext| {
            cx.executor().advance_clock(std::time::Duration::from_millis(100));
            cx.run_until_parked();
            cx.update(|cx| {
                let v = view.read(cx);
                names.map(|n| v.result.solved.placement(instance(v, n, cx)).unwrap())
            })
        };
        let before = poses(cx);
        let moved = |p: &[noshape_model::assembly::Placement; 3], i: usize| {
            let dt = (glam::DVec3::from(p[i].translation) - glam::DVec3::from(before[i].translation)).length();
            dt > 1e-6 || glam::DQuat::from_array(p[i].rotation).angle_between(glam::DQuat::from_array(before[i].rotation)) > 1e-6
        };
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.start_mate(MateType::Revolute, w, cx);
                let (p1, p2) = (face_connector(v, "Part 1 <1>", true, cx), face_connector(v, "Part 2 <1>", false, cx));
                let (a, b) = if part1_first { (p1, p2) } else { (p2, p1) };
                v.deliver_connector(a, w, cx);
                v.deliver_connector(b, w, cx);
            });
        })
        .unwrap();
        let preview = poses(cx);
        let expect = if part1_first { [true, false, false] } else { [false, false, true] };
        assert_eq!([0, 1, 2].map(|i| moved(&preview, i)), expect, "preview, Part 1 first: {part1_first}");
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_mate(w, cx))).unwrap();
        let after = poses(cx);
        let expect = if part1_first { [true, true, false] } else { [false, false, true] };
        assert_eq!([0, 1, 2].map(|i| moved(&after, i)), expect, "after ✓, Part 1 first: {part1_first}");
        let rel = |p: &[noshape_model::assembly::Placement; 3]| {
            let (q0, q1) = (glam::DQuat::from_array(p[0].rotation), glam::DQuat::from_array(p[1].rotation));
            (q1.inverse() * q0, q1.inverse() * (glam::DVec3::from(p[0].translation) - glam::DVec3::from(p[1].translation)))
        };
        let ((qa, ta), (qb, tb)) = (rel(&before), rel(&after));
        assert!(qa.angle_between(qb) < 1e-6 && (ta - tb).length() < 1e-6, "the Fastened pair kept together after ✓");
    }
}
