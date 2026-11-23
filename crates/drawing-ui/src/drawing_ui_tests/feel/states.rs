//! Drawing looks in combined states (user P0 11-17, 11-21; lead audit 11-23): hovered × selected, hovered with another
//! entity selected, and what a drag shows while the button is held.

use super::*;
use crate::drawing_view::{DRAWING_HOVER, DRAWING_SELECTED};
use noshape_render::state;

/// The look `s` gets with `sel` selected and `hover` under the pointer: (a tan label box, an orange one, an on-top
/// tinted overlay, the OR of its picks' state flags).
fn look(cx: &mut TestAppContext, editor: &Entity<DrawingEditor>, s: Sel, sel: &[Sel], hover: Option<Sel>) -> (bool, bool, bool, u32) {
    cx.update(|cx| {
        editor.update(cx, |e, _| {
            e.selection = sel.to_vec();
            e.click_still = None;
            e.hover = hover.and_then(|h| e.picks_of(h).first().copied());
            let scene = e.build_scene();
            let boxed = |c: u32| scene.instances.iter().any(|i| i.layer == -4 && i.color == crate::theme::lin(c));
            let flags = e.picks_of(s).iter().fold(0, |f, p| f | scene.states.get(*p as usize).copied().unwrap_or(0));
            (boxed(DRAWING_HOVER), boxed(DRAWING_SELECTED), scene.instances.iter().any(|i| i.layer == 20), flags)
        })
    })
}

/// A dimension (from DIMENSIONED), a note (SETUP), a centerline in the first view and the sheet's left border line.
fn annotated(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<DrawingEditor>, [Sel; 4], noshape_model::drawing::ViewId) {
    use noshape_model::drawing::{Annotation, AnnotationKind};
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    SETUP.set(Some(|ctx, dw| {
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "BREAK ALL SHARP EDGES", "position": [30, 40] }));
    }));
    let (window, editor) = drawing(cx, name);
    DIMENSIONED.set(false);
    cx.update_window(window, |_, _, cx| editor.update(cx, |e, cx| e.edit("Formats", cx, |d| d.props.formats_locked = false))).unwrap();
    real_sheet(cx, window, &editor);
    let (view, centerline) = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let view = e.built.as_ref().and_then(|b| b.placements.first().copied()).expect("a view").id;
            let id = e.current_sheet().next_annotation_id();
            e.edit_sheet("Centerline", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Centerline { view, a: [-20.0, 0.0], b: [20.0, 0.0] }, ..Default::default() }));
            (view, id)
        })
    });
    real_sheet(cx, window, &editor);
    let (dim, note, border) = cx.update(|cx| {
        let e = editor.read(cx);
        let s = e.current_sheet();
        let dim = s.annotations.iter().find(|a| a.kind.is_dimension()).expect("a dimension").id;
        let note = s.annotations.iter().find(|a| matches!(a.kind, AnnotationKind::Note { .. })).expect("the note").id;
        let [fx0, fy0, _, fy1] = s.size.frame_rect();
        (dim, note, noshape_model::drawing::format_line_key([fx0, fy0], [fx0, fy1]))
    });
    (window, editor, [Sel::Annotation(dim), Sel::Annotation(note), Sel::Annotation(centerline), Sel::FormatLine(border)], view)
}

/// Live drw-hover-1122 (crop pixels: sh-* equal h-*), qa walk6-1121 5: a selected dimension, note, centerline or sheet
/// border under the pointer takes the hover look (tan); the pointer off it, the selected look (orange) again.
#[gpui::test]
fn selected_and_hovered_annotations_take_the_hover_look(cx: &mut TestAppContext) {
    let (_window, editor, [dim, note, centerline, border], _) = annotated(cx, "feel-sel-hover-looks");
    for (name, s) in [("dimension", dim), ("note", note)] {
        let (tan, orange, ..) = look(cx, &editor, s, &[s], None);
        assert_eq!((tan, orange), (false, true), "a selected {name}: an orange box");
        let (tan, orange, ..) = look(cx, &editor, s, &[s], Some(s));
        assert_eq!((tan, orange), (true, false), "a selected {name} hovered: the tan box");
        let (tan, orange, ..) = look(cx, &editor, s, &[], Some(s));
        assert_eq!((tan, orange), (true, false), "a hovered {name}: the tan box");
    }
    let (_, _, overlay, flags) = look(cx, &editor, centerline, &[centerline], None);
    assert!(!overlay && flags & state::SELECTED != 0, "a selected centerline: orange ({flags:#x})");
    let (_, _, overlay, _) = look(cx, &editor, centerline, &[centerline], Some(centerline));
    assert!(overlay, "a selected centerline hovered: the tan overlay on top");
    let (.., flags) = look(cx, &editor, border, &[border], None);
    assert_eq!(flags & (state::SELECTED | state::HOVER), state::SELECTED, "the selected border: orange");
    let (.., flags) = look(cx, &editor, border, &[border], Some(border));
    assert_ne!(flags & state::HOVER, 0, "the selected border hovered: tan (hover over the selection)");
}

/// Live drw-hover-1122 oh-*: with a view selected, hovering a dimension, a note or a centerline gives each its own
/// hover look and the view keeps its orange edges and frame.
#[gpui::test]
fn hovering_another_entity_keeps_a_selected_views_orange(cx: &mut TestAppContext) {
    let (_window, editor, [dim, note, centerline, _], view) = annotated(cx, "feel-other-hovered");
    for (name, s) in [("dimension", dim), ("note", note), ("centerline", centerline)] {
        let (tan, _, overlay, _) = look(cx, &editor, s, &[Sel::View(view)], Some(s));
        // (A note without a leader has no lines to tint: its look is the tan box.)
        assert!(if s == centerline { overlay } else { tan }, "{name}: its hover look");
        let (frames, edges, lit) = cx.update(|cx| {
            let e = editor.read(cx);
            let scene = e.build_scene();
            let picks = e.picks_of(Sel::View(view));
            let edges = picks.iter().filter(|p| scene.states.get(**p as usize).is_some_and(|f| f & state::SELECTED != 0)).count();
            (e.view_frames(), edges, picks.len())
        });
        assert_eq!(frames, vec![(view, DRAWING_SELECTED)], "{name} hovered: the view's orange frame");
        assert!(edges > 0 && edges + 2 >= lit, "{name} hovered: the view's edges orange ({edges} of {lit})");
    }
}

/// Live drawing-feel-1118 drag2, feel/cov-1120 (46474-1), drw-drag-feel-1122 "View drag colours": a view dragged from
/// hover (unselected) is drawn in the selected look while the button is held: its edges and frame orange.
#[gpui::test]
fn a_dragged_view_is_drawn_orange_while_the_button_is_held(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-drag-view-look");
    real_sheet(cx, window, &editor);
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().copied())).expect("a view");
    let inside = [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    assert_eq!(view_of(&lit_at(cx, window, &editor, inside)), Some(pl.id), "the view lights");
    let during = std::rc::Rc::new(std::cell::RefCell::new(None));
    let d = during.clone();
    let ed = editor.clone();
    drag_sheet(cx, window, &editor, inside, [inside[0] + 20.0, inside[1] + 15.0], move |cx| {
        *d.borrow_mut() = Some(cx.update(|cx| {
            let e = ed.read(cx);
            let scene = e.build_scene();
            let picks = e.picks_of(Sel::View(pl.id));
            let label = |p: &u32| matches!(e.built.as_ref().and_then(|b| b.picks.get(*p as usize)), Some(noshape_model::drawing::build::PickTarget::ViewLabel(_)));
            let edges: Vec<u32> = picks.into_iter().filter(|p| !label(p)).collect();
            let orange = edges.iter().filter(|p| scene.states.get(**p as usize).is_some_and(|f| f & state::SELECTED != 0)).count();
            (e.view_frames(), orange, edges.len())
        }));
    });
    let (frames, orange, all) = during.borrow_mut().take().expect("looked during the drag");
    assert_eq!(frames, vec![(pl.id, DRAWING_SELECTED)], "while dragged: the orange frame");
    assert!(all > 0 && orange == all, "while dragged: every edge orange ({orange} of {all})");
    assert!(cx.update(|cx| editor.read(cx).selection.is_empty()), "released: not selected");
}

/// Live feel/cov-1120 (46474-8, -9), drw-drag-feel-1122: a dimension being dragged is plain black (no box, no tint);
/// released with the pointer still on it, the tan hover look; the pointer away, plain again.
#[gpui::test]
fn a_dragged_dimension_is_plain_and_hovered_again_on_release(cx: &mut TestAppContext) {
    let (window, editor, [dim, ..], _) = annotated(cx, "feel-drag-dim-look");
    let Sel::Annotation(id) = dim else { unreachable!() };
    let text = cx.update(|cx| {
        let e = editor.read(cx);
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(dim));
        b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text").pos
    });
    let to = [text[0] + 12.0, text[1] + 6.0];
    let during = std::rc::Rc::new(std::cell::RefCell::new(None));
    let d = during.clone();
    let ed = editor.clone();
    drag_sheet(cx, window, &editor, text, to, move |cx| {
        *d.borrow_mut() = Some(cx.update(|cx| {
            let e = ed.read(cx);
            let scene = e.build_scene();
            let boxed = scene.instances.iter().any(|i| i.layer == -4 && (i.color == crate::theme::lin(DRAWING_HOVER) || i.color == crate::theme::lin(DRAWING_SELECTED)));
            let flags = e.picks_of(dim).iter().fold(0, |f, p| f | scene.states.get(*p as usize).copied().unwrap_or(0));
            (e.dragging_annotation_id(), boxed, flags)
        }));
    });
    let (dragging, boxed, flags) = during.borrow_mut().take().expect("looked during the drag");
    assert_eq!(dragging, Some(id), "the dimension is being dragged");
    assert!(!boxed && flags & (state::SELECTED | state::HOVER) == 0, "while dragged: plain black (box {boxed}, flags {flags:#x})");
    // Released where the text now is: hovered again.
    let now = cx.update(|cx| {
        let e = editor.read(cx);
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(dim));
        b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text").pos
    });
    assert_eq!(lit_at(cx, window, &editor, now), Some(dim), "the pointer on the released text lights it");
    let tan = cx.update(|cx| editor.read(cx).build_scene().instances.iter().any(|i| i.layer == -4 && i.color == crate::theme::lin(DRAWING_HOVER)));
    assert!(tan, "released, the pointer on it: the tan hover look");
    assert!(cx.update(|cx| editor.read(cx).selection.is_empty()), "released: not selected");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let boxed = cx.update(|cx| editor.read(cx).build_scene().instances.iter().any(|i| i.layer == -4));
    assert!(!boxed, "the pointer away: plain");
}

/// Live dx q2489 (drw-note-move-to-title-block step 8): a note's Move to ▸ Title block clicked with the menu row over a
/// view: the menu closes, the note stays selected and the view under the pointer shows its hover look at once.
#[gpui::test]
fn a_menu_row_click_lights_what_the_pointer_is_left_on(cx: &mut TestAppContext) {
    use gpui_kit::{MouseDownEvent, MouseUpEvent};
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let num = |v: &Value| v.as_f64().unwrap_or(0.0);
        // Just above the first view's top left corner.
        let (mut x0, mut y1) = (f64::MAX, f64::MIN);
        for e in g["views"][0]["edges"].as_array().cloned().unwrap_or_default() {
            for k in ["a", "b"] {
                x0 = x0.min(num(&e[k][0]));
                y1 = y1.max(num(&e[k][1]));
            }
        }
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "TEMP", "position": [x0, y1 + 12.0] }));
    }));
    let (window, editor) = drawing(cx, "feel-menu-row-hover");
    real_sheet(cx, window, &editor);
    let (note, text) = cx.update(|cx| {
        let e = editor.read(cx);
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).expect("the note");
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(Sel::Annotation(a.id)));
        (a.id, b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text").pos)
    });
    let p = at_window(cx, &editor, text);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, Modifiers::default());
    v.simulate_event(MouseDownEvent { button: MouseButton::Right, position: p, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
    v.simulate_event(MouseUpEvent { button: MouseButton::Right, position: p, modifiers: Modifiers::default(), click_count: 1 });
    draw(cx, window);
    assert_eq!(cx.update(|cx| editor.read(cx).selection.clone()), vec![Sel::Annotation(note)], "the right-click selects the note");
    let mut v = VisualTestContext::from_window(window, cx);
    let move_to = v.debug_bounds("menu:Move to").expect("Move to ▸").center();
    v.simulate_mouse_move(move_to, None, Modifiers::default());
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    draw(cx, window);
    let mut v = VisualTestContext::from_window(window, cx);
    let row = v.debug_bounds("menu:Title block").expect("Move to ▸ Title block").center();
    // A view under the row (wherever the menu opened).
    let under = cx.update(|cx| editor.read(cx).to_sheet(editor.read(cx).local(row)));
    cx.update(|cx| editor.update(cx, |e, cx| {
        let view = e.current_sheet().views[0].id;
        e.edit_sheet("Move view", cx, |s| {
            if let Some(v) = s.views.iter_mut().find(|v| v.id == view) {
                v.position = under;
            }
        });
    }));
    draw(cx, window);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(row, None, Modifiers::default());
    v.simulate_click(row, Modifiers::default());
    draw(cx, window);
    let after = cx.update(|cx| {
        let e = editor.read(cx);
        (e.selection.clone(), e.hover.and_then(|h| e.sel_of_pick(h)))
    });
    assert_eq!(after.0, vec![Sel::Annotation(note)], "the note stays selected");
    // What a pointer move there would light.
    let expected = lit_at(cx, window, &editor, under);
    assert!(view_of(&expected).is_some(), "the row sits over a view: {expected:?}");
    assert_eq!(after.1, expected, "lit at once, before any pointer move");
}

/// Live drawing/regrip rg-2..rg-4 (behaviours D26): a selected radial's blue grip dragged onto another hole's circle:
/// that circle lights orange while the button is held; released, the radial re-attaches to it.
#[gpui::test]
fn a_radials_grip_dragged_onto_another_circle_lights_it_and_reattaches(cx: &mut TestAppContext) {
    use noshape_model::drawing::views::EdgeGeom;
    use noshape_model::drawing::{AnnotationKind, build::PickTarget, sheet::GripKind};
    START.set(Some("holes-plate-drawing"));
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let num = |v: &Value| v.as_f64().unwrap_or(0.0);
        let circle = g["views"].as_array().into_iter().flatten().flat_map(|v| v["edges"].as_array().cloned().unwrap_or_default()).find(|e| e["type"] == "circle").expect("a hole's circle");
        let (cx, cy, r) = (num(&circle["center"][0]), num(&circle["center"][1]), num(&circle["radius"]));
        call(ctx, "drawing_dimension", json!({ "drawing": dw, "kind": "radius", "picks": [[cx + r, cy]], "place": [cx + r + 15.0, cy + 15.0] }));
    }));
    let (window, editor) = drawing(cx, "feel-radial-regrip");
    real_sheet(cx, window, &editor);
    let (id, view, grip, from) = cx.update(|cx| {
        editor.update(cx, |e, _| {
            let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, AnnotationKind::RadialDimension { .. })).cloned().expect("the radial");
            let AnnotationKind::RadialDimension { view, center, .. } = a.kind else { unreachable!() };
            e.selection = vec![Sel::Annotation(a.id)];
            let b = e.built.as_ref().expect("built");
            let grip = b.geometry.grips.iter().find(|g| g.kind == GripKind::Origin && b.picks.get(g.pick as usize) == Some(&PickTarget::Annotation(a.id))).map(|g| g.pos).expect("its blue grip");
            (a.id, view, grip, center)
        })
    });
    draw(cx, window);
    // Another hole's circle in the same view: a point on it (sheet) and its centre (view).
    let (on, to, edge) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.placement(view).expect("its view");
        let g = e.geoms.get(&view).expect("its geometry");
        g.model_edges.iter().enumerate().find_map(|(i, m)| match m.geom {
            EdgeGeom::Circle { center, radius, face_on: true, .. } if (center[0] - from[0]).hypot(center[1] - from[1]) > 1e-6 => {
                let k = std::f64::consts::FRAC_1_SQRT_2 * radius;
                Some((pl.to_sheet([center[0] + k, center[1] - k]), center, i))
            }
            _ => None,
        })
    }).expect("another hole's circle");
    let during = std::rc::Rc::new(std::cell::RefCell::new(None));
    let d = during.clone();
    let ed = editor.clone();
    drag_sheet(cx, window, &editor, grip, on, move |cx| {
        *d.borrow_mut() = Some(cx.update(|cx| {
            let e = ed.read(cx);
            let scene = e.build_scene();
            let b = e.built.as_ref().expect("built");
            let target: Vec<usize> = b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Edge { view: v, edge: k } if *v == view && e.geoms.get(&view).and_then(|g| g.edges.get(*k as usize)).is_some_and(|x| x.model == Some(edge as u32)))).map(|(i, _)| i).collect();
            let lit = target.iter().any(|p| scene.states.get(*p).is_some_and(|s| s & state::SELECTED != 0));
            (matches!(e.drag, noshape_scene::drawing_tools::Drag::Reattach { .. }), lit, target.len())
        }));
    });
    let (reattaching, lit, n) = during.borrow_mut().take().expect("looked during the drag");
    assert!(reattaching, "the blue grip's drag re-attaches");
    assert!(n > 0 && lit, "the target circle orange while the button is held ({n} picks)");
    let now = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.id == id).map(|a| a.kind.clone())).expect("the radial");
    let AnnotationKind::RadialDimension { center, .. } = now else { panic!("{now:?}") };
    assert!((center[0] - to[0]).hypot(center[1] - to[1]) < 1e-6, "re-attached to the other circle: {center:?} vs {to:?}");
}

/// Live drawing 1392 (behaviours l.1504-1507): a diameter dimension hovered or selected lights its whole referenced
/// circle light blue, the same either way; with the Dimension tool on, a placed one lights nothing.
#[gpui::test]
fn a_diameters_circle_lights_blue_hovered_or_selected_alike(cx: &mut TestAppContext) {
    use noshape_model::drawing::{AnnotationKind, build::PickTarget};
    START.set(Some("holes-plate-drawing"));
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let num = |v: &Value| v.as_f64().unwrap_or(0.0);
        let circle = g["views"].as_array().into_iter().flatten().flat_map(|v| v["edges"].as_array().cloned().unwrap_or_default()).find(|e| e["type"] == "circle").expect("a hole's circle");
        let (cx, cy, r) = (num(&circle["center"][0]), num(&circle["center"][1]), num(&circle["radius"]));
        call(ctx, "drawing_dimension", json!({ "drawing": dw, "kind": "diameter", "picks": [[cx + r, cy]], "place": [cx + r + 15.0, cy + 15.0] }));
    }));
    let (window, editor) = drawing(cx, "feel-diameter-blue");
    real_sheet(cx, window, &editor);
    let (id, circle) = cx.update(|cx| {
        let e = editor.read(cx);
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, AnnotationKind::RadialDimension { diameter: true, .. })).cloned().expect("the diameter");
        let AnnotationKind::RadialDimension { view, center, .. } = a.kind else { unreachable!() };
        (a.id, e.pk().centermark_circle_picks(view, noshape_model::drawing::build::centermark_key(center)))
    });
    assert!(!circle.is_empty(), "its circle drawn");
    let lit = |cx: &mut TestAppContext, sel: Vec<Sel>, hovered: bool| {
        cx.update(|cx| {
            editor.update(cx, |e, _| {
                e.selection = sel;
                e.hover = hovered.then(|| e.picks_of(Sel::Annotation(id)).first().copied()).flatten();
                let anns = e.related_annotations();
                let edges: Vec<u32> = anns.iter().flat_map(|a| e.reference_edge_picks(*a)).collect();
                (anns.contains(&id), edges)
            })
        })
    };
    let (on, hovered) = lit(cx, vec![], true);
    assert!(on, "hovered: its references light");
    assert!(circle.iter().all(|p| hovered.contains(p)), "hovered: the whole circle blue ({circle:?} in {hovered:?})");
    let (on, selected) = lit(cx, vec![Sel::Annotation(id)], false);
    assert!(on, "selected: its references light");
    assert_eq!(selected, hovered, "selected: the same blue as hovered");
    let b = cx.update(|cx| editor.read(cx).built.clone().expect("built"));
    assert!(hovered.iter().all(|p| matches!(b.picks.get(*p as usize), Some(PickTarget::Edge { .. }))), "only model edges light");
    cx.update(|cx| editor.update(cx, |e, _| e.tool = Tool::Dimension { kind: noshape_scene::drawing_tools::DimTool::Linear, picks: vec![] }));
    let (on, _) = lit(cx, vec![Sel::Annotation(id)], false);
    assert!(!on, "the Dimension tool on: a selected one lights nothing");
}

/// Live drawing u-9xx (asb 10-20, behaviours 2817): a hovered view label shows two orange grips at its ends and no
/// box, its view not framed; a click selects the label alone (an orange box, white grips), the view not framed.
#[gpui::test]
fn a_view_label_hovers_with_two_grips_and_a_click_takes_it_alone(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let front = g["views"].as_array().and_then(|v| v.iter().find(|v| v["orientation"] == "front").cloned()).expect("Front");
        call(ctx, "drawing_detail_view", json!({ "drawing": dw, "parent": front["id"], "center": [0, 0], "radius": 8, "position": [300, 200] }));
    }));
    let (window, editor) = drawing(cx, "feel-view-label-look");
    real_sheet(cx, window, &editor);
    let (detail, at) = cx.update(|cx| {
        let e = editor.read(cx);
        let detail = e.current_sheet().views.iter().find(|v| matches!(v.kind, noshape_model::drawing::ViewKind::Detail { .. })).map(|v| v.id).expect("the detail");
        let b = e.label_box(detail).expect("its label");
        (detail, [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])
    });
    let looks = |cx: &mut TestAppContext| cx.update(|cx| {
        let e = editor.read(cx);
        let scene = e.build_scene();
        let boxed = scene.instances.iter().any(|i| i.layer == -4 && i.color == crate::theme::lin(DRAWING_SELECTED));
        let grips = scene.instances.iter().any(|i| i.layer == 22);
        (boxed, grips, e.view_frames().iter().any(|f| f.0 == detail))
    });
    assert_eq!(lit_at(cx, window, &editor, at), Some(Sel::ViewLabel(detail)), "the label lights");
    assert_eq!(looks(cx), (false, true, false), "hovered: its grips, no box, the view not framed");
    assert_eq!(click_at(cx, window, &editor, at, Modifiers::default()), vec![Sel::ViewLabel(detail)], "a click takes the label alone");
    // Off everything (the detail sits near the sheet's corner).
    let size = cx.update(|cx| editor.read(cx).current_sheet().size.size_mm());
    let away = [[size.0 - 5.0, size.1 - 5.0], [size.0 / 2.0, size.1 - 5.0], [5.0, 5.0]].into_iter().find(|p| lit_at(cx, window, &editor, *p).is_none()).expect("a point off everything");
    lit_at(cx, window, &editor, away);
    assert_eq!(looks(cx), (true, true, false), "selected: the orange box and its grips, the view not framed");
}
