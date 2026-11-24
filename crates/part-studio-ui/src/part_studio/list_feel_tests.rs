//! The feature list and Parts list matrix (docs/parity/interaction-feel.md "Feature list and Parts list"): one row per
//! behaviour live2 measured (live/feel/featurelist.json), acted out with real clicks, double clicks and drags at the
//! rows on a fresh Part Studio, at a device pixel ratio of 1 and 2. A row ours doesn't match yet is marked a gap; the
//! test fails when a row's status moves either way.

use super::*;
use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, point};

const FEATURES: [&str; 7] = ["Sketch 1", "Extrude 1", "Sketch 2", "Extrude 2", "Plane 1", "Sketch 3", "Extrude 3"];

/// Three parts, each a sketch and an extrude, Plane 1 between the second and the third.
fn studio(cx: &mut TestAppContext, name: &str, scale: f32) -> List {
    let rect = |n: u32, x: f64| serde_json::json!({ "type": "newSketch", "name": format!("Sketch {n}"), "plane": "Top", "shapes": [{ "corner_rectangle": [[x, 0.0], [x + 20.0, 20.0]] }] });
    let extrude = |n: u32| serde_json::json!({ "type": "extrude", "name": format!("Extrude {n}"), "params": { "entities": [format!("Sketch {n} region 0")], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } });
    let plane = serde_json::json!({ "type": "cPlane", "name": "Plane 1", "params": { "entities": ["plane:top"], "cplaneType": "OFFSET", "offset": "50 mm" } });
    let doc = serde_json::json!({ "units": "mm", "features": [rect(1, 0.0), extrude(1), rect(2, 40.0), extrude(2), plane, rect(3, 80.0), extrude(3)] });
    let (window, view) = part_studio_from(cx, name, Some(doc));
    cx.update_window(window, |_, w, _| {
        w.set_scale_factor(scale);
        w.activate_window();
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 3);
    draw(cx, window);
    List { window, view }
}

struct List {
    window: AnyWindowHandle,
    view: Entity<PartStudioView>,
}

impl List {
    fn bounds(&self, cx: &mut TestAppContext, sel: &str) -> Option<Bounds<Pixels>> {
        draw(cx, self.window);
        gpui_kit::VisualTestContext::from_window(self.window, cx).debug_bounds(Box::leak(sel.to_string().into_boxed_str()))
    }

    /// On a row's name, half way down.
    fn at(&self, cx: &mut TestAppContext, row: &str) -> Point<Pixels> {
        let b = self.bounds(cx, row).unwrap_or_else(|| panic!("no row {row}"));
        point(b.origin.x + px(40.), b.center().y)
    }

    /// `dy` px below the row's top edge.
    fn at_y(&self, cx: &mut TestAppContext, row: &str, dy: f32) -> Point<Pixels> {
        let b = self.bounds(cx, row).unwrap_or_else(|| panic!("no row {row}"));
        point(b.origin.x + px(40.), b.origin.y + px(dy))
    }

    /// Empty graphics, away from the parts, the triad and the view cube.
    fn canvas(&self, cx: &mut TestAppContext) -> Point<Pixels> {
        let o = cx.update(|cx| self.view.read(cx).viewport.read(cx).origin());
        point(o.x + px(300.), o.y + px(560.))
    }

    fn event(&self, cx: &mut TestAppContext, e: impl gpui_kit::InputEvent) {
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_event(e);
    }

    fn press(&self, cx: &mut TestAppContext, at: Point<Pixels>, button: MouseButton, modifiers: Modifiers, click_count: usize) {
        self.event(cx, MouseMoveEvent { position: at, pressed_button: None, modifiers });
        self.event(cx, MouseDownEvent { button, position: at, modifiers, click_count, first_mouse: false });
        // (A click lasts frames: what the press opened draws before the release.)
        draw(cx, self.window);
        self.event(cx, MouseUpEvent { button, position: at, modifiers, click_count });
        draw(cx, self.window);
    }

    /// A click a person's pause after the last one (past the list's 400 ms double-click).
    fn click(&self, cx: &mut TestAppContext, row: &str, m: Modifiers) {
        cx.executor().advance_clock(std::time::Duration::from_millis(500));
        let at = self.at(cx, row);
        self.press(cx, at, MouseButton::Left, m, 1);
    }

    fn double_click_at(&self, cx: &mut TestAppContext, at: Point<Pixels>) {
        self.press(cx, at, MouseButton::Left, NONE, 1);
        self.press(cx, at, MouseButton::Left, NONE, 2);
        wait_until(cx, self.window, |cx| !self.view.read(cx).ui_busy());
        draw(cx, self.window);
    }

    fn double_click(&self, cx: &mut TestAppContext, row: &str) {
        let at = self.at(cx, row);
        self.double_click_at(cx, at);
    }

    fn right_click(&self, cx: &mut TestAppContext, row: &str) {
        let at = self.at(cx, row);
        self.press(cx, at, MouseButton::Right, NONE, 1);
    }

    /// Keys typed a moment after the last click (a person's 100 ms: what the click moved the keyboard to has it).
    fn keys(&self, cx: &mut TestAppContext, keys: &str) {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        draw(cx, self.window);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_keystrokes(keys);
        draw(cx, self.window);
    }

    /// A left drag from `from` to `to` in 2 px steps, released there when `release`.
    fn drag(&self, cx: &mut TestAppContext, from: Point<Pixels>, to: Point<Pixels>, release: bool) {
        self.event(cx, MouseMoveEvent { position: from, pressed_button: None, modifiers: NONE });
        self.event(cx, MouseDownEvent { button: MouseButton::Left, position: from, modifiers: NONE, click_count: 1, first_mouse: false });
        let d = to - from;
        let steps = (f32::from(d.x.abs().max(d.y.abs())) / 2.).ceil().max(1.) as usize;
        for k in 1..=steps {
            let t = k as f32 / steps as f32;
            let p = point(from.x + d.x * t, from.y + d.y * t);
            self.event(cx, MouseMoveEvent { position: p, pressed_button: Some(MouseButton::Left), modifiers: NONE });
            draw(cx, self.window);
        }
        if release {
            self.event(cx, MouseUpEvent { button: MouseButton::Left, position: to, modifiers: NONE, click_count: 1 });
            draw(cx, self.window);
            wait_until(cx, self.window, |cx| !self.view.read(cx).ui_busy());
        }
    }

    fn release(&self, cx: &mut TestAppContext, at: Point<Pixels>) {
        self.event(cx, MouseUpEvent { button: MouseButton::Left, position: at, modifiers: NONE, click_count: 1 });
        draw(cx, self.window);
    }

    /// The selected rows' names in list order: default geometry, features, parts.
    fn selected(&self, cx: &mut TestAppContext) -> Vec<String> {
        cx.update(|cx| {
            let v = self.view.read(cx);
            let mut out = Vec::new();
            if v.selection.contains(&Reference::Origin) {
                out.push("Origin".to_string());
            }
            for (p, n) in [(DatumPlane::Top, "Top"), (DatumPlane::Front, "Front"), (DatumPlane::Right, "Right")] {
                if v.selection.contains(&Reference::Plane(p)) {
                    out.push(n.to_string());
                }
            }
            if let Some(s) = v.studio(cx) {
                out.extend(s.features.iter().filter(|f| v.selected_features.contains(&f.id)).map(|f| f.name.clone()));
            }
            out.extend(v.display.bodies.iter().filter(|b| v.selection.contains(&Reference::Body(b.id.clone()))).map(|b| b.name.clone()));
            out
        })
    }

    fn order(&self, cx: &mut TestAppContext) -> Vec<String> {
        cx.update(|cx| self.view.read(cx).studio(cx).map(|s| s.features.iter().map(|f| f.name.clone()).collect()).unwrap_or_default())
    }

    /// The feature just above the rollback bar.
    fn rolled_to(&self, cx: &mut TestAppContext) -> Option<String> {
        cx.update(|cx| self.view.read(cx).studio(cx).and_then(|s| s.rollback_index().checked_sub(1).and_then(|i| s.features.get(i)).map(|f| f.name.clone())))
    }

    fn editing(&self, cx: &mut TestAppContext) -> Option<String> {
        cx.update(|cx| self.view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.name.clone()))
    }

    /// The open context menu's rows, separators left out.
    fn menu(&self, cx: &mut TestAppContext) -> Vec<String> {
        let rows = cx.update(|cx| {
            let graphics = self.view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels());
            graphics.unwrap_or_else(|| crate::os_menu::open_labels(cx))
        });
        rows.into_iter().filter(|s| s != "———").collect()
    }

    /// The open context menu's rows, separators as "———".
    fn menu_grouped(&self, cx: &mut TestAppContext) -> Vec<String> {
        cx.update(|cx| {
            let graphics = self.view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels());
            graphics.unwrap_or_else(|| crate::os_menu::open_labels(cx))
        })
    }

    /// The pointer onto a row from the graphics, then `ms` of the clock.
    fn hover(&self, cx: &mut TestAppContext, row: &str, ms: u64) {
        let (from, at) = (self.canvas(cx), self.at(cx, row));
        self.event(cx, MouseMoveEvent { position: from, pressed_button: None, modifiers: NONE });
        draw(cx, self.window);
        crate::ui_script::clear_tooltip();
        self.event(cx, MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
        self.wait(cx, ms);
    }

    /// `ms` of the clock, drawn every 10 ms.
    fn wait(&self, cx: &mut TestAppContext, ms: u64) {
        for _ in 0..ms / 10 {
            cx.executor().advance_clock(std::time::Duration::from_millis(10));
            draw(cx, self.window);
        }
    }

    /// The hidden sketches the scene draws for now, and whether their geometry is lit as the hover.
    fn drawn_sketches(&self, cx: &mut TestAppContext) -> (Vec<String>, bool) {
        cx.update(|cx| {
            let v = self.view.read(cx);
            let shown = v.sketches_shown_for_now();
            let drawn: Vec<String> = v.display_for_scene().sketches.iter().filter(|s| !s.hidden && shown.contains(&s.feature)).map(|s| s.name.clone()).collect();
            let lit = !shown.is_empty() && v.highlighted_hover(cx).iter().any(|r| matches!(r, Reference::SketchEntity { sketch, .. } if shown.contains(sketch)));
            (drawn, lit)
        })
    }

    fn lit_row(&self, cx: &mut TestAppContext) -> Option<String> {
        cx.update(|cx| {
            let v = self.view.read(cx);
            let id = v.ui.list_hover?;
            v.studio(cx)?.feature(id).map(|f| f.name.clone())
        })
    }

    /// The folders: name, features inside (list order), collapsed.
    fn folders(&self, cx: &mut TestAppContext) -> Vec<(String, Vec<String>, bool)> {
        cx.update(|cx| {
            let v = self.view.read(cx);
            let Some(s) = v.studio(cx) else { return Vec::new() };
            s.folders.iter().map(|fd| (fd.name.clone(), s.features.iter().filter(|f| f.folder == Some(fd.id)).map(|f| f.name.clone()).collect(), v.folder_collapsed(fd))).collect()
        })
    }

    /// The header's New folder, then Enter in its popover.
    fn new_folder(&self, cx: &mut TestAppContext, enter: bool) {
        let at = self.bounds(cx, "button:New folder").expect("no New folder button").center();
        self.press(cx, at, MouseButton::Left, NONE, 1);
        if enter {
            self.keys(cx, "enter");
        }
    }

    /// Each folder's name and its parent's.
    fn parents(&self, cx: &mut TestAppContext) -> Vec<(String, Option<String>)> {
        cx.update(|cx| {
            let v = self.view.read(cx);
            let Some(s) = v.studio(cx) else { return Vec::new() };
            s.folders.iter().filter(|f| !f.name.chars().last().is_some_and(|c| c.is_ascii_digit())).map(|f| (f.name.clone(), f.parent.and_then(|p| s.folder(p)).map(|p| p.name.clone()))).collect()
        })
    }

    /// The document's Undo (what the workspace's Ctrl+Z runs).
    fn undo(&self, cx: &mut TestAppContext) {
        cx.update(|cx| self.view.read(cx).doc.clone()).update(cx, |d, cx| d.undo(cx));
        draw(cx, self.window);
    }

    fn dragging(&self, cx: &mut TestAppContext) -> bool {
        cx.update(|cx| cx.has_active_drag())
    }
}

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false, platform: false, function: false };
const SHIFT: Modifiers = Modifiers { shift: true, ..NONE };
const CTRL: Modifiers = Modifiers { control: true, ..NONE };
const ALT: Modifiers = Modifiers { alt: true, ..NONE };
const CMD: Modifiers = Modifiers { platform: true, ..NONE };

type Check = fn(&List, &mut TestAppContext) -> Result<(), String>;

/// The row's menu › Rename: the inline input open.
fn rename(l: &List, cx: &mut TestAppContext, row: &str) {
    l.right_click(cx, &format!("feature:{row}"));
    let at = l.bounds(cx, "menu:Rename").expect("no Rename row").center();
    l.press(cx, at, MouseButton::Left, NONE, 1);
}

fn expect<T: PartialEq + std::fmt::Debug>(got: T, want: T, what: &str) -> Result<(), String> {
    if got == want { Ok(()) } else { Err(format!("{what}: ours {got:?}, live {want:?}")) }
}

fn names(n: &[&str]) -> Vec<String> {
    n.iter().map(|s| s.to_string()).collect()
}

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "click / toggles, others kept", live: "Extrude 1 then Extrude 3 selects both", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 3", NONE);
            expect(l.selected(cx), names(&["Extrude 1", "Extrude 3"]), "selected")
        } },
        Row { id: "click / a selected row deselects it", live: "clicking Extrude 2 twice leaves nothing", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 2", NONE);
            l.click(cx, "feature:Extrude 2", NONE);
            expect(l.selected(cx), names(&[]), "selected")
        } },
        Row { id: "Alt+click / as a plain click", live: "adds the row", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 2", ALT);
            expect(l.selected(cx), names(&["Extrude 1", "Extrude 2"]), "selected")
        } },
        Row { id: "Shift / the range from the anchor", live: "Extrude 1, Shift Extrude 3: every row between, sketches and the plane too", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 3", SHIFT);
            expect(l.selected(cx), names(&FEATURES[1..]), "selected")
        } },
        Row { id: "Shift / upwards", live: "Extrude 3, Shift Extrude 1: the same range", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 3", NONE);
            l.click(cx, "feature:Extrude 1", SHIFT);
            expect(l.selected(cx), names(&FEATURES[1..]), "selected")
        } },
        Row { id: "Shift / on a selected row deselects the range", live: "Extrude 1, Shift Extrude 3, Shift Extrude 2 leaves Extrude 1 and Sketch 2", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 3", SHIFT);
            l.click(cx, "feature:Extrude 2", SHIFT);
            expect(l.selected(cx), names(&["Extrude 1", "Sketch 2"]), "selected")
        } },
        Row { id: "Shift / nothing selected: from the top", live: "Origin, Top, Front, Right, Sketch 1 … the clicked row", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", SHIFT);
            expect(l.selected(cx), names(&["Origin", "Top", "Front", "Right", "Sketch 1", "Extrude 1"]), "selected")
        } },
        Row { id: "Shift / a part with a feature as anchor", live: "does nothing", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "part:Part 2", SHIFT);
            expect(l.selected(cx), names(&["Extrude 1"]), "selected")
        } },
        Row { id: "Ctrl+click a feature / its menu, selection kept", live: "opens the context menu as a right click; the selection doesn't change", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 2", NONE);
            l.click(cx, "feature:Extrude 1", CTRL);
            expect((!l.menu(cx).is_empty(), l.selected(cx)), (true, names(&["Extrude 2"])), "menu open, selected")
        } },
        Row { id: "clearing / a click on empty canvas", live: "clears everything", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "part:Part 2", NONE);
            // (What the viewport sends once the pick under the click comes back empty: the headless GPU picks nothing.)
            let at = l.canvas(cx);
            cx.update(|cx| l.view.update(cx, |v, cx| v.viewport.update(cx, |_, cx| cx.emit(ViewportEvent::Click { pick: None, position: glam::DVec2::new(f64::from(f32::from(at.x)), f64::from(f32::from(at.y))), modifiers: NONE }))));
            draw(cx, l.window);
            wait_until(cx, l.window, |cx| l.view.read(cx).selection.is_empty() && l.view.read(cx).selected_features.is_empty());
            expect(l.selected(cx), names(&[]), "selected")
        } },
        Row { id: "clearing / Escape keeps the selection", live: "Escape doesn't clear the list's selection", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.keys(cx, "escape");
            expect(l.selected(cx), names(&["Extrude 1"]), "selected")
        } },
        Row { id: "clearing / a click on empty list space", live: "keeps the selection", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            let at = l.at_y(cx, "part:Part 3", 90.);
            l.press(cx, at, MouseButton::Left, NONE, 1);
            expect(l.selected(cx), names(&["Extrude 1"]), "selected")
        } },
        Row { id: "double-click / a feature: its dialog", live: "Extrude 1's edit dialog", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Extrude 1");
            expect(l.editing(cx), Some("Extrude 1".to_string()), "editing")
        } },
        Row { id: "double-click / a quick one shows no row tooltip, a shown one stays", live: "Sheet metal model 1 double-clicked at once: no tooltip at 1.35 s (qa wf23 01 s17); Extrude 1's name shown before the double-click stays (wf10); live row-tip-dblclick-1123: none after a double-click in 12 trials (0 / 50 / 300 ms leads), a shown one stays through both clicks", gap: true, check: |l, cx| {
            crate::ui_script::clear_tooltip();
            l.double_click(cx, "feature:Extrude 1");
            l.wait(cx, 1350);
            let shown = crate::ui_script::shown_tooltip();
            let quick = shown.is_some();
            l.keys(cx, "escape");
            l.wait(cx, 450);
            crate::ui_script::clear_tooltip();
            l.hover(cx, "feature:Extrude 1", 1200);
            l.double_click(cx, "feature:Extrude 1");
            let kept = crate::ui_script::shown_tooltip().is_some();
            l.keys(cx, "escape");
            expect((quick, kept), (false, true), &format!("(a tooltip 1.35 s after a quick double-click {shown:?}, one shown before kept)"))
        } },
        Row { id: "double-click / a feature's blank right side", live: "its dialog too", gap: false, check: |l, cx| {
            let b = l.bounds(cx, "feature:Extrude 1").ok_or("no row")?;
            l.double_click_at(cx, point(b.origin.x + b.size.width - px(40.), b.center().y));
            expect(l.editing(cx), Some("Extrude 1".to_string()), "editing")
        } },
        Row { id: "double-click / the list's own: a second click 80 px along within 400 ms", live: "two clicks on the same row within ≈370–400 ms open its dialog anywhere on the row (no native dblclick); 420 ms apart or on another row never (live2 consts-1118, pinned 07:26)", gap: false, check: |l, cx| {
            let a = l.at(cx, "feature:Extrude 1");
            let b = point(a.x + px(80.), a.y);
            l.press(cx, a, MouseButton::Left, NONE, 1);
            cx.executor().advance_clock(std::time::Duration::from_millis(350));
            l.press(cx, b, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            let near = l.editing(cx);
            l.keys(cx, "escape");
            cx.executor().advance_clock(std::time::Duration::from_millis(600));
            l.press(cx, a, MouseButton::Left, NONE, 1);
            cx.executor().advance_clock(std::time::Duration::from_millis(420));
            l.press(cx, b, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            let late = l.editing(cx);
            // The second click on the next row: two clicks, no dialog.
            cx.executor().advance_clock(std::time::Duration::from_millis(600));
            l.press(cx, a, MouseButton::Left, NONE, 1);
            cx.executor().advance_clock(std::time::Duration::from_millis(200));
            let next = l.at(cx, "feature:Sketch 2");
            l.press(cx, next, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            expect((near, late, l.editing(cx)), (Some("Extrude 1".to_string()), None, None), "(editing after two clicks 80 px and 350 ms apart, 420 ms apart, on the next row)")
        } },
        Row { id: "dialog / a new feature's row shows while its dialog is open", live: "Extrude from the toolbar: the list shows the new \"Extrude 4\" row at the rollback bar while the dialog is open (live workflow-1118)", gap: false, check: |l, cx| {
            cx.update_window(l.window, |_, w, cx| l.view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
            draw(cx, l.window);
            let new = l.bounds(cx, "feature:new").ok_or("no row for the new feature")?;
            let last = l.bounds(cx, "feature:Extrude 3").ok_or("no Extrude 3 row")?;
            let name = cx.update(|cx| l.view.read(cx).placeholder_name(cx));
            l.keys(cx, "escape");
            expect((name, new.top() >= last.bottom()), ("Extrude 4".to_string(), true), "(the new row's name, below the last feature)")
        } },
        Row { id: "double-click / a sketch: sketch mode", live: "edits Sketch 1", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Sketch 1");
            expect(cx.update(|cx| l.view.read(cx).sketch.is_some()), true, "in sketch mode")
        } },
        Row { id: "double-click / a default plane: left selected", live: "no dialog, no rename, the plane left selected, not two toggles (live list-1118; live's document had no Plane feature: its plane row is Top)", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Top");
            let renaming = cx.update(|cx| l.view.read(cx).ui.dg_renaming.is_some());
            expect((l.editing(cx), renaming, l.selected(cx)), (None, false, names(&["Top"])), "editing, renaming, selected")
        } },
        Row { id: "double-click / a part: two toggles", live: "nothing selected after, no rename", gap: false, check: |l, cx| {
            l.double_click(cx, "part:Part 1");
            let renaming = cx.update(|cx| l.view.read(cx).ui.part_renaming.is_some());
            expect((renaming, l.selected(cx)), (false, names(&[])), "renaming, selected")
        } },
        Row { id: "keys / Y after a part's double-click", live: "hides the part under the pointer, nothing being selected (ui-parity parts-row-dblclick-rename-escape, q2572)", gap: false, check: |l, cx| {
            l.double_click(cx, "part:Part 1");
            l.keys(cx, "y");
            let part1_hidden = |cx: &gpui_kit::App| l.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.hidden);
            wait_until(cx, l.window, |cx| part1_hidden(cx) == Some(true));
            expect((cx.update(|cx| part1_hidden(cx)), l.selected(cx)), (Some(true), names(&[])), "Part 1 hidden, selected")
        } },
        Row { id: "keys / Y and Shift+Y: the part goes and comes back with the key, not the regeneration", live: "Y: the Parts row at 0.5 at 73 ms, the part gone at 436 ms; Shift+Y back at 108 / 562 ms (ps-feel2-1118; ours waited for the regeneration: 269 / 669 ms on live2's lane)", gap: false, check: |l, cx| {
            l.click(cx, "part:Part 1", NONE);
            // Read in the same update as the key's handler: before any regeneration could come back.
            let part1 = |v: &PartStudioView| v.display.bodies.iter().find(|b| b.name == "Part 1").map(|b| (b.id.clone(), b.hidden));
            let hidden = cx.update(|cx| l.view.update(cx, |v, cx| {
                v.hide_hovered_or_selected(cx);
                part1(v)
            }));
            let Some((id, _)) = hidden.clone() else { return Err("no Part 1".into()) };
            let shown = cx.update(|cx| l.view.update(cx, |v, cx| {
                v.set_visibility(&[Reference::Body(id.clone())], false, cx);
                part1(v).map(|p| p.1)
            }));
            expect((hidden.map(|p| p.1), shown), (Some(true), Some(false)), "Part 1 hidden at Y, shown at Shift+Y")
        } },
        Row { id: "rename / Rename in the menu: inline, all selected", live: "an input over the row, the whole name selected; Escape cancels", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Extrude 1");
            let at = l.bounds(cx, "menu:Rename").ok_or("no Rename row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            let input = cx.update(|cx| l.view.read(cx).renaming.as_ref().map(|(_, st, _)| (st.read(cx).value().to_string(), st.read(cx).selected_range().len())));
            l.keys(cx, "x escape");
            let after = (cx.update(|cx| l.view.read(cx).renaming.is_some()), l.order(cx)[1].clone());
            expect((input, after), (Some(("Extrude 1".to_string(), 9)), (false, "Extrude 1".to_string())), "input (value, selected bytes), after typing and Escape")
        } },
        Row { id: "rename / the input's box", live: "square, 1 px #1651B0, 24 px tall, to the list's right edge (featurelist-feel D1: x 62-239, 178 x 24)", gap: false, check: |l, cx| {
            rename(l, cx, "Extrude 1");
            let (b, row) = (l.bounds(cx, "rename-box").ok_or("no rename box")?, l.bounds(cx, "feature:Extrude 1").ok_or("no row")?);
            l.keys(cx, "escape");
            expect((f32::from(b.size.height), f32::from(b.right() - row.right())), (24., 0.), "(height, right edge - the list's)")
        } },
        Row { id: "parts / rows 26 px apart, the clicked row keeps its tooltip", live: "Part 1 and Part 2 26 px apart (24 px band); a click on Part 1 leaves its 'Part 1' tooltip shown (qa wf23b 17 s11)", gap: false, check: |l, cx| {
            let (a, b) = (l.bounds(cx, "part-band:Part 1").ok_or("no Part 1")?, l.bounds(cx, "part-band:Part 2").ok_or("no Part 2")?);
            l.hover(cx, "part:Part 1", 450);
            l.click(cx, "part:Part 1", NONE);
            l.wait(cx, 450);
            let tip = crate::ui_script::shown_tooltip();
            crate::ui_script::clear_tooltip();
            expect((f32::from(b.top() - a.top()), f32::from(a.size.height), tip), (26., 24., Some("Part 1".to_string())), "(pitch, band, tooltip after the click)")
        } },
        Row { id: "dialog / Final", live: "a 49 x 22 button right of the slider; pressed, Final is on (walk8 9a-9c)", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Extrude 1");
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            let (fin, slider) = (l.bounds(cx, "dlg-final").ok_or("no Final")?, l.bounds(cx, "dlg-preview").ok_or("no slider")?);
            l.press(cx, fin.center(), MouseButton::Left, NONE, 1);
            let on = cx.update(|cx| l.view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).show_final));
            l.keys(cx, "escape");
            expect((f32::from(fin.size.width), f32::from(fin.size.height), fin.left() > slider.right(), on), (49., 22., true, Some(true)), "(width, height, after the slider, on after a click)")
        } },
        Row { id: "menu / one feature", live: "Rename | Edit… | Edit Sketch 1… | Show dimensions | Add selection to folder… | Suppress | Dynamic suppression | Zoom to selection | Show dependencies… | Roll to here | Add appearance to feature… | Delete (Add comment: cloud)", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Extrude 1");
            expect(l.menu(cx), names(&["Rename", "Edit…", "Edit Sketch 1…", "Show dimensions", "Add selection to folder…", "Suppress", "Dynamic suppression", "Zoom to selection", "Show dependencies…", "Roll to here", "Add appearance to feature…", "Delete"]), "rows")
        } },
        Row { id: "menu / two features, on one of them", live: "Add selection to folder… | Suppress | Dynamic suppression | Clear selection | Add appearance to 2 features… | Delete", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 2", NONE);
            l.right_click(cx, "feature:Extrude 2");
            expect(l.menu(cx), names(&["Add selection to folder…", "Suppress", "Dynamic suppression", "Clear selection", "Add appearance to 2 features…", "Delete"]), "rows")
        } },
        Row { id: "menu / two selected, on a third", live: "the same menu counting 3; the third not selected", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 2", NONE);
            l.right_click(cx, "feature:Extrude 3");
            let counts = l.menu(cx).iter().any(|s| s == "Add appearance to 3 features…");
            expect((counts, l.selected(cx)), (true, names(&["Extrude 1", "Extrude 2"])), "counts 3, selected")
        } },
        Row { id: "menu / one sketch", live: "Rename | Edit… | Copy sketch | Show dimensions | Add selection to folder… | Show | Show all sketches | Create Drawing of Sketch 1… | Export as DXF/DWG… | Section view… | Suppress | Dynamic suppression | Zoom to selection | View normal to | Show dependencies… | Roll to here | Edit sketch appearance… | Delete on a plain right-click, again after its own menu's Esc; after a click on its row Clear selection after View normal to (live keys-1118, secmenu-rerecord-1121, secmenu-1121 state 12; with another sketch there Show all sketches too: live 1714; Add comment: cloud)", gap: false, check: |l, cx| {
            let plain = names(&["Rename", "Edit…", "Copy sketch", "Show dimensions", "Add selection to folder…", "Show", "Show all sketches", "Create Drawing of Sketch 1…", "Export as DXF/DWG…", "Section view…", "Suppress", "Dynamic suppression", "Zoom to selection", "View normal to", "Show dependencies…", "Roll to here", "Edit sketch appearance…", "Delete"]);
            l.right_click(cx, "feature:Sketch 1");
            expect(l.menu(cx), plain.clone(), "nothing selected")?;
            l.keys(cx, "escape");
            l.right_click(cx, "feature:Sketch 1");
            expect(l.menu(cx), plain.clone(), "after its own menu")?;
            l.keys(cx, "escape");
            l.click(cx, "feature:Sketch 1", NONE);
            l.right_click(cx, "feature:Sketch 1");
            // Selected visibly: Clear selection after View normal to (secmenu-1121 state 12).
            let mut clicked = plain;
            let k = clicked.iter().position(|r| r == "View normal to").map_or(0, |k| k + 1);
            clicked.insert(k, "Clear selection".to_string());
            expect(l.menu(cx), clicked, "after a click on its row")
        } },
        Row { id: "menu / two sketches", live: "Add selection to folder… | Show | Show all sketches | Section view… | Suppress | Dynamic suppression | Clear selection | Edit appearance of 2 sketches… | Delete", gap: false, check: |l, cx| {
            l.click(cx, "feature:Sketch 1", NONE);
            l.click(cx, "feature:Sketch 2", NONE);
            l.right_click(cx, "feature:Sketch 2");
            expect(l.menu(cx), names(&["Add selection to folder…", "Show", "Show all sketches", "Section view…", "Suppress", "Dynamic suppression", "Clear selection", "Edit appearance of 2 sketches…", "Delete"]), "rows")
        } },
        Row { id: "menu / a sketch and a feature", live: "Add selection to folder… | Hide other sketches | Hide all sketches | Section view… | Suppress | Dynamic suppression | Clear selection | Edit sketch appearance… | Add appearance to feature… | Delete", gap: false, check: |l, cx| {
            l.click(cx, "feature:Sketch 1", NONE);
            l.click(cx, "feature:Extrude 2", NONE);
            l.right_click(cx, "feature:Extrude 2");
            expect(l.menu(cx), names(&["Add selection to folder…", "Hide other sketches", "Hide all sketches", "Section view…", "Suppress", "Dynamic suppression", "Clear selection", "Edit sketch appearance…", "Add appearance to feature…", "Delete"]), "rows")
        } },
        Row { id: "menu / a feature and a part, on the feature", live: "the one-feature menu with Clear selection before Show dependencies…", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "part:Part 2", NONE);
            l.right_click(cx, "feature:Extrude 1");
            expect(l.menu(cx), names(&["Rename", "Edit…", "Edit Sketch 1…", "Show dimensions", "Add selection to folder…", "Suppress", "Dynamic suppression", "Zoom to selection", "Clear selection", "Show dependencies…", "Roll to here", "Add appearance to feature…", "Delete"]), "rows")
        } },
        Row { id: "menu / configured suppression", live: "a feature whose suppression is configured: Unconfigure suppression where Configure suppression was (live rollto-menus 11-20)", gap: false, check: |l, cx| {
            l.view.update(cx, |v, cx| {
                let fid = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.name == "Extrude 1").map(|f| f.id)).expect("Extrude 1");
                v.edit_config("configure", cx, |ps| {
                    let input = noshape_ops::config::add_input(ps, "Checkbox");
                    noshape_ops::config::configure_parameter(ps, fid, noshape_model::configuration::SUPPRESSED_PARAM, &input);
                });
            });
            l.right_click(cx, "feature:Extrude 1");
            let rows = l.menu(cx);
            expect((rows.iter().any(|r| r == "Unconfigure suppression"), rows.iter().any(|r| r == "Configure suppression")), (true, false), &format!("rows {rows:?}"))?;
            l.keys(cx, "escape");
            l.right_click(cx, "feature:Extrude 2");
            let rows = l.menu(cx);
            expect((rows.iter().any(|r| r == "Unconfigure suppression"), rows.iter().any(|r| r == "Configure suppression")), (false, true), &format!("Extrude 2's rows {rows:?}"))
        } },
        Row { id: "menu / rolled back under Sketch 1", live: "Sketch 1 › Roll to here: Sketch 1 keeps its full menu (no Section view…), Hide | Hide other sketches | Hide all sketches and Roll to end in place of Roll to here; every row below the bar: Rename | Add selection to folder… | Section view… (a sketch: Hide other sketches | Hide all sketches) | Suppress | Dynamic suppression | (Un)configure suppression | Roll to here | Roll to end | Delete (live rollback-menus 11-21; Add comment: cloud)", gap: false, check: |l, cx| {
            l.view.update(cx, |v, cx| {
                let fid = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.name == "Extrude 2").map(|f| f.id)).expect("Extrude 2");
                v.edit_config("configure", cx, |ps| {
                    let input = noshape_ops::config::add_input(ps, "Checkbox");
                    noshape_ops::config::configure_parameter(ps, fid, noshape_model::configuration::SUPPRESSED_PARAM, &input);
                });
            });
            l.right_click(cx, "feature:Sketch 1");
            let k = l.menu(cx).iter().position(|r| r == "Roll to here").expect("Roll to here");
            l.keys(cx, &vec!["down"; k + 1].join(" "));
            l.keys(cx, "enter");
            draw(cx, l.window);
            expect(l.rolled_to(cx).as_deref(), Some("Sketch 1"), "the bar under")?;
            let menu = |cx: &mut TestAppContext, row: &str| {
                l.right_click(cx, &format!("feature:{row}"));
                let rows = l.menu(cx);
                l.keys(cx, "escape");
                rows
            };
            let sketch1 = menu(cx, "Sketch 1");
            let hides = sketch1.iter().position(|r| r == "Add selection to folder…").map(|k| sketch1[k + 1..].iter().take(3).cloned().collect::<Vec<_>>());
            // qa ROLLBACK-ABOVE-BAR-SECTION-VIEW: no Section view… after its own Roll to here.
            let flags = |r: &str| sketch1.iter().any(|x| x == r);
            expect((hides, flags("Roll to end"), flags("Roll to here"), flags("Edit…"), flags("Section view…")), (Some(names(&["Hide", "Hide other sketches", "Hide all sketches"])), true, false, true, false), &format!("Sketch 1's rows {sketch1:?}"))?;
            let tail = |configure: &str| names(&["Suppress", "Dynamic suppression", configure, "Roll to here", "Roll to end", "Delete"]);
            let short = |middle: &[&str], configure: &str| names(&["Rename", "Add selection to folder…"]).into_iter().chain(names(middle)).chain(tail(configure)).collect::<Vec<_>>();
            expect(menu(cx, "Extrude 1"), short(&["Section view…"], "Configure suppression"), "Extrude 1")?;
            expect(menu(cx, "Sketch 2"), short(&["Hide other sketches", "Hide all sketches"], "Configure suppression"), "Sketch 2")?;
            expect(menu(cx, "Extrude 2"), short(&["Section view…"], "Unconfigure suppression"), "Extrude 2")
        } },
        Row { id: "menu / after a sketch's menu", live: "Section view… only with a sketch or plane selected: none on Extrude 1 with nothing selected; after a right-click on Sketch 2 and Esc (nothing shown selected) Extrude 1's own menu has it after Add selection to folder… (live psui 11-20 19:11, dm/secview.json)", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Extrude 1");
            let rows = l.menu(cx);
            expect(rows.iter().any(|r| r == "Section view…"), false, &format!("nothing selected: rows {rows:?}"))?;
            l.keys(cx, "escape");
            l.right_click(cx, "feature:Sketch 2");
            l.keys(cx, "escape");
            l.right_click(cx, "feature:Extrude 1");
            let rows = l.menu(cx);
            let after = rows.iter().position(|r| r == "Add selection to folder…").and_then(|k| rows.get(k + 1)).cloned();
            expect((after.as_deref(), l.selected(cx)), (Some("Section view…"), Vec::new()), &format!("after Sketch 2's menu: rows {rows:?}"))?;
            l.keys(cx, "escape");
            l.click(cx, "feature:Extrude 1", NONE);
            l.right_click(cx, "feature:Extrude 1");
            let rows = l.menu(cx);
            expect(rows.iter().any(|r| r == "Section view…"), false, &format!("after a click: rows {rows:?}"))
        } },
        Row { id: "menu / a feature and a part, on the part", live: "the one-part menu without Copy Part 2", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "part:Part 2", NONE);
            l.right_click(cx, "part:Part 2");
            expect(l.menu(cx), names(&["Rename", "Properties…", "Assign material…", "Edit appearance…", "Copy here…", "Create Drawing of Part 2…", "Export…", "Hide", "Hide other parts", "Hide all parts", "Isolate…", "Make transparent…", "Zoom to selection", "Delete…"]), "rows")
        } },
        Row { id: "menu / a feature and a default plane, on the feature", live: "Size to Part Studio | Hide other planes | Hide all planes | Section view… | Clear selection | Add appearance to feature… (no Delete; live's document had no Plane feature: its plane is Top)", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Top", NONE);
            l.right_click(cx, "feature:Extrude 1");
            expect(l.menu(cx), names(&["Size to Part Studio", "Hide other planes", "Hide all planes", "Section view…", "Clear selection", "Add appearance to feature…"]), "rows")
        } },
        Row { id: "menu / one part", live: "Rename | Properties… | Assign material… | Edit appearance… | Copy here… | Copy Part 1 | Create Drawing of Part 1… | Export… | Where used… | Hide | Hide other parts | Hide all parts | Isolate… | Make transparent… | Zoom to selection | Delete…", gap: false, check: |l, cx| {
            l.right_click(cx, "part:Part 1");
            expect(l.menu(cx), names(&["Rename", "Properties…", "Assign material…", "Edit appearance…", "Copy here…", "Copy Part 1", "Create Drawing of Part 1…", "Export…", "Where used…", "Hide", "Hide other parts", "Hide all parts", "Isolate…", "Make transparent…", "Zoom to selection", "Delete…"]), "rows")
        } },
        Row { id: "menu / two parts", live: "Properties… | Assign material for 2 parts… | Edit appearance for 2 parts… | Check interference for 2 parts… | Boolean | Copy 2 items | Hide | Hide other parts | Hide all parts | Isolate… | Make transparent… | Export… | Delete", gap: false, check: |l, cx| {
            l.click(cx, "part:Part 1", NONE);
            l.click(cx, "part:Part 2", NONE);
            l.right_click(cx, "part:Part 2");
            expect(l.menu(cx), names(&["Properties…", "Assign material for 2 parts…", "Edit appearance for 2 parts…", "Check interference for 2 parts…", "Boolean", "Copy 2 items", "Hide", "Hide other parts", "Hide all parts", "Isolate…", "Make transparent…", "Export…", "Delete"]), "rows")
        } },
        Row { id: "parts / the Parts header's menu shows and hides the parts list properties", live: "right-clicking 'Parts (n)': one item, 'Show parts list properties'; on, each part row shows its state chip 'In progress' (#eeeeee, 20 px high, radius 3) after its name; the item then reads 'Hide parts list properties' (live rn-1222-1223 pl1 / pl2.json, Onshape 1.222)", gap: false, check: |l, cx| {
            let chip = |l: &List, cx: &mut TestAppContext| {
                let part = cx.update(|cx| l.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.id.clone()))?;
                l.bounds(cx, &format!("part-state-{part}")).map(|b| f32::from(b.size.height).round())
            };
            let before = chip(l, cx);
            l.right_click(cx, "section:parts");
            let off = l.menu(cx);
            let at = l.bounds(cx, "menu:Show parts list properties").ok_or("no Show parts list properties row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            let on = chip(l, cx);
            l.right_click(cx, "section:parts");
            let menu_on = l.menu(cx);
            let at = l.bounds(cx, "menu:Hide parts list properties").ok_or("no Hide parts list properties row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            expect((before, off, on, menu_on, chip(l, cx)), (None, names(&["Show parts list properties"]), Some(20.0), names(&["Hide parts list properties"]), None), "(chip before, the header's menu, chip height on, the menu on, chip after hiding)")
        } },
        Row { id: "menu / a hidden part", live: "… Show, Show all parts, Isolate… (no Hide other / all parts; live2 view-tools-1118)", gap: false, check: |l, cx| {
            let hidden = |cx: &gpui_kit::App| l.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.hidden);
            l.right_click(cx, "part:Part 1");
            let at = l.bounds(cx, "menu:Hide").ok_or("no Hide row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| hidden(cx) == Some(true));
            l.right_click(cx, "part:Part 1");
            let menu = l.menu(cx);
            let pos = |t: &str| menu.iter().position(|m| m == t);
            let ok = pos("Show").zip(pos("Show all parts")).zip(pos("Isolate…")).is_some_and(|((a, b), c)| b == a + 1 && c == b + 1) && pos("Hide other parts").is_none();
            expect(ok, true, &format!("rows {menu:?}"))
        } },
        Row { id: "menu / the Top plane row", live: "Rename | New sketch…, Offset plane…, Size to Part Studio | Hide, Hide other planes, Hide all planes | Section view… | Zoom to selection, View normal to | Show dependencies… (Add comment: cloud; live ctx-menus-1117 row-top-plane, qa MENU-TOP-ROW-SECTION-GROUP)", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Top");
            expect(l.menu_grouped(cx), names(&["Rename", "———", "New sketch…", "Offset plane…", "Size to Part Studio", "———", "Hide", "Hide other planes", "Hide all planes", "———", "Section view…", "———", "Zoom to selection", "View normal to", "———", "Show dependencies…"]), "rows")
        } },
        Row { id: "menu / the Top row's Section view… opens on Top", live: "a floating 'Section view' dialog, Exclude / Include, 'Section plane 1 (Top)' already in Section planes, Items to exclude empty; the arrow's shaft starts at its ring on the origin (live cov-1120 R9, sv1-open.png)", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Top");
            let at = l.bounds(cx, "menu:Section view…").ok_or("no Section view… row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            let got = cx.update(|cx| {
                let v = l.view.read(cx);
                (v.ui.section_dialog.as_ref().map(|d| d.items.len()), v.section_chips(), v.section_arrow().is_some_and(|a| a.length.abs() < 1e-5))
            });
            l.keys(cx, "escape");
            expect(got, (Some(0), vec![(1, "Top".to_string())], true), "(the dialog's items to exclude, its planes (number, name), the arrow's shaft from its base ring: qa FEEL-SECTION-ARROW-FOOT)")
        } },
        Row { id: "menu / the Origin row", live: "Rename | Hide | Show dependencies… (live ctx-menus-1117 row-origin)", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Origin");
            expect(l.menu(cx), names(&["Rename", "Hide", "Show dependencies…"]), "rows")
        } },
        Row { id: "menu / the rollback bar", live: "no menu", gap: false, check: |l, cx| {
            let at = l.bounds(cx, "rollback").ok_or("no rollback bar")?.center();
            l.press(cx, at, MouseButton::Right, NONE, 1);
            expect(l.menu(cx), names(&[]), "rows")
        } },
        Row { id: "drag / a row's top half drops before it", live: "Extrude 3 onto Sketch 2's top half: before Sketch 2; the selection as before (nothing: live 08:18, feature-row-drag-above)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Sketch 2", 6.));
            l.drag(cx, from, to, true);
            expect((l.order(cx), l.selected(cx)), (names(&["Sketch 1", "Extrude 1", "Extrude 3", "Sketch 2", "Extrude 2", "Plane 1", "Sketch 3"]), names(&[])), "order, selected")
        } },
        Row { id: "drag / a row's bottom half drops after it", live: "Extrude 3 onto Extrude 1's bottom half: after Extrude 1", gap: false, check: |l, cx| {
            let h = f32::from(l.bounds(cx, "feature:Extrude 1").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Extrude 1", h - 6.));
            l.drag(cx, from, to, true);
            expect(l.order(cx), names(&["Sketch 1", "Extrude 1", "Extrude 3", "Sketch 2", "Extrude 2", "Plane 1", "Sketch 3"]), "order")
        } },
        Row { id: "drag / carries the selection, in list order", live: "Plane 1 selected, Extrude 3 dragged before Sketch 2: Plane 1, Extrude 3 move together; Plane 1 stays selected", gap: false, check: |l, cx| {
            l.click(cx, "feature:Plane 1", NONE);
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Sketch 2", 6.));
            l.drag(cx, from, to, true);
            expect((l.order(cx), l.selected(cx)), (names(&["Sketch 1", "Extrude 1", "Plane 1", "Extrude 3", "Sketch 2", "Extrude 2", "Sketch 3"]), names(&["Plane 1"])), "order, selected")
        } },
        Row { id: "drag / no refusal for dependencies", live: "Extrude 3 dropped above Sketch 3 moves (then fails), no message", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Sketch 3", 6.));
            l.drag(cx, from, to, true);
            expect(l.order(cx), names(&["Sketch 1", "Extrude 1", "Sketch 2", "Extrude 2", "Plane 1", "Extrude 3", "Sketch 3"]), "order")
        } },
        Row { id: "drag / released over the canvas: cancelled", live: "order unchanged", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.canvas(cx));
            l.drag(cx, from, to, true);
            expect(l.order(cx), names(&FEATURES), "order")
        } },
        Row { id: "drag / off the list the line stays", live: "the drop line stays at its last spot while the pointer is off the list (drag_ghost_outside_list)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Sketch 2", 6.));
            l.drag(cx, from, to, false);
            let on = cx.update(|cx| l.view.read(cx).ui.feature_drop.get());
            let canvas = l.canvas(cx);
            l.event(cx, MouseMoveEvent { position: canvas, pressed_button: Some(MouseButton::Left), modifiers: NONE });
            draw(cx, l.window);
            let off = cx.update(|cx| l.view.read(cx).ui.feature_drop.get());
            l.keys(cx, "escape");
            l.release(cx, canvas);
            expect((on.is_some(), off == on), (true, true), "a slot over Sketch 2, the same one off the list")
        } },
        Row { id: "drag / Escape cancels", live: "order unchanged after Escape and a release on Sketch 2", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Sketch 2", 6.));
            l.drag(cx, from, to, false);
            l.keys(cx, "escape");
            l.release(cx, to);
            expect(l.order(cx), names(&FEATURES), "order")
        } },
        Row { id: "drag / default geometry can't be dragged", live: "pressing Top and moving 40 px starts no drag", gap: false, check: |l, cx| {
            let from = l.at(cx, "feature:Top");
            l.drag(cx, from, point(from.x, from.y + px(40.)), false);
            let d = l.dragging(cx);
            l.release(cx, from);
            expect(d, false, "dragging")
        } },
        Row { id: "drag / onto default geometry: nothing", live: "Extrude 3 released on Right: order unchanged", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at(cx, "feature:Right"));
            l.drag(cx, from, to, true);
            expect(l.order(cx), names(&FEATURES), "order")
        } },
        Row { id: "Parts list / parts can't be dragged", live: "no drag, order unchanged", gap: false, check: |l, cx| {
            let from = l.at(cx, "part:Part 1");
            l.drag(cx, from, point(from.x, from.y + px(40.)), false);
            let d = l.dragging(cx);
            l.release(cx, from);
            expect(d, false, "dragging")
        } },
        Row { id: "Parts list / Rename: whole name selected, Esc cancels, Enter commits", live: "row menu › Rename: an inline box with 'Part 2' all selected; Esc keeps the old name; 'Foo' + Enter renames (live2 parts-list-1118)", gap: false, check: |l, cx| {
            let part_rename = |l: &List, cx: &mut TestAppContext| {
                l.right_click(cx, "part:Part 2");
                let at = l.bounds(cx, "menu:Rename").expect("no Rename row").center();
                l.press(cx, at, MouseButton::Left, NONE, 1);
            };
            let parts = |cx: &mut TestAppContext| cx.update(|cx| l.view.read(cx).display.bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
            part_rename(l, cx);
            let input = cx.update(|cx| l.view.read(cx).ui.part_renaming.as_ref().map(|(_, st, _)| (st.read(cx).value().to_string(), st.read(cx).selected_range().len())));
            l.keys(cx, "x escape");
            let esc = (cx.update(|cx| l.view.read(cx).ui.part_renaming.is_some()), parts(cx));
            part_rename(l, cx);
            l.keys(cx, "F o o enter");
            wait_until(cx, l.window, |cx| l.view.read(cx).display.bodies.iter().any(|b| b.name == "Foo"));
            let entered = (cx.update(|cx| l.view.read(cx).ui.part_renaming.is_some()), parts(cx));
            let three = |b: &str| names(&["Part 1", b, "Part 3"]);
            expect((input, esc, entered), (Some(("Part 2".to_string(), 6)), (false, three("Part 2")), (false, three("Foo"))), "(input, after x + Esc, after Foo + Enter)")
        } },
        Row { id: "Parts list / a click away commits a rename", live: "'Bar' then a click in the graphics renames the part (live2 parts-list-1118)", gap: false, check: |l, cx| {
            l.right_click(cx, "part:Part 2");
            let at = l.bounds(cx, "menu:Rename").ok_or("no Rename row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            l.keys(cx, "B a r");
            let at = l.canvas(cx);
            l.press(cx, at, MouseButton::Left, NONE, 1);
            // The rename regenerates the studio before the list shows it (a loaded host: not within the press).
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.iter().any(|b| b.name == "Bar"));
            let parts = cx.update(|cx| l.view.read(cx).display.bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
            expect((cx.update(|cx| l.view.read(cx).ui.part_renaming.is_some()), parts), (false, names(&["Part 1", "Bar", "Part 3"])), "(still renaming, names)")
        } },
        Row { id: "Parts list / Shift+N renames a selected part, F2 nothing", live: "Shift+N on the selected Part 1 row opens the same box; F2 opens nothing (live2 parts-list-1118)", gap: false, check: |l, cx| {
            l.click(cx, "part:Part 1", NONE);
            l.keys(cx, "f2");
            let f2 = cx.update(|cx| l.view.read(cx).ui.part_renaming.is_some());
            l.keys(cx, "shift-n");
            let value = cx.update(|cx| l.view.read(cx).ui.part_renaming.as_ref().map(|(_, st, _)| st.read(cx).value().to_string()));
            expect((f2, value), (false, Some("Part 1".to_string())), "(renaming after F2, the box after Shift+N)")
        } },
        Row { id: "keys / Tab from the graphics: Measure, Analysis tools, Mass properties, then out; Shift+Tab back", live: "after a click on empty graphics: Tab -> Show measure details -> Show analysis tools -> Display mass and section properties -> out of the page (then Share); Shift+Tab walks back; no list row or toolbar button is a stop (live2 keys-1118 (2))", gap: false, check: |l, cx| {
            let at = |cx: &mut TestAppContext| {
                cx.update_window(l.window, |_, w, cx| {
                    let v = l.view.read(cx);
                    if v.focus.is_focused(w) {
                        return Some("graphics");
                    }
                    v.ui.eval_stops.iter().position(|h| h.is_focused(w)).map(|i| ["measure", "analysis", "mass"][i])
                })
                .unwrap()
            };
            let canvas = l.canvas(cx);
            l.press(cx, canvas, MouseButton::Left, NONE, 1);
            let start = at(cx);
            let mut forward = Vec::new();
            for _ in 0..4 {
                l.keys(cx, "tab");
                forward.push(at(cx));
            }
            l.press(cx, canvas, MouseButton::Left, NONE, 1);
            l.keys(cx, "tab tab tab");
            let mut back = Vec::new();
            for _ in 0..3 {
                l.keys(cx, "shift-tab");
                back.push(at(cx));
            }
            // Space on the focused Measure button pins the measure panel (GPUI clicks on the release); Enter likewise.
            l.keys(cx, "tab space");
            l.event(cx, gpui_kit::KeyUpEvent { keystroke: gpui_kit::Keystroke::parse("space").unwrap() });
            draw(cx, l.window);
            let pinned = cx.update(|cx| l.view.read(cx).ui.measure_pinned);
            // Enter on Mass properties opens it.
            l.keys(cx, "tab tab enter");
            l.event(cx, gpui_kit::KeyUpEvent { keystroke: gpui_kit::Keystroke::parse("enter").unwrap() });
            draw(cx, l.window);
            let mass = cx.update(|cx| l.view.read(cx).show_mass);
            let want_forward = vec![Some("measure"), Some("analysis"), Some("mass"), None];
            let want_back = vec![Some("analysis"), Some("measure"), Some("graphics")];
            expect((start, forward, back, pinned, mass), (Some("graphics"), want_forward, want_back, true, true), "(focus after the click, after each Tab, after each Shift+Tab from Mass, Measure pinned by Space, Mass open by Enter)")
        } },
        Row { id: "click / a face or edge in the graphics relates its feature and part rows", live: "the face's (edge's) feature row and its part's row related rgb(236,240,244), neither selected (live2 11-17 sel-feel)", gap: false, check: |l, cx| {
            let mut got = Vec::new();
            for edge in [false, true] {
                l.keys(cx, "space");
                cx.update_window(l.window, |_, w, cx| l.view.update(cx, |v, cx| {
                    let b = v.display.bodies.iter().find(|b| b.name == "Part 1").unwrap();
                    let r = if edge { b.edge_refs.first().cloned() } else { b.face_refs.first().cloned() };
                    let pick = r.and_then(|r| v.built.as_ref().and_then(|bt| bt.picks.ids_for(&r).first().copied()));
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: glam::DVec2::new(500., 300.), modifiers: NONE }, w, cx);
                }))
                .unwrap();
                draw(cx, l.window);
                got.push(cx.update(|cx| {
                    let v = l.view.read(cx);
                    let b = v.display.bodies.iter().find(|b| b.name == "Part 1").cloned().unwrap();
                    let mut related: Vec<String> = v.studio(cx).map(|s| s.features.iter().filter(|f| v.row_ctx.related.contains(&f.id)).map(|f| f.name.clone()).collect()).unwrap_or_default();
                    related.sort();
                    (v.selection.len(), v.selected_features.len(), related, v.part_row_flags(&b, cx))
                }));
            }
            let want = (1, 0, names(&["Extrude 1"]), (false, true));
            expect(got, vec![want.clone(), want], "(selected in the graphics, rows selected, rows related, Part 1's row (selected, related)) after a face's click, an edge's")
        } },
        Row { id: "Parts list / a dialog field's edge relates its part; a preview's new part bold", live: "workflow 21 (qa PARTS-ROW-OF-PICK, tickets/wf21-parts.png): the part of the face or edge in Helix's field related (#ECF0F4, a 1 px #90CEF1 line under it); the Revolve preview's new Part 1 bold", gap: false, check: |l, cx| {
            cx.update_window(l.window, |_, w, cx| l.view.update(cx, |v, cx| {
                v.selection.clear();
                v.start_feature("fillet", w, cx);
                let edge = v.display.bodies.iter().find(|b| b.name == "Part 1").and_then(|b| b.edge_refs.first().cloned());
                let d = v.edit.as_ref().expect("the Fillet dialog").dialog.clone();
                d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(edge.into_iter().collect()), cx));
            }))
            .unwrap();
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            draw(cx, l.window);
            let flags = |cx: &gpui_kit::App| {
                let v = l.view.read(cx);
                let mut rows: Vec<(String, (bool, bool), bool)> = v.display.bodies.iter().map(|b| (b.name.clone(), v.part_row_flags(b, cx), v.part_row_new(b, cx))).collect();
                rows.sort();
                rows
            };
            let fillet = cx.update(|cx| flags(cx).into_iter().filter(|r| r.1 != (false, false) || r.2).collect::<Vec<_>>());
            l.keys(cx, "escape");
            wait_until(cx, l.window, |cx| l.view.read(cx).edit.is_none() && !l.view.read(cx).ui_busy());
            cx.update_window(l.window, |_, w, cx| l.view.update(cx, |v, cx| {
                let sk = v.display.sketches.iter().find(|s| v.studio(cx).and_then(|st| st.feature(s.feature)).is_some_and(|f| f.name == "Sketch 1")).expect("Sketch 1");
                let region = Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() };
                v.start_feature("extrude", w, cx);
                let d = v.edit.as_ref().expect("the Extrude dialog").dialog.clone();
                d.update(cx, |d, cx| {
                    d.set_value("entities", noshape_model::ParamValue::Query(vec![region]), cx);
                    d.set_value("operationType", noshape_model::ParamValue::Enum("NEW".into()), cx);
                });
            }))
            .unwrap();
            wait_until(cx, l.window, |cx| {
                let v = l.view.read(cx);
                !v.ui_busy() && v.edit.as_ref().is_some_and(|e| v.display.bodies.iter().any(|b| b.id.feature == e.dialog.read(cx).feature.id))
            });
            draw(cx, l.window);
            let bold = cx.update(|cx| flags(cx).into_iter().filter(|r| r.2).count());
            l.keys(cx, "escape");
            expect((fillet, bold), (vec![("Part 1".to_string(), (false, true), false)], 1), "(the rows tinted with Fillet's edge picked: (name, (selected, related), bold), the Extrude preview's bold rows)")
        } },
        Row { id: "click / a sketch region relates only its sketch", live: "Sketch 1's row related, not the Extrude that used it nor the part on it; no row selected (live2 11-17 sel-feel)", gap: false, check: |l, cx| {
            l.keys(cx, "space");
            cx.update(|cx| l.view.update(cx, |v, cx| {
                let sk = v.display.sketches.iter().find(|s| v.studio(cx).and_then(|st| st.feature(s.feature)).is_some_and(|f| f.name == "Sketch 1")).unwrap();
                v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().unwrap().key.clone() }];
                v.refresh_scene(false, cx);
                cx.notify();
            }));
            draw(cx, l.window);
            let got = cx.update(|cx| {
                let v = l.view.read(cx);
                let b = v.display.bodies.iter().find(|b| b.name == "Part 1").cloned().unwrap();
                let related: Vec<String> = v.studio(cx).map(|s| s.features.iter().filter(|f| v.row_ctx.related.contains(&f.id)).map(|f| f.name.clone()).collect()).unwrap_or_default();
                (v.selected_features.len(), related, v.part_row_flags(&b, cx))
            });
            expect(got, (0, names(&["Sketch 1"]), (false, false)), "(rows selected, rows related, Part 1's row (selected, related))")
        } },
        Row { id: "Parts list / a part hovered in the graphics leaves its row as is", live: "Part 2 outlined in the graphics, its row gets no hover or related tint: the link runs list -> graphics only (live2 parts-list-1118 c1)", gap: false, check: |l, cx| {
            cx.update(|cx| {
                l.view.update(cx, |v, cx| {
                    let face = v.display.bodies.iter().find(|b| b.name == "Part 2").and_then(|b| b.face_refs.first().cloned());
                    v.hover = face;
                    v.refresh_scene(false, cx);
                })
            });
            draw(cx, l.window);
            let flags = cx.update(|cx| {
                let v = l.view.read(cx);
                let b = v.display.bodies.iter().find(|b| b.name == "Part 2").cloned();
                (v.hover.is_some(), b.map(|b| v.part_row_flags(&b, cx)))
            });
            expect(flags, (true, Some((false, false))), "(hovered, Part 2's row (selected, related))")
        } },
        Row { id: "sketching / later rows grey and the Parts list follows, ✗ brings them back", live: "Sketch 2 edited: Extrude 2 and the rows after it grey italic (not computed), Parts (2) -> Parts (1), Final offered; ✗ brings everything back (live2 sketch-entry-1118 e1)", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Sketch 2");
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).sketch.is_some());
            draw(cx, l.window);
            let greyed = |cx: &mut TestAppContext| cx.update(|cx| {
                let v = l.view.read(cx);
                v.studio(cx).map(|s| s.features.iter().enumerate().filter(|(i, _)| *i >= v.row_ctx.rollback).map(|(_, f)| f.name.clone()).collect::<Vec<_>>()).unwrap_or_default()
            });
            let parts = |cx: &mut TestAppContext| cx.update(|cx| l.view.read(cx).display.bodies.len());
            let offered = cx.update(|cx| l.view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).final_available));
            let editing = (l.editing(cx), greyed(cx), parts(cx), offered);
            cx.update_window(l.window, |_, w, cx| l.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.len() == 3);
            draw(cx, l.window);
            let after = (l.editing(cx), greyed(cx), parts(cx));
            expect((editing, after), ((Some("Sketch 2".to_string()), names(&["Extrude 2", "Plane 1", "Sketch 3", "Extrude 3"]), 1, Some(true)), (None, names(&[]), 3)), "(editing, grey rows, parts, Final offered) while Sketch 2 is edited, after ✗")
        } },
        Row { id: "sketching / another row clicked does nothing", live: "Extrude 1 clicked while Sketch 2 is edited: the sketch stays open, nothing selected, no dialog (live2 sketch-entry-1118)", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Sketch 2");
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).sketch.is_some());
            draw(cx, l.window);
            l.click(cx, "feature:Extrude 1", NONE);
            let after = (l.editing(cx), cx.update(|cx| l.view.read(cx).sketch.is_some()), l.selected(cx));
            expect(after, (Some("Sketch 2".to_string()), true, names(&[])), "(editing, sketch open, selected) after the click")
        } },
        Row { id: "keys / Delete on a used sketch: no dialog, its user fails", live: "Sketch 1 selected, Delete: gone at once, no dialog; Extrude 1 red, its tooltip 'Extrude 1 did not regenerate properly: Select face or sketch region to extrude.' then '1 missing selection' (live2 keys-1118)", gap: false, check: |l, cx| {
            l.click(cx, "feature:Sketch 1", NONE);
            l.keys(cx, "delete");
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).studio(cx).is_some_and(|s| s.features.iter().all(|f| f.name != "Sketch 1")));
            let dialog = (cx.update(|cx| l.view.read(cx).edit.is_some()), cx.update(|cx| l.view.read(cx).context_menu.is_some()));
            l.hover(cx, "feature:Extrude 1", 450);
            let tip = crate::ui_script::shown_tooltip();
            expect((dialog, tip), ((false, false), Some("Extrude 1 did not regenerate properly: Select face or sketch region to extrude.\n1 missing selection".to_string())), "((a dialog, a menu), Extrude 1's tooltip)")
        } },
        Row { id: "rollback bar / its whole row grabs", live: "a drag from the row's top edge (outside the 7 px bar) onto Extrude 1's bottom half rolls back to after Extrude 1", gap: false, check: |l, cx| {
            let h = f32::from(l.bounds(cx, "feature:Extrude 1").ok_or("no row")?.size.height);
            let (from, to) = (l.at_y(cx, "rollback", 2.), l.at_y(cx, "feature:Extrude 1", h - 6.));
            l.drag(cx, from, to, true);
            expect(l.rolled_to(cx), Some("Extrude 1".to_string()), "rolled to")
        } },
        Row { id: "rollback bar / a row's top half: before it", live: "the bar dropped on Extrude 2's top half rolls back to after Sketch 2", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 2", 6.));
            l.drag(cx, from, to, true);
            expect(l.rolled_to(cx), Some("Sketch 2".to_string()), "rolled to")
        } },
        Row { id: "rollback bar / the drag starts past 6 px", live: "3 px steps from the bar: no drop line at 3 or 6 px, a line at 9 (live2 ps-feel2-1118)", gap: false, check: |l, cx| {
            let from = l.at(cx, "rollback");
            l.event(cx, MouseMoveEvent { position: from, pressed_button: None, modifiers: NONE });
            l.event(cx, MouseDownEvent { button: MouseButton::Left, position: from, modifiers: NONE, click_count: 1, first_mouse: false });
            let mut got = Vec::new();
            for dy in [3., 6., 9.] {
                l.event(cx, MouseMoveEvent { position: point(from.x, from.y - px(dy)), pressed_button: Some(MouseButton::Left), modifiers: NONE });
                draw(cx, l.window);
                got.push(l.dragging(cx));
            }
            l.keys(cx, "escape");
            l.release(cx, point(from.x, from.y - px(9.)));
            expect(got, vec![false, false, true], "dragging at 3, 6, 9 px")
        } },
        Row { id: "drag / a feature row's drag starts past about 5.5 px", live: "the ghost and drop line at 6 px straight down and 7.1 px diagonally, none at 5 or 5.7 (live list-1119; ours started past 2)", gap: false, check: |l, cx| {
            let from = l.at(cx, "feature:Extrude 1");
            let mut got = Vec::new();
            for (dx, dy) in [(0., 5.), (0., 6.), (4., 4.), (5., 5.)] {
                l.event(cx, MouseMoveEvent { position: from, pressed_button: None, modifiers: NONE });
                l.event(cx, MouseDownEvent { button: MouseButton::Left, position: from, modifiers: NONE, click_count: 1, first_mouse: false });
                let to = point(from.x + px(dx), from.y + px(dy));
                l.event(cx, MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: NONE });
                draw(cx, l.window);
                got.push(l.dragging(cx));
                l.keys(cx, "escape");
                l.release(cx, to);
            }
            expect(got, vec![false, true, false, true], "dragging at 5 / 6 px down, 5.7 / 7.1 px diagonally")
        } },
        Row { id: "rollback bar / while held nothing moves or greys", live: "the 'Rollback bar' chip follows the pointer, the insertion line shows the slot, the row under it lit (live cov-1120 R8); the bar stays at its old place, nothing greys (live2 featurelist-feel 1)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 1", 6.));
            l.drag(cx, from, to, false);
            let filled = cx.update(|cx| {
                let v = l.view.read(cx);
                v.studio(cx).and_then(|s| s.features.iter().filter(|f| v.row_hover_filled(f.id, cx)).map(|f| f.name.clone()).next())
            });
            let held = (l.dragging(cx), cx.update(|cx| l.view.read(cx).ui.feature_drop.get().is_some()), l.rolled_to(cx), cx.update(|cx| l.view.read(cx).display.bodies.len()), l.lit_row(cx), filled);
            l.keys(cx, "escape");
            l.release(cx, to);
            expect(held, (true, true, Some("Extrude 3".to_string()), 3, Some("Extrude 1".to_string()), Some("Extrude 1".to_string())), "(dragging, a slot shown, rolled to, parts, the row's geometry lit under it, its fill: qa FEEL-ROLLBACK-DROP-ROW)")
        } },
        Row { id: "rollback bar / its tooltip 3 px under it, none on it after a drop", live: "'Rollback bar' 3 px under the 7 px bar (its centre ~20 px above the tip's); after a drop on a row no tooltip in 2.2 s, nor after a 1 px move, the row's hover band kept (live cov-1120 R8, rollback-drop-tip-1120; qa FEEL-ROLLBACK-DROP-ROW)", gap: false, check: |l, cx| {
            l.hover(cx, "rollback", 600);
            let shown = crate::ui_script::shown_tooltip();
            let bar = l.bounds(cx, "rollback").ok_or("no bar")?.center().y;
            let tip = l.bounds(cx, "placed-tooltip").ok_or("no tooltip")?.origin.y;
            let gap = (f32::from(tip - bar) - 6.5).abs() < 1.;
            let h = f32::from(l.bounds(cx, "feature:Extrude 1").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 1", h - 6.));
            l.drag(cx, from, to, true);
            crate::ui_script::clear_tooltip();
            l.wait(cx, 800);
            let after = crate::ui_script::shown_tooltip();
            let h = f32::from(l.bounds(cx, "feature:Extrude 3").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 3", h - 6.));
            l.drag(cx, from, to, true);
            expect((shown.as_deref(), gap, after), (Some("Rollback bar"), true, None), "(tooltip on hover, 6.5 px under the bar's centre, any tooltip after a drop on a row)")
        } },
        Row { id: "rollback bar / held over Top: Top lit with its eye; no tooltip after the drop", live: "the bar dragged onto Top: Top's row lit (its hover band, its eye) and the plane, not the row under the clamped drop line; dropped there, no 'Top' tooltip (live sel-gestures-1122 bar-over-top-held, bar-released-over-top; qa FEEL-ROLLBACK-CLAMP-HOVER)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "rollback"), l.at(cx, "feature:Top"));
            l.drag(cx, from, to, false);
            let held = cx.update(|cx| {
                let v = l.view.read(cx);
                (v.hover.clone(), v.ui.list_hover, v.ui.rollback_drop_dg.clone())
            });
            l.release(cx, to);
            crate::ui_script::clear_tooltip();
            l.wait(cx, 800);
            let after = crate::ui_script::shown_tooltip();
            let h = f32::from(l.bounds(cx, "feature:Extrude 3").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 3", h - 6.));
            l.drag(cx, from, to, true);
            let top = Some(Reference::Plane(noshape_model::reference::DatumPlane::Top));
            expect((held, after), ((top.clone(), None, top), None), "((the hover, the feature row lit, the default row under the bar) held over Top, any tooltip after the drop)")
        } },
        Row { id: "panel / the list toggle: its tooltip, the model kept centred", live: "the toggle's tooltip 'Features and parts', collapsed or not; collapsing widens the canvas, the model moves half the change, its scale kept (live fc-1120 (9), panel-resize.json: canvas 1673 -> 1879 px, centre x 1159 -> 1052)", gap: false, check: |l, cx| {
            let tip = |l: &List, cx: &mut TestAppContext| -> Result<Option<String>, String> {
                let at = l.bounds(cx, "list-toggle").ok_or("no toggle")?.center();
                let away = l.canvas(cx);
                l.event(cx, MouseMoveEvent { position: away, pressed_button: None, modifiers: NONE });
                draw(cx, l.window);
                crate::ui_script::clear_tooltip();
                l.event(cx, MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
                l.wait(cx, 1500);
                Ok(crate::ui_script::shown_tooltip())
            };
            let seen = |l: &List, cx: &mut TestAppContext| cx.update(|cx| {
                let vp = l.view.read(cx).viewport.read(cx);
                let (o, w) = (vp.project(glam::DVec3::ZERO), vp.size().x);
                let s = vp.project(glam::DVec3::new(0.01, 0.0, 0.0)).zip(o).map(|(a, b)| (a - b).length());
                (o.map(|o| o.x), w, s)
            });
            let open = tip(l, cx)?;
            let before = seen(l, cx);
            let at = l.bounds(cx, "list-toggle").ok_or("no toggle")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            let collapsed = tip(l, cx)?;
            let after = seen(l, cx);
            let at = l.bounds(cx, "list-toggle").ok_or("no toggle")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            let ((Some(x0), w0, Some(s0)), (Some(x1), w1, Some(s1))) = (before, after) else { return Err("the origin off screen".into()) };
            // In the canvas's own px (its left edge moved left by the change): the model, kept centred, half of it right.
            let (moved, half) = (x1 - x0, (w1 - w0) / 2.0);
            let centred = w1 > w0 + 100.0 && (moved - half).abs() < 1.0 && (s1 - s0).abs() < 1e-6;
            expect((open.as_deref(), collapsed.as_deref(), centred), (Some("Features and parts"), Some("Features and parts"), true), &format!("(tooltips open / collapsed, centred: width {w0:.0} -> {w1:.0}, origin x {x0:.1} -> {x1:.1}, 1 cm {s0:.2} -> {s1:.2} px)"))
        } },
        Row { id: "rollback bar / on release the rows below grey and the Parts list follows", live: "the bar above Extrude 1: Parts (1) -> Parts (0) here (every part's extrude rolled back); back at the end all three again", gap: false, check: |l, cx| {
            let h = f32::from(l.bounds(cx, "feature:Sketch 1").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Sketch 1", h - 6.));
            l.drag(cx, from, to, true);
            wait_until(cx, l.window, |cx| l.view.read(cx).display.bodies.is_empty());
            let rolled = (l.rolled_to(cx), cx.update(|cx| l.view.read(cx).display.bodies.len()));
            let h = f32::from(l.bounds(cx, "feature:Extrude 3").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 3", h - 6.));
            l.drag(cx, from, to, true);
            wait_until(cx, l.window, |cx| l.view.read(cx).display.bodies.len() == 3);
            let back = (l.rolled_to(cx), cx.update(|cx| l.view.read(cx).display.bodies.len()));
            expect((rolled, back), ((Some("Sketch 1".to_string()), 0), (Some("Extrude 3".to_string()), 3)), "(rolled to, parts) after Sketch 1, back at the end")
        } },
        Row { id: "rollback bar / above the first feature", live: "the bar onto Sketch 1's top half: every feature rolled back, Parts (0)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Sketch 1", 6.));
            l.drag(cx, from, to, true);
            wait_until(cx, l.window, |cx| l.view.read(cx).display.bodies.is_empty());
            expect((l.rolled_to(cx), cx.update(|cx| l.view.read(cx).display.bodies.len())), (None, 0), "(rolled to, parts)")
        } },
        Row { id: "rollback bar / onto default geometry: the top", live: "released over Top it lands above the first feature, everything rolled back (live2 rb-followup 11-18)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "rollback"), l.at(cx, "feature:Top"));
            l.drag(cx, from, to, true);
            expect(l.rolled_to(cx), None, "rolled to")
        } },
        Row { id: "rollback bar / its menu while rolled back: Roll to end", live: "a right-click on a rolled-back bar: 'Roll to end' alone, which rolls to the end (live2 doc-level-1117 3)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "rollback"), l.at_y(cx, "feature:Extrude 2", 6.));
            l.drag(cx, from, to, true);
            let at = l.bounds(cx, "rollback").ok_or("no rollback bar")?.center();
            l.press(cx, at, MouseButton::Right, NONE, 1);
            let menu = l.menu(cx);
            let roll = l.bounds(cx, "menu:Roll to end").ok_or("no Roll to end row")?.center();
            l.press(cx, roll, MouseButton::Left, NONE, 1);
            expect((menu, l.rolled_to(cx)), (names(&["Roll to end"]), Some("Extrude 3".to_string())), "(the menu, rolled to after it)")
        } },
        Row { id: "drag / a reorder the history can't take: red, no message", live: "Fillet 1 above its Extrude: the row moves, then reads red rgb(170,37,45) with a warning triangle; no bubble, no refusal (live2 featurelist-feel 2)", gap: false, check: |l, cx| {
            let (from, to) = (l.at(cx, "feature:Extrude 3"), l.at_y(cx, "feature:Sketch 3", 6.));
            l.drag(cx, from, to, true);
            let failed = |cx: &gpui_kit::App| {
                let v = l.view.read(cx);
                let id = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.name == "Extrude 3").map(|f| f.id));
                id.and_then(|id| v.display.feature_state(id)).map(|s| s.status)
            };
            wait_until(cx, l.window, |cx| failed(cx).is_some_and(|s| s != noshape_model::display::FeatureStatus::Ok));
            let status = cx.update(|cx| failed(cx));
            expect((l.order(cx)[5].clone(), status.is_some_and(|s| matches!(s, noshape_model::display::FeatureStatus::Error | noshape_model::display::FeatureStatus::Warning))), ("Extrude 3".to_string(), true), "(the moved row, failing)")
        } },
        Row { id: "rename / Enter commits", live: "'Foo' + Enter renames the feature", gap: false, check: |l, cx| {
            rename(l, cx, "Extrude 1");
            l.keys(cx, "F o o enter");
            expect((cx.update(|cx| l.view.read(cx).renaming.is_some()), l.order(cx)[1].clone()), (false, "Foo".to_string()), "(still renaming, name)")
        } },
        Row { id: "rename / a click elsewhere commits", live: "'Bar' then a click in the graphics renames the feature", gap: false, check: |l, cx| {
            rename(l, cx, "Extrude 1");
            l.keys(cx, "B a r");
            let at = l.canvas(cx);
            l.press(cx, at, MouseButton::Left, NONE, 1);
            expect((cx.update(|cx| l.view.read(cx).renaming.is_some()), l.order(cx)[1].clone()), (false, "Bar".to_string()), "(still renaming, name)")
        } },
        Row { id: "keys / F2 on a selected row: nothing", live: "no rename input opens (rename is in the menu only)", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.keys(cx, "f2");
            expect(cx.update(|cx| l.view.read(cx).renaming.is_some()), false, "renaming")
        } },
        Row { id: "hover / a renamed feature's tooltip has its type", live: "'Bar [Extrude] <- Sketch 1'", gap: false, check: |l, cx| {
            rename(l, cx, "Extrude 1");
            l.keys(cx, "B a r enter");
            l.hover(cx, "feature:Bar", 450);
            expect(crate::ui_script::shown_tooltip(), Some("Bar [Extrude] <- Sketch 1".to_string()), "tooltip")
        } },
        Row { id: "suppress / from the menu, then Unsuppress", live: "Suppress: the row grey italic and struck through, its part gone; its menu then has Unsuppress, which restores it", gap: false, check: |l, cx| {
            let suppressed = |cx: &gpui_kit::App| l.view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.name == "Extrude 1").map(|f| f.suppressed));
            l.right_click(cx, "feature:Extrude 1");
            let at = l.bounds(cx, "menu:Suppress").ok_or("no Suppress row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.len() == 2);
            let off = (cx.update(|cx| suppressed(cx)), cx.update(|cx| l.view.read(cx).display.bodies.len()));
            l.right_click(cx, "feature:Extrude 1");
            let at = l.bounds(cx, "menu:Unsuppress").ok_or("no Unsuppress row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.len() == 3);
            let on = (cx.update(|cx| suppressed(cx)), cx.update(|cx| l.view.read(cx).display.bodies.len()));
            expect((off, on), ((Some(true), 2), (Some(false), 3)), "(suppressed, parts) after Suppress, after Unsuppress")
        } },
        Row { id: "suppress / the parts left renumber, the consumed sketch drawn", live: "Extrude 2 suppressed: Parts (2) named Part 1, Part 2 (the old Part 3 now Part 2); Sketch 2 drawn (live2 ps-feel2-1118)", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Extrude 2");
            let at = l.bounds(cx, "menu:Suppress").ok_or("no Suppress row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.len() == 2);
            let got = cx.update(|cx| {
                let v = l.view.read(cx);
                let mut parts: Vec<String> = v.display.bodies.iter().map(|b| b.name.clone()).collect();
                parts.sort();
                let sketch2 = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.name == "Sketch 2").map(|f| f.id));
                let drawn = v.display.sketches.iter().find(|s| Some(s.feature) == sketch2).map(|s| !s.hidden);
                (parts, drawn)
            });
            expect(got, (names(&["Part 1", "Part 2"]), Some(true)), "(parts, Sketch 2 drawn)")
        } },
        Row { id: "suppress / two features from one menu: one undo step", live: "Extrude 1 and Extrude 3 selected, Suppress: both struck through, Parts (1); Ctrl+Z restores both (live2 ps-feel2-1118)", gap: false, check: |l, cx| {
            let suppressed = |cx: &mut TestAppContext| cx.update(|cx| l.view.read(cx).studio(cx).map(|s| s.features.iter().filter(|f| f.suppressed).map(|f| f.name.clone()).collect::<Vec<_>>()).unwrap_or_default());
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "feature:Extrude 3", NONE);
            l.right_click(cx, "feature:Extrude 1");
            let at = l.bounds(cx, "menu:Suppress").ok_or("no Suppress row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.len() == 1);
            let off = suppressed(cx);
            l.undo(cx);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy() && l.view.read(cx).display.bodies.len() == 3);
            expect((off, suppressed(cx)), (names(&["Extrude 1", "Extrude 3"]), Vec::new()), "(suppressed after the menu, after one Ctrl+Z)")
        } },
        Row { id: "double-click / the dialog right of the list, 220 px wide", live: "Extrude 1's dialog at (246, 79) 220 x 352, right of the list (live2 featurelist-feel 3)", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Extrude 1");
            let d = l.bounds(cx, "feature-dialog").ok_or("no dialog")?;
            let row = l.bounds(cx, "feature:Extrude 1").ok_or("no row")?;
            let (w, left_of) = (f32::from(d.size.width).round(), d.origin.x > row.origin.x + row.size.width);
            expect((w, left_of), (220., true), "(width, right of the list)")
        } },
        Row { id: "undo / a rename is one step, no toast", live: "Fin -> Fin2 then Ctrl+Z: Fin again; nothing but the change and Undo / Redo's state (live2 doc-level-1117 2)", gap: false, check: |l, cx| {
            rename(l, cx, "Extrude 1");
            l.keys(cx, "F o o enter");
            let can = cx.update(|cx| l.view.read(cx).doc.read(cx).can_undo());
            // (Ctrl+Z goes up to the workspace's Undo, not in this window.)
            l.undo(cx);
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            expect((can, l.order(cx)[1].clone(), crate::ui_script::noted_message()), (true, "Extrude 1".to_string(), None), "(Undo on after the rename, the name after Ctrl+Z, a message)")
        } },
        Row { id: "undo / hiding a part is one step", live: "context-menu Hide, then Ctrl+Z shows it again", gap: false, check: |l, cx| {
            let hidden = |cx: &gpui_kit::App| l.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.hidden);
            l.right_click(cx, "part:Part 1");
            let at = l.bounds(cx, "menu:Hide").ok_or("no Hide row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            wait_until(cx, l.window, |cx| hidden(cx) == Some(true));
            l.undo(cx);
            wait_until(cx, l.window, |cx| hidden(cx) == Some(false));
            expect(cx.update(|cx| hidden(cx)), Some(false), "Part 1 hidden after Ctrl+Z")
        } },
        Row { id: "undo / a view change or a selection is no step", live: "Shift+5 and a row click: Undo stays off", gap: false, check: |l, cx| {
            let before = cx.update(|cx| l.view.read(cx).doc.read(cx).can_undo());
            l.click(cx, "feature:Extrude 1", NONE);
            let at = l.canvas(cx);
            l.event(cx, MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
            l.keys(cx, "shift-5");
            l.wait(cx, 600);
            expect((before, cx.update(|cx| l.view.read(cx).doc.read(cx).can_undo())), (false, false), "(Undo before, after a selection and Shift+5)")
        } },
        Row { id: "hover / the row's tooltip", live: "'<name> <- <consumed features>': 'Extrude 1 <- Sketch 1'", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 450);
            expect(crate::ui_script::shown_tooltip(), Some("Extrude 1 <- Sketch 1".to_string()), "tooltip")
        } },
        Row { id: "hover / a row clicked into a dialog's field keeps its tooltip", live: "workflow 07 step 3 (qa FEEL-ROW-DEPENDENCY-TIP-IN-DIALOG): Linear pattern of features, a click on Extrude 2's row fills Features to pattern; the pointer left on it, 'Extrude 2 <- Sketch 2' beside the row 4 s later", gap: false, check: |l, cx| {
            cx.update_window(l.window, |_, w, cx| l.view.update(cx, |v, cx| {
                v.selection.clear();
                v.start_feature("linearPattern", w, cx);
                let d = v.edit.as_ref().expect("the Linear pattern dialog").dialog.clone();
                d.update(cx, |d, cx| d.set_value("patternType", noshape_model::ParamValue::Enum("FEATURE".into()), cx));
            }))
            .unwrap();
            wait_until(cx, l.window, |cx| !l.view.read(cx).ui_busy());
            draw(cx, l.window);
            l.click(cx, "feature:Extrude 1", NONE);
            crate::ui_script::clear_tooltip();
            l.wait(cx, 4500);
            let picked = cx.update(|cx| l.view.read(cx).edit.as_ref().map(|e| format!("{:?}", e.dialog.read(cx).feature.params.get("instanceFunction"))));
            let tip = crate::ui_script::shown_tooltip();
            l.keys(cx, "escape");
            wait_until(cx, l.window, |cx| l.view.read(cx).edit.is_none() && !l.view.read(cx).ui_busy());
            expect(tip, Some("Extrude 1 <- Sketch 1".to_string()), &format!("tooltip (Features to pattern: {picked:?})"))
        } },
        Row { id: "hover / a feature row's tooltip 3 px right of the row, level with it", live: "live err-tip-placement-1123: the row 40, 352, 200 × 26, its tip 243, 352, 26 high; it stays there as the pointer moves (qa ERR-CHROME-TIP-PLACEMENT: ours ~5 px right, 7 px lower)", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 450);
            let row = l.bounds(cx, "feature:Extrude 1").ok_or("no Extrude 1 row")?;
            let tip = l.bounds(cx, "placed-tooltip").ok_or("no tooltip box")?;
            let (dx, dy) = (f32::from(tip.left() - row.right()), f32::from(tip.top() - row.top()));
            expect(((dx - 3.).abs() <= 1., dy.abs() <= 1.), (true, true), &format!("(3 px right, level: ours {dx} right, {dy} lower; row {row:?})"))
        } },
        Row { id: "hover / a Parts row's tooltip right of the row", live: "'Part 1' beside the list, right of the row and centred on it, as a feature row's (live list-1119 4; ours showed under the pointer)", gap: false, check: |l, cx| {
            l.hover(cx, "part:Part 1", 450);
            let row = l.bounds(cx, "part:Part 1").ok_or("no Part 1 row")?;
            let tip = l.bounds(cx, "placed-tooltip").ok_or("no tooltip box")?;
            let beside = tip.origin.x >= row.origin.x + row.size.width && (tip.center().y - row.center().y).abs() < px(2.);
            expect((crate::ui_script::shown_tooltip(), beside), (Some("Part 1".to_string()), true), "(the tooltip, right of the row and centred on it)")
        } },
        Row { id: "menu / a Parts row's eye goes while its menu is open", live: "live2 nav-feel-1119: the right-clicked Parts row shows no eye while its menu is open; back after", gap: false, check: |l, cx| {
            let id = cx.update(|cx| l.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.id.clone())).ok_or("no Part 1")?;
            let eye = format!("part-eye-{id}");
            l.right_click(cx, "part:Part 1");
            let open = l.bounds(cx, &eye).is_some();
            l.keys(cx, "escape");
            let after = l.bounds(cx, &eye).is_some();
            expect((open, after), (false, true), "(the eye drawn while the menu is open, after it closes)")
        } },
        Row { id: "hover / the tooltip from 80 ms", live: "fades in from 65–100 ms, opaque at 166–209 ms (live list-1118, 3 runs; ours: shown at 80 ms, a 130 ms fade)", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 50);
            let early = crate::ui_script::shown_tooltip();
            l.wait(cx, 70);
            expect((early, crate::ui_script::shown_tooltip().is_some()), (None, true), "at 50 ms, at 120 ms")
        } },
        Row { id: "hover / the geometry lit 100 ms after, cleared 100 ms after leaving", live: "featureHoverIn / featureHoverOut debounced 100 ms", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 60);
            let early = l.lit_row(cx);
            l.wait(cx, 60);
            let lit = l.lit_row(cx);
            let away = l.canvas(cx);
            l.event(cx, MouseMoveEvent { position: away, pressed_button: None, modifiers: NONE });
            l.wait(cx, 60);
            let still = l.lit_row(cx);
            l.wait(cx, 60);
            expect((early, lit, still, l.lit_row(cx)), (None, Some("Extrude 1".to_string()), Some("Extrude 1".to_string()), None), "at 60 / 120 ms in, 60 / 120 ms out")
        } },
        Row { id: "hover / a hidden sketch: drawn in place, lit", live: "Sketch 1 (hidden, consumed) drawn light orange where it is while its row is hovered, gone after (live2 11-17 sel-feel 4)", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Sketch 1", 150);
            let lit = l.drawn_sketches(cx);
            let away = l.canvas(cx);
            l.event(cx, MouseMoveEvent { position: away, pressed_button: None, modifiers: NONE });
            l.wait(cx, 150);
            expect((lit, l.drawn_sketches(cx)), ((names(&["Sketch 1"]), true), (Vec::new(), false)), "drawn (and lit) while hovered, after leaving")
        } },
        Row { id: "click / a default plane: Default geometry related", live: "the plane's row selected, the Default geometry folder related; Space clears both (live2 11-17 sel-feel 2, 5)", gap: false, check: |l, cx| {
            l.click(cx, "feature:Top", NONE);
            let related = cx.update(|cx| l.view.read(cx).default_geometry_related());
            l.keys(cx, "space");
            expect((related, cx.update(|cx| l.view.read(cx).default_geometry_related())), (true, false), "after the plane's click, after Space clears")
        } },
        Row { id: "hover / a sweep across rows lights only where it rests", live: "the debounce: rows crossed in under 100 ms never light", gap: false, check: |l, cx| {
            let mut seen = Vec::new();
            l.hover(cx, "feature:Sketch 1", 40);
            for row in ["feature:Extrude 1", "feature:Sketch 2"] {
                seen.extend(l.lit_row(cx));
                let at = l.at(cx, row);
                l.event(cx, MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
                l.wait(cx, 40);
            }
            seen.extend(l.lit_row(cx));
            l.wait(cx, 100);
            expect((seen, l.lit_row(cx)), (Vec::new(), Some("Sketch 2".to_string())), "lit while sweeping, at rest")
        } },
        Row { id: "folders / New folder with nothing selected", live: "the 'Folder name' popover; Enter makes an empty 'New folder (0)' just above the rollback bar, its caret shown collapsed (live featurelist-gaps 11-16)", gap: false, check: |l, cx| {
            l.new_folder(cx, true);
            let after = cx.update(|cx| l.view.read(cx).studio(cx).and_then(|s| s.folders.first().and_then(|fd| fd.after).and_then(|a| s.feature(a)).map(|f| f.name.clone())));
            expect((l.folders(cx), after), (vec![("New folder".to_string(), Vec::new(), true)], Some("Extrude 3".to_string())), "(folders (name, features, collapsed), listed after)")
        } },
        Row { id: "folders / a selected folder's menu starts with the sketch hide items", live: "a new folder over Sketch 1 + Extrude 1: right-clicked unselected, Section view… first (ui-parity folder-rclick-menu); selected, Hide other sketches, Hide all sketches, then Section view… (live list-1119 p6)", gap: false, check: |l, cx| {
            l.click(cx, "feature:Sketch 1", NONE);
            l.click(cx, "feature:Extrude 1", SHIFT);
            l.new_folder(cx, true);
            l.right_click(cx, "folder:New folder");
            let unselected: Vec<String> = l.menu(cx).into_iter().take(1).collect();
            l.keys(cx, "escape");
            l.click(cx, "folder:New folder", NONE);
            l.right_click(cx, "folder:New folder");
            let selected: Vec<String> = l.menu(cx).into_iter().take(3).collect();
            l.keys(cx, "escape");
            expect((unselected, selected), (names(&["Section view…"]), names(&["Hide other sketches", "Hide all sketches", "Section view…"])), "(the unselected folder's first row, the selected folder's first three)")
        } },
        Row { id: "folders / selected features and a part", live: "the features only (Extrude 1 + Part 2: Extrude 1), collapsed", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.click(cx, "part:Part 2", NONE);
            l.new_folder(cx, true);
            expect(l.folders(cx), vec![("New folder".to_string(), names(&["Extrude 1"]), true)], "folders")
        } },
        Row { id: "folders / only default geometry selected", live: "nothing: no popover, no message", gap: false, check: |l, cx| {
            l.click(cx, "feature:Top", NONE);
            l.new_folder(cx, false);
            expect((cx.update(|cx| l.view.read(cx).folder_prompt.is_some()), l.folders(cx)), (false, Vec::new()), "popover, folders")
        } },
        Row { id: "folders / Escape in the popover", live: "no folder", gap: false, check: |l, cx| {
            l.click(cx, "feature:Extrude 1", NONE);
            l.new_folder(cx, false);
            l.keys(cx, "escape");
            expect((cx.update(|cx| l.view.read(cx).folder_prompt.is_some()), l.folders(cx)), (false, Vec::new()), "popover, folders")
        } },
        Row { id: "folders / onto a collapsed folder's middle third", live: "inside, at the folder's end; it stays collapsed", gap: false, check: |l, cx| {
            folder_of_last_part(l, cx);
            let h = f32::from(l.bounds(cx, "folder:New folder").ok_or("no folder row")?.size.height);
            let (from, to) = (l.at(cx, "feature:Extrude 1"), l.at_y(cx, "folder:New folder", h * 0.5));
            l.drag(cx, from, to, true);
            expect((l.order(cx), l.folders(cx)), (names(&["Sketch 1", "Sketch 2", "Extrude 2", "Plane 1", "Sketch 3", "Extrude 3", "Extrude 1"]), vec![("New folder".to_string(), names(&["Sketch 3", "Extrude 3", "Extrude 1"]), true)]), "order, folders")
        } },
        Row { id: "folders / a collapsed folder's top and bottom thirds", live: "before / after the folder, outside it", gap: false, check: |l, cx| {
            folder_of_last_part(l, cx);
            let h = f32::from(l.bounds(cx, "folder:New folder").ok_or("no folder row")?.size.height);
            let (from, to) = (l.at(cx, "feature:Extrude 1"), l.at_y(cx, "folder:New folder", h * 0.15));
            l.drag(cx, from, to, true);
            let before = (l.order(cx), l.folders(cx));
            let (from, to) = (l.at(cx, "feature:Extrude 2"), l.at_y(cx, "folder:New folder", h * 0.85));
            l.drag(cx, from, to, true);
            let inside = names(&["Sketch 3", "Extrude 3"]);
            expect(
                (before, (l.order(cx), l.folders(cx))),
                (
                    (names(&["Sketch 1", "Sketch 2", "Extrude 2", "Plane 1", "Extrude 1", "Sketch 3", "Extrude 3"]), vec![("New folder".to_string(), inside.clone(), true)]),
                    (names(&["Sketch 1", "Sketch 2", "Plane 1", "Extrude 1", "Sketch 3", "Extrude 3", "Extrude 2"]), vec![("New folder".to_string(), inside, true)]),
                ),
                "top third, then bottom third",
            )
        } },
        Row { id: "folders / an expanded folder's halves", live: "top half: before the folder; bottom half: inside it, as its first child", gap: false, check: |l, cx| {
            folder_of_last_part(l, cx);
            cx.update(|cx| l.view.update(cx, |v, cx| {
                let id = v.studio(cx).and_then(|s| s.folders.first().map(|f| f.id));
                if let Some(id) = id {
                    v.set_folder_collapsed(id, false, cx);
                }
            }));
            draw(cx, l.window);
            let h = f32::from(l.bounds(cx, "folder:New folder").ok_or("no folder row")?.size.height);
            let (from, to) = (l.at(cx, "feature:Extrude 1"), l.at_y(cx, "folder:New folder", h * 0.25));
            l.drag(cx, from, to, true);
            let top = (l.order(cx), l.folders(cx));
            let (from, to) = (l.at(cx, "feature:Extrude 2"), l.at_y(cx, "folder:New folder", h * 0.75));
            l.drag(cx, from, to, true);
            expect(
                (top, (l.order(cx), l.folders(cx))),
                (
                    (names(&["Sketch 1", "Sketch 2", "Extrude 2", "Plane 1", "Extrude 1", "Sketch 3", "Extrude 3"]), vec![("New folder".to_string(), names(&["Sketch 3", "Extrude 3"]), false)]),
                    (names(&["Sketch 1", "Sketch 2", "Plane 1", "Extrude 1", "Extrude 2", "Sketch 3", "Extrude 3"]), vec![("New folder".to_string(), names(&["Extrude 2", "Sketch 3", "Extrude 3"]), false)]),
                ),
                "top half, then bottom half",
            )
        } },
        Row { id: "folders / a drop between its children", live: "inside the folder (the insertion line 10 px in at depth 1)", gap: false, check: |l, cx| {
            expanded_folder_of_last_part(l, cx);
            let (from, to) = (l.at(cx, "feature:Extrude 1"), l.at_y(cx, "feature:Extrude 3", 6.));
            l.drag(cx, from, to, true);
            expect(l.folders(cx), vec![("New folder".to_string(), names(&["Sketch 3", "Extrude 1", "Extrude 3"]), false)], "folders")
        } },
        Row { id: "folders / sideways, 19 px a level", live: "on the last child's bottom half: inside; 19 px left: after the folder, outside it", gap: false, check: |l, cx| {
            expanded_folder_of_last_part(l, cx);
            let h = f32::from(l.bounds(cx, "feature:Extrude 3").ok_or("no row")?.size.height);
            let (from, to) = (l.at(cx, "feature:Extrude 1"), l.at_y(cx, "feature:Extrude 3", h * 0.75));
            l.drag(cx, from, to, true);
            let inside = l.folders(cx);
            // Onto the last child's bottom half, then 20 px left within it.
            let (from, to) = (l.at(cx, "feature:Extrude 2"), l.at_y(cx, "feature:Extrude 1", h * 0.75));
            l.drag(cx, from, to, false);
            for k in 1..=10 {
                l.event(cx, MouseMoveEvent { position: to - point(px(2. * k as f32), px(0.)), pressed_button: Some(MouseButton::Left), modifiers: NONE });
                draw(cx, l.window);
            }
            l.release(cx, to - point(px(20.), px(0.)));
            expect(
                (inside, l.folders(cx), l.order(cx)),
                (
                    vec![("New folder".to_string(), names(&["Sketch 3", "Extrude 3", "Extrude 1"]), false)],
                    vec![("New folder".to_string(), names(&["Sketch 3", "Extrude 3", "Extrude 1"]), false)],
                    names(&["Sketch 1", "Sketch 2", "Plane 1", "Sketch 3", "Extrude 3", "Extrude 1", "Extrude 2"]),
                ),
                "folders after the first drop; folders and order after the second",
            )
        } },
        Row { id: "folders / a folder dragged onto another", live: "its features go with it, into the other folder at its end (live2 11-14, feel/nest-drags-single.png)", gap: false, check: |l, cx| {
            nest(l, cx, &[("B", &["Sketch 2", "Extrude 2"], 1), ("A", &["Sketch 3", "Extrude 3"], 1)]);
            let (from, to) = (l.at(cx, "folder:A"), l.at(cx, "folder:B"));
            l.drag(cx, from, to, true);
            expect((l.parents(cx), l.order(cx)), (vec![("B".to_string(), None), ("A".to_string(), Some("B".to_string()))], names(&["Sketch 1", "Extrude 1", "Sketch 2", "Extrude 2", "Sketch 3", "Extrude 3", "Plane 1"])), "parents, order")
        } },
        Row { id: "folders / a drop past four levels does nothing", live: "a 4-deep chain onto another folder's middle: a red line, the drop does nothing, no message (live2 11-14)", gap: false, check: |l, cx| {
            nest(l, cx, &[("B", &["Sketch 2", "Extrude 2"], 1), ("A", &["Sketch 3", "Extrude 3"], 4)]);
            let before = (l.parents(cx), l.order(cx));
            let (from, to) = (l.at(cx, "folder:A"), l.at(cx, "folder:B"));
            l.drag(cx, from, to, true);
            expect((l.parents(cx), l.order(cx)), before, "parents, order")
        } },
        Row { id: "Parts list / click toggles, Shift range, Ctrl adds", live: "plain click toggles; Shift+click the range from the anchor; Ctrl+click adds like a plain click, no menu", gap: false, check: |l, cx| {
            l.click(cx, "part:Part 1", NONE);
            l.click(cx, "part:Part 3", SHIFT);
            let range = l.selected(cx);
            l.click(cx, "part:Part 2", NONE);
            let toggled = l.selected(cx);
            l.click(cx, "part:Part 2", CTRL);
            expect((range, toggled, l.selected(cx), l.menu(cx).is_empty()), (names(&["Part 1", "Part 2", "Part 3"]), names(&["Part 1", "Part 3"]), names(&["Part 1", "Part 2", "Part 3"]), true), "range, toggled, Ctrl, no menu")
        } },
        Row { id: "Parts list / Cmd toggles as Ctrl does", live: "list rows toggle on Ctrl or Cmd on every OS (metaKey || ctrlKey; live2 11-17 key mapping)", gap: false, check: |l, cx| {
            l.click(cx, "part:Part 1", CMD);
            let on = l.selected(cx);
            l.click(cx, "part:Part 1", CMD);
            expect((on, l.selected(cx), l.menu(cx).is_empty()), (names(&["Part 1"]), names(&[]), true), "after Cmd+click, again, no menu")
        } },
        Row { id: "Parts list / Shift with no part clicked before", live: "selects nothing (a fresh tab; live 08:18); a plain click selects", gap: false, check: |l, cx| {
            l.click(cx, "part:Part 2", SHIFT);
            expect(l.selected(cx), names(&[]), "selected")
        } },
        Row { id: "Parts list / Shift on a selected part deselects the range", live: "Parts 1, 2, 3 then Shift Part 1 leaves nothing", gap: false, check: |l, cx| {
            for p in ["part:Part 1", "part:Part 2", "part:Part 3"] {
                l.click(cx, p, NONE);
            }
            l.click(cx, "part:Part 1", SHIFT);
            expect(l.selected(cx), names(&[]), "selected")
        } },
        Row { id: "edit / a double-click keeps the row's name tooltip", live: "Extrude 1 double-clicked, the pointer still: its dialog opens and the row's tooltip 'Extrude 1' shows by the pointer (live workflow 10 step 1, qa FEEL-ROW-HOVER-IN-OWN-DIALOG)", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 50);
            l.double_click(cx, "feature:Extrude 1");
            crate::ui_script::clear_tooltip();
            l.wait(cx, 600);
            let tip = crate::ui_script::shown_tooltip();
            expect((l.editing(cx), tip.is_some_and(|t| t.starts_with("Extrude 1"))), (Some("Extrude 1".to_string()), true), "(editing, the name's tooltip shown)")
        } },
    ]
}

/// Folders made in the model: each (name, its features, levels): `levels` folders nested round the features, the
/// outer one named `name` (the inner ones "<name>2"…), all collapsed.
fn nest(l: &List, cx: &mut TestAppContext, folders: &[(&str, &[&str], usize)]) {
    cx.update(|cx| {
        l.view.update(cx, |v, cx| {
            let tab = v.tab;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "folders", cx, |ps| {
                    for (name, features, levels) in folders {
                        let ids: Vec<noshape_model::FeatureId> = features.iter().filter_map(|n| ps.features.iter().find(|f| f.name == *n).map(|f| f.id)).collect();
                        let mut made = Vec::new();
                        for _ in 0..*levels {
                            made.push(ps.new_folder(&ids, None));
                        }
                        for (k, id) in made.iter().enumerate() {
                            if let Some(f) = ps.folder_mut(*id) {
                                f.name = if k == 0 { name.to_string() } else { format!("{name}{}", k + 1) };
                                f.collapsed = true;
                            }
                        }
                    }
                });
            });
        })
    });
    draw(cx, l.window);
}

/// Sketch 3 and Extrude 3 in a new folder (collapsed), nothing selected after.
fn folder_of_last_part(l: &List, cx: &mut TestAppContext) {
    l.click(cx, "feature:Sketch 3", NONE);
    l.click(cx, "feature:Extrude 3", NONE);
    l.new_folder(cx, true);
    cx.update(|cx| {
        l.view.update(cx, |v, cx| {
            v.selected_features.clear();
            v.ui.anchor = None;
            cx.notify();
        })
    });
    draw(cx, l.window);
}

fn expanded_folder_of_last_part(l: &List, cx: &mut TestAppContext) {
    folder_of_last_part(l, cx);
    cx.update(|cx| l.view.update(cx, |v, cx| {
        let id = v.studio(cx).and_then(|s| s.folders.first().map(|f| f.id));
        if let Some(id) = id {
            v.set_folder_collapsed(id, false, cx);
        }
    }));
    draw(cx, l.window);
}

/// Every row at a pixel ratio of 1 and 2, each on a fresh Part Studio. Prints matched / total (`LIST_ROW=<part of an
/// id>` runs only those rows).
#[gpui::test]
fn the_feature_list_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let only = std::env::var("LIST_ROW").ok();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if only.as_deref().is_some_and(|o| !r.id.contains(o)) {
            continue;
        }
        let mut why = Vec::new();
        for scale in [1.0f32, 2.0] {
            let l = studio(cx, &format!("list-{i}-{scale}"), scale);
            if let Err(e) = (r.check)(&l, cx) {
                why.push(format!("scale {scale}: {e}"));
            }
            // The open menu is the app's (one at a time): Escape closes it before the window goes.
            if cx.update(|cx| crate::os_menu::is_open(cx)) {
                l.keys(cx, "escape");
            }
            assert!(!cx.update(|cx| crate::os_menu::is_open(cx)), "{}: Escape left a menu open", r.id);
            cx.update_window(l.window, |_, w, _| w.remove_window()).unwrap();
        }
        let ok = why.is_empty();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, if ok { String::new() } else { format!("\n    {}", why.join("\n    ")) }));
        if ok == r.gap {
            moved.push(format!("{}{}", r.id, if ok { String::new() } else { format!(": {}", why.join("; ")) }));
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("feature list matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}

/// live2 fl-autoscroll-1118: the drop slot is picked only by a move, so a list scrolling under a still pointer (the
/// autoscroll) carries the insertion line away with its rows and a release without moving drops at that line, not at
/// the row now under the pointer. Sketch 2 dragged to the top half of Sketch 10, the list (60 sketches) scrolled 5
/// rows, released.
#[gpui::test]
fn a_list_scrolled_under_a_still_drag_drops_at_its_line(cx: &mut TestAppContext) {
    let features: Vec<serde_json::Value> = (1..=60).map(|n| serde_json::json!({ "type": "newSketch", "name": format!("Sketch {n}"), "plane": "Top", "shapes": [{ "corner_rectangle": [[0.0, 0.0], [10.0, 10.0]] }] })).collect();
    let (window, view) = part_studio_from(cx, "fl-autoscroll", Some(serde_json::json!({ "units": "mm", "features": features })));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let l = List { window, view };
    let (from, to) = (l.at(cx, "feature:Sketch 2"), l.at_y(cx, "feature:Sketch 10", 6.));
    l.drag(cx, from, to, false);
    let before = l.bounds(cx, "feature:Sketch 10").expect("Sketch 10's row");
    let row_h = f32::from(before.size.height);
    cx.update(|cx| l.view.update(cx, |v, cx| {
        let o = v.ui.list_scroll.offset();
        v.ui.list_scroll.set_offset(point(o.x, o.y - px(5. * row_h)));
        cx.notify();
    }));
    draw(cx, window);
    let moved = l.bounds(cx, "feature:Sketch 10").map(|b| f32::from(before.top() - b.top()));
    assert!(moved.is_some_and(|m| (m - 5. * row_h).abs() < 1.), "the list scrolled 5 rows under the pointer: {moved:?}");
    l.release(cx, to);
    wait_until(cx, window, |cx| !l.view.read(cx).ui_busy());
    let order = l.order(cx);
    let at = |name: &str| order.iter().position(|n| n == name);
    assert_eq!((at("Sketch 2"), at("Sketch 10")), (Some(8), Some(9)), "Sketch 2 just before Sketch 10 (its line), not before the row under the pointer: {order:?}");
}

/// Parts list › Properties… on two parts (live q2962): one modal for both, "2 selected", the differing fields empty
/// ("Multiple values") and the common ones shown; a Description typed and saved goes to both and nothing else changes
/// (each keeps its name, part number and Exclude from all BOMs); a row clicked shows that part alone, Ctrl+click brings
/// the other back.
#[gpui::test]
fn several_parts_share_one_properties_dialog(cx: &mut TestAppContext) {
    let l = studio(cx, "multi-props", 1.0);
    let parts: Vec<noshape_model::reference::BodyId> = cx.update(|cx| l.view.read(cx).display.bodies.iter().map(|b| b.id.clone()).collect());
    let doc = cx.update(|cx| l.view.read(cx).doc.clone());
    let tab = cx.update(|cx| l.view.read(cx).tab);
    cx.update(|cx| {
        doc.update(cx, |d, cx| {
            d.edit_studio(tab, "setup", cx, |ps| {
                for (b, desc, pn, exclude) in [(&parts[0], "A", "PN-1", true), (&parts[1], "B", "PN-2", false)] {
                    let p = ps.part_properties.entry(b.clone()).or_default();
                    p.description = Some(desc.into());
                    p.part_number = Some(pn.into());
                    p.meta.vendor = Some("V".into());
                    p.meta.exclude_from_bom = exclude;
                }
            })
        })
    });
    draw(cx, l.window);
    l.click(cx, "part:Part 1", NONE);
    l.click(cx, "part:Part 2", Modifiers::secondary_key());
    assert_eq!(l.selected(cx), ["Part 1", "Part 2"]);
    l.right_click(cx, "part:Part 2");
    let at = l.bounds(cx, "menu:Properties…").expect("Properties… in the menu").center();
    l.press(cx, at, MouseButton::Left, NONE, 1);
    let form = crate::properties_dialog::open_form().expect("the Properties modal");
    let read = |cx: &mut TestAppContext| cx.update(|cx| (form.read(cx).header(), form.read(cx).mixed_fields(), form.read(cx).list_rows()));
    let (header, mixed, rows) = read(cx);
    assert_eq!(header, "2 selected");
    assert_eq!(mixed, ["Name", "Description", "Part number", "Exclude from all BOMs"], "the differing fields (Vendor V is common)");
    assert_eq!(rows, [("Part 1".to_string(), true), ("Part 2".to_string(), true)]);
    cx.update_window(l.window, |_, w, cx| form.update(cx, |f, cx| f.focus_field("Description", w, cx))).unwrap();
    l.keys(cx, "D");
    assert_eq!(cx.update(|cx| form.read(cx).edited_marks()), ["Description"], "the orange corner on the edited field (and on both rows)");
    cx.update_window(l.window, |_, w, cx| form.update(cx, |f, cx| f.press_save(true, w, cx))).unwrap();
    draw(cx, l.window);
    let props = |cx: &mut TestAppContext, i: usize| cx.update(|cx| l.view.read(cx).studio(cx).and_then(|s| s.part_properties.get(&parts[i]).cloned()).unwrap_or_default());
    for (i, pn, exclude) in [(0, "PN-1", true), (1, "PN-2", false)] {
        let p = props(cx, i);
        assert_eq!((p.description.as_deref(), p.part_number.as_deref(), p.meta.vendor.as_deref(), p.meta.exclude_from_bom), (Some("D"), Some(pn), Some("V"), exclude), "part {}", i + 1);
    }
    let names: Vec<String> = cx.update(|cx| l.view.read(cx).display.bodies.iter().map(|b| b.name.clone()).collect());
    assert_eq!(names, ["Part 1", "Part 2", "Part 3"], "the names stay");
    // Again: a row alone, then both.
    l.right_click(cx, "part:Part 2");
    let at = l.bounds(cx, "menu:Properties…").expect("Properties… in the menu").center();
    l.press(cx, at, MouseButton::Left, NONE, 1);
    let form = crate::properties_dialog::open_form().expect("the Properties modal");
    cx.update_window(l.window, |_, w, cx| form.update(cx, |f, cx| f.press_row(0, NONE, w, cx))).unwrap();
    let (header, mixed, rows) = cx.update(|cx| (form.read(cx).header(), form.read(cx).mixed_fields(), form.read(cx).list_rows()));
    assert_eq!((header.as_str(), mixed.len()), ("Part 1", 0));
    assert_eq!(rows, [("Part 1".to_string(), true), ("Part 2".to_string(), false)], "the list still shows both");
    cx.update_window(l.window, |_, w, cx| form.update(cx, |f, cx| f.press_row(1, Modifiers::secondary_key(), w, cx))).unwrap();
    assert_eq!(cx.update(|cx| form.read(cx).header()), "2 selected");
}

/// The part Properties modal is live's 1173×687 with nothing stored, centred, 15 px from the top
/// (live psui/properties-dialog-1123 at 1920×1080: buttons 5.25 px apart, Close x 1474.1 w 57.9, y 653 h 33.5).
#[gpui::test]
fn the_properties_modal_has_lives_size(cx: &mut TestAppContext) {
    let l = studio(cx, "props-size", 1.0);
    assert_eq!(crate::properties_dialog::props_size(), (1173., 687.), "nothing stored");
    l.right_click(cx, "part:Part 1");
    let at = l.bounds(cx, "menu:Properties…").expect("Properties… in the menu").center();
    l.press(cx, at, MouseButton::Left, NONE, 1);
    let mid = cx.update_window(l.window, |_, w, _| f32::from(w.viewport_size().width) / 2.).unwrap();
    let [apply, save, close] = ["meta-apply", "meta-save", "meta-close"].map(|id| l.bounds(cx, id).expect(id));
    // (The buttons' widths follow the font, so Apply's left is checked through the 5.25 px gaps.)
    let got = [f32::from(save.left() - apply.right()), f32::from(close.left() - save.right()), f32::from(close.right()) - mid, f32::from(close.top()), f32::from(close.bottom())];
    let live = [5.25, 5.25, 1474.1 + 57.9 - 960., 653., 686.5];
    assert!(got.iter().zip(live).all(|(g, l)| (g - l).abs() <= 1.), "gaps, Close right (from the centre), Close top/bottom: ours {got:?}, live {live:?}");
}

/// A part's Properties: Reset all goes by the saved values, so a Part number only typed stays (live
/// part-properties-typed-reset-all: "P-100" kept); one saved is reset.
#[gpui::test]
fn reset_all_resets_saved_properties_not_typing(cx: &mut TestAppContext) {
    let l = studio(cx, "reset-props", 1.0);
    let open = |cx: &mut TestAppContext| {
        l.right_click(cx, "part:Part 1");
        let at = l.bounds(cx, "menu:Properties…").expect("Properties… in the menu").center();
        l.press(cx, at, MouseButton::Left, NONE, 1);
        draw(cx, l.window);
        crate::properties_dialog::open_form().expect("the Properties modal")
    };
    let pn = |cx: &mut TestAppContext, f: &Entity<crate::properties_dialog::MetaForm>| cx.update(|cx| f.read(cx).values(cx).part_number);
    let form = open(cx);
    cx.update_window(l.window, |_, w, cx| form.update(cx, |f, cx| f.focus_field("Part number", w, cx))).unwrap();
    gpui_kit::VisualTestContext::from_window(l.window, cx).simulate_input("P-100");
    draw(cx, l.window);
    l.click(cx, "meta-reset", NONE);
    assert_eq!(pn(cx, &form), "P-100", "typing alone doesn't enable Reset all");
    cx.update_window(l.window, |_, w, cx| form.update(cx, |f, cx| f.press_save(true, w, cx))).unwrap();
    draw(cx, l.window);
    let form = open(cx);
    assert_eq!(pn(cx, &form), "P-100", "saved");
    l.click(cx, "meta-reset", NONE);
    assert_eq!(pn(cx, &form), "", "a saved Part number is reset");
}
