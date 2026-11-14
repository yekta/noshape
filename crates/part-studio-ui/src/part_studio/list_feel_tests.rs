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

    fn click(&self, cx: &mut TestAppContext, row: &str, m: Modifiers) {
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

    fn dragging(&self, cx: &mut TestAppContext) -> bool {
        cx.update(|cx| cx.has_active_drag())
    }
}

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false, platform: false, function: false };
const SHIFT: Modifiers = Modifiers { shift: true, ..NONE };
const CTRL: Modifiers = Modifiers { control: true, ..NONE };
const ALT: Modifiers = Modifiers { alt: true, ..NONE };

type Check = fn(&List, &mut TestAppContext) -> Result<(), String>;

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
        Row { id: "double-click / a feature's blank right side", live: "its dialog too", gap: false, check: |l, cx| {
            let b = l.bounds(cx, "feature:Extrude 1").ok_or("no row")?;
            l.double_click_at(cx, point(b.origin.x + b.size.width - px(40.), b.center().y));
            expect(l.editing(cx), Some("Extrude 1".to_string()), "editing")
        } },
        Row { id: "double-click / a sketch: sketch mode", live: "edits Sketch 1", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Sketch 1");
            expect(cx.update(|cx| l.view.read(cx).sketch.is_some()), true, "in sketch mode")
        } },
        Row { id: "double-click / a default plane: nothing", live: "no dialog, no rename (live's document had no Plane feature: its plane row is Top)", gap: false, check: |l, cx| {
            l.double_click(cx, "feature:Top");
            let renaming = cx.update(|cx| l.view.read(cx).ui.dg_renaming.is_some());
            expect((l.editing(cx), renaming), (None, false), "editing, renaming")
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
        Row { id: "rename / Rename in the menu: inline, all selected", live: "an input over the row, the whole name selected; Escape cancels", gap: false, check: |l, cx| {
            l.right_click(cx, "feature:Extrude 1");
            let at = l.bounds(cx, "menu:Rename").ok_or("no Rename row")?.center();
            l.press(cx, at, MouseButton::Left, NONE, 1);
            let input = cx.update(|cx| l.view.read(cx).renaming.as_ref().map(|(_, st, _)| (st.read(cx).value().to_string(), st.read(cx).selected_range().len())));
            l.keys(cx, "x escape");
            let after = (cx.update(|cx| l.view.read(cx).renaming.is_some()), l.order(cx)[1].clone());
            expect((input, after), (Some(("Extrude 1".to_string(), 9)), (false, "Extrude 1".to_string())), "input (value, selected bytes), after typing and Escape")
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
        Row { id: "hover / the row's tooltip", live: "'<name> <- <consumed features>': 'Extrude 1 <- Sketch 1'", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 450);
            expect(crate::ui_script::shown_tooltip(), Some("Extrude 1 <- Sketch 1".to_string()), "tooltip")
        } },
        Row { id: "hover / the tooltip from 160 ms", live: "fades in from about 160 ms, fully shown by about 390 ms (ours: shown at 160 ms, a 230 ms fade)", gap: false, check: |l, cx| {
            l.hover(cx, "feature:Extrude 1", 120);
            let early = crate::ui_script::shown_tooltip();
            l.wait(cx, 80);
            expect((early, crate::ui_script::shown_tooltip().is_some()), (None, true), "at 120 ms, at 200 ms")
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
        Row { id: "folders / New folder with nothing selected", live: "the 'Folder name' popover; Enter makes an empty 'New folder (0)' just above the rollback bar", gap: false, check: |l, cx| {
            l.new_folder(cx, true);
            let after = cx.update(|cx| l.view.read(cx).studio(cx).and_then(|s| s.folders.first().and_then(|fd| fd.after).and_then(|a| s.feature(a)).map(|f| f.name.clone())));
            // (Its caret: live2's queued feature-list-new-folder-empty says whether an empty one opens collapsed.)
            let folders: Vec<(String, Vec<String>)> = l.folders(cx).into_iter().map(|(n, f, _)| (n, f)).collect();
            expect((folders, after), (vec![("New folder".to_string(), Vec::new())], Some("Extrude 3".to_string())), "folders, listed after")
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
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("feature list matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
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
