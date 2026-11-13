//! The editing matrix (docs/parity/interaction-feel.md "Editing"): live2's feel/editing.json rows (undo granularity,
//! copy / paste, Delete, Y / Shift+Y) acted out with keys on a sketch on Top or the block start. A row ours doesn't
//! match yet is a gap; the test fails when a row's status moves either way.

use super::*;

fn count(cx: &mut TestAppContext, view: &Entity<PartStudioView>, kind: noshape_sketch::EntityKind) -> usize {
    cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == kind && !e.construction).count()))
}

fn lines(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> usize {
    count(cx, view, noshape_sketch::EntityKind::Line)
}

fn feature_names(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<String> {
    cx.update(|cx| view.read(cx).studio(cx).map(|s| s.features.iter().map(|f| f.name.clone()).collect()).unwrap_or_default())
}

/// Feature `name` selected in the list.
fn select_feature(cx: &mut TestAppContext, view: &Entity<PartStudioView>, name: &str) {
    cx.update(|cx| {
        let ids: Vec<noshape_model::reference::FeatureId> = view.read(cx).studio(cx).map(|s| s.features.iter().filter(|f| f.name == name).map(|f| f.id).collect()).unwrap_or_default();
        view.update(cx, |v, _| v.selected_features = ids);
    });
}

fn block(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, name, Some(starts["block"].clone()));
    cx.update_window(window, |_, w, cx| {
        w.activate_window();
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    (window, view)
}

fn expect<T: PartialEq + std::fmt::Debug>(got: T, want: T, what: &str) -> Result<(), String> {
    if got == want { Ok(()) } else { Err(format!("{what}: ours {got:?}, live {want:?}")) }
}

type Check = fn(&mut TestAppContext) -> Result<(), String>;

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "undo / a chain: one step per segment", live: "a 3-segment chain needs 3 undos", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "ed-chain", Tool::Line);
            for at in [(400., 300.), (500., 300.), (500., 400.), (400., 400.)] {
                click(cx, window, &view, at);
            }
            keys(cx, window, &["escape"]);
            let drawn = lines(cx, &view);
            keys(cx, window, &["secondary-z"]);
            let one = lines(cx, &view);
            keys(cx, window, &["secondary-z", "secondary-z"]);
            expect((drawn, one, lines(cx, &view)), (3, 2, 0), "lines drawn, after one undo, after three")
        } },
        Row { id: "undo / a rectangle: one step", live: "its 4 lines and constraints in one step", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "ed-rect", Tool::CornerRect);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 400.));
            let drawn = lines(cx, &view);
            keys(cx, window, &["secondary-z"]);
            expect((drawn, lines(cx, &view)), (4, 0), "lines drawn, after one undo")
        } },
        Row { id: "undo / after ✓ one undo removes the sketch", live: "after ✓ on a new sketch, one Ctrl+Z removes the whole sketch feature; Ctrl+Y brings it back", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "ed-confirm", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let made = feature_names(cx, &view).len();
            // (The workspace takes Ctrl+Z / Ctrl+Y outside a dialog: the document's own undo here.)
            let doc = cx.update(|cx| view.read(cx).doc.clone());
            cx.update(|cx| doc.update(cx, |d, cx| d.undo(cx)));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let undone = feature_names(cx, &view).len();
            cx.update(|cx| doc.update(cx, |d, cx| d.redo(cx)));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            expect((made, undone, feature_names(cx, &view).len()), (1, 0, 1), "features: made, undone, redone")
        } },
        Row { id: "copy / sketch entities paste under the pointer", live: "Ctrl+V puts the copy's centre under the pointer at once, selected; one Ctrl+Z removes it", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "ed-paste", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            keys(cx, window, &["escape"]);
            let line = cx.update(|cx| view.read(cx).sketch_data(cx).and_then(|d| d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).map(|e| e.id)));
            cx.update(|cx| view.update(cx, |v, _| v.sketch.as_mut().unwrap().selection = line.into_iter().collect()));
            keys(cx, window, &["secondary-c"]);
            let at = glam::DVec2::new(650., 420.);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
            }))
            .unwrap();
            keys(cx, window, &["secondary-v"]);
            let (n, mid, selected) = cx.update(|cx| {
                let v = view.read(cx);
                let d = v.sketch_data(cx).unwrap();
                let ed = v.sketch.as_ref().unwrap();
                let newest = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).max_by_key(|e| e.id.0);
                let mid = newest.and_then(|e| e.endpoints()).and_then(|(a, b)| {
                    let (pa, pb) = (d.sketch.point(a)?, d.sketch.point(b)?);
                    v.viewport.read(cx).project(ed.frame.to_world((pa + pb) * 0.5))
                });
                (d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.construction).count(), mid, newest.is_some_and(|e| ed.selection.contains(&e.id)))
            });
            keys(cx, window, &["secondary-z"]);
            let off = mid.map_or(f64::MAX, |m| m.distance(at));
            expect((n, off < 2.0, selected, lines(cx, &view)), (2, true, true, 1), "lines after paste, centre under the pointer, selected, lines after undo")
        } },
        Row { id: "copy / a feature's Ctrl+C, Ctrl+V does nothing", live: "no new feature, no dialog, no toast", gap: false, check: |cx| {
            let (window, view) = block(cx, "ed-copy");
            select_feature(cx, &view, "Extrude 1");
            keys(cx, window, &["secondary-c", "secondary-v"]);
            let open = cx.update(|cx| view.read(cx).edit.is_some());
            expect((feature_names(cx, &view), open), (vec!["Sketch 1".to_string(), "Extrude 1".to_string()], false), "features, a dialog open")
        } },
        Row { id: "delete / a sketch another feature uses: no warning", live: "Delete on Sketch 1 removes it; Extrude 1 turns red; Parts (0)", gap: false, check: |cx| {
            let (window, view) = block(cx, "ed-del-sketch");
            select_feature(cx, &view, "Sketch 1");
            keys(cx, window, &["delete"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let open = cx.update(|cx| view.read(cx).edit.is_some() || crate::os_menu::is_open(cx));
            expect((feature_names(cx, &view), cx.update(|cx| view.read(cx).display.bodies.len()), open), (vec!["Extrude 1".to_string()], 0, false), "features, parts, a dialog")
        } },
        Row { id: "delete / a part adds Delete part 1", live: "Delete on a part adds a 'Delete part 1' feature at the end, no warning (behaviours 1.12: its dialog opens pre-filled, the row in the list)", gap: false, check: |cx| {
            let (window, view) = block(cx, "ed-del-part");
            cx.update(|cx| view.update(cx, |v, _| v.selection = v.display.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect()));
            keys(cx, window, &["delete"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let open = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.type_id.clone()));
            expect(open.as_deref(), Some("deleteBodies"), "the open feature")
        } },
        Row { id: "Y / Shift+Y with nothing selected or under the pointer", live: "nothing (Shift+Y isn't 'show all')", gap: false, check: |cx| {
            let (window, view) = block(cx, "ed-y");
            keys(cx, window, &["y"]);
            let hidden_by_y = cx.update(|cx| view.read(cx).display.bodies.iter().filter(|b| b.hidden).count());
            cx.update(|cx| view.update(cx, |v, cx| {
                let all: Vec<Reference> = v.display.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
                v.set_visibility(&all, true, cx);
            }));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            keys(cx, window, &["shift-y"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let shown = cx.update(|cx| view.read(cx).display.bodies.iter().filter(|b| !b.hidden).count());
            expect((hidden_by_y, shown), (0, 0), "hidden by Y, shown by Shift+Y")
        } },
        Row { id: "eye icons / a click toggles, the selection stays", live: "a row's eye (at its right on hover) toggles that item: Part 1 hidden -> shown, a hidden sketch's closed eye -> shown; the selection as it was", gap: false, check: |cx| {
            let (window, view) = block(cx, "ed-eyes");
            select_feature(cx, &view, "Extrude 1");
            let part = cx.update(|cx| view.read(cx).display.bodies.first().map(|b| b.id.clone())).ok_or("no part")?;
            let sketch = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").map(|f| f.id)).ok_or("no sketch")?;
            let press = |cx: &mut TestAppContext, sel: String| -> Result<(), String> {
                draw(cx, window);
                let b = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(sel.clone().into_boxed_str())).ok_or(format!("no {sel}"))?;
                let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
                vcx.simulate_event(gpui_kit::MouseMoveEvent { position: b.center(), pressed_button: None, modifiers: Modifiers::default() });
                vcx.simulate_click(b.center(), Modifiers::default());
                wait_until(cx, window, |cx| !view.read(cx).ui_busy());
                Ok(())
            };
            let part_hidden = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.id == part).map(|b| b.hidden));
            let sketch_hidden = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.id == sketch).map(|f| f.hidden));
            let before = (part_hidden(cx), sketch_hidden(cx));
            press(cx, format!("part-eye-{part}"))?;
            let hidden = part_hidden(cx);
            press(cx, format!("part-eye-{part}"))?;
            let shown = part_hidden(cx);
            press(cx, format!("feat-eye-{sketch:?}"))?;
            let sketch_after = sketch_hidden(cx);
            let selected: Vec<String> = cx.update(|cx| {
                let v = view.read(cx);
                v.studio(cx).unwrap().features.iter().filter(|f| v.selected_features.contains(&f.id)).map(|f| f.name.clone()).collect()
            });
            expect((before, hidden, shown, sketch_after, selected), ((Some(false), Some(true)), Some(true), Some(false), Some(false), vec!["Extrude 1".to_string()]), "(part, sketch) hidden before; the part after one click, two; the sketch after its click; selected")
        } },
    ]
}

#[gpui::test]
fn the_editing_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
        let res = (r.check)(cx);
        let ok = res.is_ok();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, res.err().map(|e| format!("\n    {e}")).unwrap_or_default()));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("editing matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
