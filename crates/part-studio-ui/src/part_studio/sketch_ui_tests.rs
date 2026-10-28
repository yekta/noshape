//! UI keystroke tests of sketch mode on GPUI's test platform (docs/TESTS.md
//! "UI keystroke tests"): the real Part Studio view in a test window, the GPU
//! a no-op stand-in, clicks as the viewport's tool events, keys through GPUI.

use super::super::PartStudioView;
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::sketch_mode::Tool;
use crate::viewport::ViewportEvent;
use glam::DVec2;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Keystroke, Modifiers, InteractiveElement as _, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use noshape_model::reference::{DatumPlane, Reference};

/// The app's workspace as far as keys go: the "Workspace" key context the
/// default bindings (Esc → Cancel, …) need above the view.
struct Workspace(Entity<PartStudioView>);

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().key_context("Workspace").size_full().child(self.0.clone())
    }
}

/// A fresh document's Part Studio in a test window, drawn once.
fn part_studio(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    cx.update(gpui_kit::init);
    // The app's default key bindings (Esc → Cancel, tool keys), without the
    // user's customizations.
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    // The value box's own keys (the app binds them once per process).
    cx.update(|cx| cx.bind_keys(super::entry_key_bindings()));
    crate::part_studio_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let session = crate::session::Session::create(&path, crate::user_units::new_document()).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    (window, view)
}

/// A fresh document's Part Studio in a test window, sketching on Top with `tool`.
fn sketch_with(cx: &mut TestAppContext, name: &str, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio(cx, name);
    sketch_in(cx, window, view, tool)
}

/// A new sketch on Top in the test window's Part Studio, with `tool`.
fn sketch_in(cx: &mut TestAppContext, window: AnyWindowHandle, view: Entity<PartStudioView>, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>) {
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Plane(DatumPlane::Top)];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    // The window active, as the user's is (focus events need it); the
    // sketch editor comes up with the next frame.
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| v.activate_tool(tool, w, cx));
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    (window, view)
}

/// A frame (layout, paint: a focused input registers its text handler).
fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// A click of the tool at viewport pixel `at` (what the viewport sends for a
/// press and release without a drag).
fn click(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, at: (f64, f64)) {
    let at = DVec2::new(at.0, at.1);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            for ev in [
                ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() },
                ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 },
                ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() },
            ] {
                v.on_viewport_event(&vp, &ev, w, cx);
            }
        });
    })
    .unwrap();
    draw(cx, window);
}

/// Keys as the user types them (a character key carries its text), a frame
/// after each.
fn keys(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &[&str]) {
    for k in keys {
        let mut ks = Keystroke::parse(k).unwrap();
        if k.chars().count() == 1 {
            ks.key_char = Some(k.to_string());
        } else if *k == "space" {
            ks.key_char = Some(" ".into());
        }
        cx.update_window(window, |_, w, cx| {
            w.dispatch_keystroke(ks, cx);
        })
        .unwrap();
        draw(cx, window);
    }
}

/// The sketch's dimension values (metres), sorted.
fn dimensions(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<f64> {
    cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).expect("sketching");
        let mut v: Vec<f64> = data.sketch.constraints().filter(|c| c.driving).filter_map(|c| c.value()).collect();
        v.sort_by(f64::total_cmp);
        v
    })
}

fn near(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

/// OTF-FIRST-KEY: Line, click, click, `5` `0` Enter: the value box takes the
/// typed digits from the first key, and Enter makes a 50 mm dimension (the
/// first digit used to come up selected, so `0` replaced it).
#[gpui::test]
fn line_value_box_takes_the_first_key(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.05]), "{:?}", dimensions(cx, &view));
}

/// `=#len` in a Line's value box (a Variable `len` = 42 mm before the
/// sketch): Enter makes a 42 mm dimension driven by the variable.
#[gpui::test]
fn line_value_box_takes_a_variable(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "line-var");
    cx.update(|cx| {
        let (doc, tab) = { let v = view.read(cx); (v.doc.clone(), v.tab) };
        doc.update(cx, |d, cx| {
            let units = d.session.doc.settings.units;
            d.edit_studio(tab, "Variable", cx, |ps| {
                let mut f = ps.new_feature("assignVariable", &units).unwrap();
                f.params.set("name", noshape_model::ParamValue::Text("len".into()));
                f.params.set("variableType", noshape_model::ParamValue::Enum("LENGTH".into()));
                f.params.set("lengthValue", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("42 mm")));
                ps.features.push(f);
            });
        });
    });
    draw(cx, window);
    let (window, view) = sketch_in(cx, window, view, Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["=", "#", "l", "e", "n", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.042]), "{:?}", dimensions(cx, &view));
}

/// OTF-FIRST-KEY: corner rectangle, `30` Enter `20` Enter: 30 × 20 (the
/// width's first key used to be lost, and `20` went to the width).
#[gpui::test]
fn rectangle_value_boxes_take_width_then_height(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "rect", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["3", "0", "enter", "2", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.02, 0.03]), "{:?}", dimensions(cx, &view));
}

/// Tab moves from the rectangle's width box to its height box, taking the
/// typed width: Rectangle, click, click, 30 Tab 20 Enter → 30 × 20.
#[gpui::test]
fn rectangle_value_boxes_tab_between_them(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "rect-tab", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["3", "0", "tab", "2", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.02, 0.03]), "{:?}", dimensions(cx, &view));
}

/// A press, a drag in `steps` and a release of the tool, viewport pixels.
fn drag(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, from: (f64, f64), to: (f64, f64), steps: usize) {
    let p = |t: f64| DVec2::new(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    let mut evs = vec![ViewportEvent::PointerMove { position: p(0.0), modifiers: Modifiers::default() }, ViewportEvent::ToolDown { position: p(0.0), modifiers: Modifiers::default(), click_count: 1 }];
    evs.extend((1..=steps).map(|k| ViewportEvent::ToolDrag { position: p(k as f64 / steps as f64) }));
    evs.push(ViewportEvent::ToolUp { position: p(1.0), modifiers: Modifiers::default() });
    for ev in evs {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ev, w, cx);
            });
        })
        .unwrap();
        draw(cx, window);
    }
}

/// Conflicting dimensions in one part of a sketch don't stop a drag in
/// another: a point placed beside a line with two different lengths drags
/// with the pointer.
#[gpui::test]
fn a_point_drags_beside_conflicting_dimensions(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "conflict-drag", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Point, w, cx))).unwrap();
    draw(cx, window);
    click(cx, window, &view, (800., 300.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.with_sketch(cx, |_, data| {
                let line = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
                data.sketch.add_constraint(noshape_sketch::ConstraintKind::Length { curve: line, value: 0.01 }).unwrap();
                data.sketch.add_constraint_ext(noshape_sketch::ConstraintKind::Length { curve: line, value: 0.02 }, true).unwrap();
                data.sketch.solve();
            });
            v.activate_tool(Tool::Select, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let point = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let data = view.read(cx).sketch_data(cx).unwrap();
            let p = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).max().unwrap();
            data.sketch.point(p).unwrap()
        })
    };
    let before = point(cx);
    drag(cx, window, &view, (800., 300.), (860., 330.), 6);
    let moved = point(cx) - before;
    assert!(moved.length() > 1e-4, "the point stayed: {moved:?}");
}

/// DIM-AC-ENTER / EXPR-PARSE-JUNK: in a dimension's value box, `1 in` opens
/// the unit list, and one Enter commits 1 in (25.4 mm) and closes the box, as
/// live; the `26` typed next goes nowhere near it (two Enters were needed, and
/// the `26` landed in the box: 126 in).
#[gpui::test]
fn dimension_box_enter_commits_with_the_unit_list_open(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dim-ac", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter", "escape", "escape"]);
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["1", "space", "i", "n"]);
    assert!(cx.update(|cx| view.read(cx).ui_ref().is_some_and(|u| u.dim_completion.is_some())), "the unit list is open");
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "one Enter closes the box");
    keys(cx, window, &["2", "6"]);
    assert!(near(&dimensions(cx, &view), &[0.0254]), "{:?}", dimensions(cx, &view));
}

/// A line drawn and its value box dismissed (Esc), then the Dimension tool.
fn line_then_dimension_tool(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = sketch_with(cx, name, Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape", "escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    draw(cx, window);
    (window, view)
}

/// Parity sketch.md 7.3: the Dimension tool's pick and placement clicks open
/// the value box, which takes the typed value from the first key; Enter
/// applies it.
#[gpui::test]
fn dimension_tool_click_click_type_enter(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "dim-tool");
    assert!(dimensions(cx, &view).is_empty());
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 330.));
    assert!(cx.update(|cx| view.read(cx).dim_input.is_some()), "placement opens the value box");
    keys(cx, window, &["2", "5", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.025]), "{:?}", dimensions(cx, &view));
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()));
}

/// Parity sketch.md 7.16a: Esc on an untouched new box closes it at once
/// and the dimension keeps its measured value; reopened and changed, the
/// first Esc reverts the text and keeps the box, the second closes it.
#[gpui::test]
fn dimension_box_escape_as_live(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "dim-esc");
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 330.));
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "an untouched box closes at once");
    let measured = dimensions(cx, &view);
    assert_eq!(measured.len(), 1, "the dimension stays");
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["9"]);
    keys(cx, window, &["escape"]);
    let text = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|d| d.1.read(cx).value().to_string()));
    assert!(text.as_deref().is_some_and(|t| !t.starts_with('9')), "the first Esc reverts and keeps the box: {text:?}");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "the second Esc closes it");
    assert!(near(&dimensions(cx, &view), &measured), "{:?}", dimensions(cx, &view));
}

/// The number of sides typed before the polygon's rim click (live: digits set
/// the side count while the polygon tool waits for its circumference); the
/// click after the rim's commits it.
#[gpui::test]
fn polygon_sides_typed_before_the_rim_click(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "polygon", Tool::Polygon);
    click(cx, window, &view, (600., 400.));
    keys(cx, window, &["8"]);
    click(cx, window, &view, (680., 400.));
    click(cx, window, &view, (680., 400.));
    let lines = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.construction).count());
    assert_eq!(lines, 8);
}

/// Enter in a Line chain (live): the last segment's boxed length becomes a
/// driving dimension at its measured value.
#[gpui::test]
fn enter_in_a_line_chain_dimensions_the_last_segment(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line-chain", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    click(cx, window, &view, (700., 450.));
    let last = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).map(|e| e.id).max().and_then(|l| d.sketch.curve(l)).map(|c| c.length()).unwrap()
    });
    keys(cx, window, &["enter"]);
    assert!(near(&dimensions(cx, &view), &[last]), "{:?} vs {last}", dimensions(cx, &view));
}

/// BOX-DIALOG-PANIC: a box selection with a feature dialog open went into the
/// dialog's field while reading the dialog inside its own update (a panic).
#[gpui::test]
fn box_select_with_a_dialog_open(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "box-dialog");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, window);
    // Every pick id the drawn scene can have (the default planes, the origin).
    let ids: Vec<u32> = (1..64).collect();
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            assert!(v.edit.is_some(), "the Fillet dialog is open");
            v.box_select(&ids, false, cx);
            v.box_select(&ids, true, cx);
        })
    })
    .unwrap();
    draw(cx, window);
}

/// SECTION-DIALOG-STACK: starting a feature with the Section view dialog open
/// closes that dialog as with ✓ and keeps the section (live 10-27).
#[gpui::test]
fn a_feature_started_over_the_section_dialog_accepts_it(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "section-stack");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.section_through(Reference::Plane(DatumPlane::Front), w, cx);
            assert!(v.ui.section_dialog.is_some());
            v.start_feature("extrude", w, cx);
            assert!(v.ui.section_dialog.is_none(), "the Section view dialog closed");
            assert!(!v.ui.section.is_empty(), "the section stays on");
            assert!(v.edit.is_some(), "the Extrude dialog is open");
        })
    })
    .unwrap();
}

/// Interaction 6.2 (live qa4-r5-s9-5b / 5f): each rotate handle of the Section view manipulator lights alone along
/// its axis line, and the glyph's exact centre, where the two cross, is on no handle.
#[gpui::test]
fn section_rotate_handles_and_their_dead_centre(cx: &mut TestAppContext) {
    use super::super::section_manip::SectionHandle;
    // A rectangle sketch gives the model its bounds (the manipulator stands on them).
    let (window, view) = sketch_with(cx, "section-glyph", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.finish_sketch(w, cx);
            v.section_through(Reference::Plane(DatumPlane::Top), w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    view.update(cx, |v, cx| {
        let s = v.section_manip().expect("the section manipulator");
        let vp = v.viewport.read(cx);
        let tip = s.arrow.base + s.arrow.dir * s.arrow.length;
        let (p0, p1) = (vp.project(tip).unwrap(), vp.project(tip + vp.camera.right() * 1e-3).unwrap());
        let wpp = 1e-3 / (p1 - p0).length();
        let centre = s.ring_center(wpp);
        let at = |p: glam::DVec3| vp.project(p).unwrap();
        assert_eq!(v.section_handle_at(&s, at(centre), cx), None, "the centre is on no handle");
        for k in 0..2 {
            assert_eq!(v.section_handle_at(&s, at(centre + s.axes[k] * 16.0 * wpp), cx), Some(SectionHandle::Ring(k)), "along axis {k}");
        }
    });
}
