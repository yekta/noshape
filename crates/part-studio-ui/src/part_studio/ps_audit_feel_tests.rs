//! Part Studio feel specs from live's behaviours.md that no other test covered (docs/parity/ps-feel-audit.md): one test
//! per spec, acted out on a fresh Part Studio.

use super::*;

/// Two blocks and a Loft with nothing picked (it fails): Origin, the three planes and five features.
fn blocks_and_a_failing_loft(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let rect = |n: u32, x: f64| serde_json::json!({ "type": "newSketch", "name": format!("Sketch {n}"), "plane": "Top", "shapes": [{ "corner_rectangle": [[x, 0.0], [x + 20.0, 20.0]] }] });
    let extrude = |n: u32| serde_json::json!({ "type": "extrude", "name": format!("Extrude {n}"), "params": { "entities": [format!("Sketch {n} region 0")], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } });
    let loft = serde_json::json!({ "type": "loft", "name": "Loft 1", "params": {} });
    let doc = serde_json::json!({ "units": "mm", "features": [rect(1, 0.0), extrude(1), rect(2, 40.0), extrude(2), loft] });
    let (window, view) = part_studio_from(cx, name, Some(doc));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 2);
    draw(cx, window);
    (window, view)
}

fn has(cx: &mut TestAppContext, window: AnyWindowHandle, sel: &str) -> bool {
    draw(cx, window);
    gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(sel.to_string().into_boxed_str())).is_some()
}

/// Live 10-27 (spec: psui part-studio.md 6.10 / 6.11): a filter keeps the features whose name or type holds the text
/// (case-insensitive), the title reads "Features (M of N)", and the red error icon goes while the filter is on, though a
/// feature still fails; an incomplete command (":type") filters nothing; clearing the box brings it all back.
#[gpui::test]
fn the_feature_filter_counts_in_the_title_and_hides_the_error_icon(cx: &mut TestAppContext) {
    let (window, view) = blocks_and_a_failing_loft(cx, "audit-filter");
    let state = |cx: &mut TestAppContext, q: &str| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_filter(q, w, cx))).unwrap();
        let title = ["Features …", "Features (9)", "Features (2 of 9)", "Features (0 of 9)", "Features (1 of 9)"].into_iter().find(|t| has(cx, window, &format!("features-title {t}")));
        let rows: Vec<&str> = ["Sketch 1", "Extrude 1", "Extrude 2", "Loft 1"].into_iter().filter(|r| has(cx, window, &format!("feature:{r}"))).collect();
        (title, has(cx, window, "feature-errors"), rows.join(", "))
    };
    let got = [
        ("", state(cx, "")),
        ("EXTRU", state(cx, "EXTRU")),
        (":type extrude", state(cx, ":type extrude")),
        ("xyz", state(cx, "xyz")),
        ("loft", state(cx, "loft")),
        (":type", state(cx, ":type")),
        ("cleared", state(cx, "")),
    ];
    let want = [
        (Some("Features …"), true, "Sketch 1, Extrude 1, Extrude 2, Loft 1"),
        (Some("Features (2 of 9)"), false, "Extrude 1, Extrude 2"),
        (Some("Features (2 of 9)"), false, "Extrude 1, Extrude 2"),
        (Some("Features (0 of 9)"), false, ""),
        (Some("Features (1 of 9)"), false, "Loft 1"),
        (Some("Features …"), true, "Sketch 1, Extrude 1, Extrude 2, Loft 1"),
        (Some("Features …"), true, "Sketch 1, Extrude 1, Extrude 2, Loft 1"),
    ];
    let wrong: Vec<String> = got.iter().zip(want).filter(|((_, g), w)| (g.0, g.1, g.2.as_str()) != *w).map(|((q, g), w)| format!("{q:?}: {g:?}, want {w:?}")).collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn dialog_state(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) -> (Option<String>, Option<String>) {
    draw(cx, window);
    let s = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let d = &s["dialog"];
    (d["ok"].as_str().map(str::to_string), d["titleState"].as_str().map(str::to_string))
}

fn click_on(cx: &mut TestAppContext, window: AnyWindowHandle, sel: &'static str) -> bool {
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let Some(b) = vcx.debug_bounds(sel) else { return false };
    vcx.simulate_click(b.center(), Modifiers::default());
    draw(cx, window);
    true
}

/// Live 10-28 (requests 1574, 1575): a new Fillet's ✓ is disabled only until something is picked; with the field
/// emptied again by its chip's × the title is red once more but the ✓ stays enabled, and a click on it commits a failing
/// "Fillet 1".
#[gpui::test]
fn a_fillets_ok_is_off_until_a_pick_and_stays_on_after_the_chip_goes(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "audit-ok-state");
    let edge = cx.update(|cx| {
        let v = view.read(cx);
        let b = &v.context.as_ref().expect("the part").bodies[0];
        Reference::Edge((*b.edges[0]).clone())
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let opened = dialog_state(cx, window, &view);
    cx.update(|cx| d.update(cx, |d, cx| { d.add_pick(edge, cx); }));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let picked = dialog_state(cx, window, &view);
    assert!(click_on(cx, window, "chipx-entities-0"), "no × on the chip");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let emptied = dialog_state(cx, window, &view);
    let s = |a: &str, b: &str| (Some(a.to_string()), Some(b.to_string()));
    assert_eq!([opened, picked, emptied], [s("disabled", "error"), s("enabled", "ok"), s("enabled", "error")], "(✓, title): opened, one pick, its chip removed");
    assert!(click_on(cx, window, "dialog-ok"), "no ✓");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let failing = cx.update(|cx| {
        let v = view.read(cx);
        v.edit.is_none() && v.studio(cx).expect("the studio").features.iter().find(|f| f.name == "Fillet 1").is_some_and(|f| v.display.feature_state(f.id).is_some_and(|s| s.shown_status() == noshape_model::display::FeatureStatus::Error))
    });
    assert!(failing, "✓ commits a failing Fillet 1");
}

/// Live (psb 10-21 e): a feature dialog's ✓ and ✕ have no tooltip.
#[gpui::test]
fn a_dialogs_ok_and_cancel_have_no_tooltip(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "audit-ok-tip");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let tip = |cx: &mut TestAppContext, sel: &'static str| {
        draw(cx, window);
        let b = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(sel).unwrap_or_else(|| panic!("no {sel}"));
        crate::ui_script::clear_tooltip();
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: b.center(), pressed_button: None, modifiers: Modifiers::default() });
        for _ in 0..150 {
            cx.executor().advance_clock(std::time::Duration::from_millis(10));
            draw(cx, window);
        }
        crate::ui_script::shown_tooltip()
    };
    assert_eq!((tip(cx, "dialog-ok"), tip(cx, "dialog-cancel")), (None, None), "(✓, ✕) tooltips");
}

/// Live 11-03 (request 1910): an edge selected off its midpoint carries into a new Fillet's "Entities to fillet" (still
/// lit, as its pick); selected at its midpoint (an implicit point: Fillet takes no points) the Fillet opens empty and
/// nothing stays lit.
#[gpui::test]
fn a_preselected_edge_carries_into_fillet_its_midpoint_does_not(cx: &mut TestAppContext) {
    let mut got = Vec::new();
    for mid in [false, true] {
        let (window, view, _, _) = block_part(cx, if mid { "audit-presel-mid" } else { "audit-presel-edge" });
        let edge = cx.update(|cx| {
            let v = view.read(cx);
            let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
            b.edge_refs.iter().find(|e| v.edge_midpoint(e).is_some()).expect("an edge with ends").clone()
        });
        let pick = if mid { Reference::Implicit { on: Box::new(edge.clone()), inference: noshape_model::reference::Inference::Midpoint } } else { edge.clone() };
        view.update(cx, |v, _| v.selection = vec![pick]);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
        let lit = cx.update(|cx| d.read(cx).active_refs().contains(&edge));
        got.push((picks(cx, &d, "entities") == [edge.clone()], lit));
    }
    assert_eq!(got, [(true, true), (false, false)], "(edge in Entities to fillet, edge still lit): edge picked off its midpoint, at it");
}

/// Live 10-26 (psui m-0*): a Mirror's picked part is lit (outlined, its Parts row selected) only while "Entities to mirror" is the active field; with Mirror
/// plane active it isn't, and back in Entities it is again.
#[gpui::test]
fn a_mirrors_part_is_lit_only_while_its_field_is_active(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "audit-active-picks");
    let part = cx.update(|cx| Reference::Body(view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").id.clone()));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("mirror", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let first = active(cx, &d).unwrap_or_default();
    cx.update(|cx| d.update(cx, |d, cx| { d.add_pick(part.clone(), cx); }));
    let lit = |cx: &mut TestAppContext| {
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
        cx.update(|cx| d.read(cx).active_refs().contains(&part))
    };
    let mut got = vec![(first.clone(), lit(cx))];
    for field in ["mirrorPlane", first.as_str()] {
        // As a click on the field: its tab stop takes the keyboard, which makes it the active field.
        let h = cx.update(|cx| d.read(cx).tab_stop_handle(&format!("q:{field}"))).unwrap_or_else(|| panic!("no tab stop for {field}"));
        cx.update_window(window, |_, w, cx| w.focus(&h, cx)).unwrap();
        let l = lit(cx);
        got.push((active(cx, &d).unwrap_or_default(), l));
    }
    assert_eq!(got, [(first.clone(), true), ("mirrorPlane".to_string(), false), (first, true)], "(active field, part lit)");
}

/// Live 10-27 (PANEL-BLEED): Shift+D or Shift+C over an open feature dialog, new or edited, opens the analysis panel
/// beside it and leaves the dialog as it is; the first Esc closes only the panel, the second cancels the dialog.
#[gpui::test]
fn an_analysis_key_over_a_feature_dialog_keeps_it_and_esc_closes_the_panel_first(cx: &mut TestAppContext) {
    let mut got = Vec::new();
    for key in ["shift-d", "shift-c"] {
        let (window, view, _, _) = block_part(cx, &format!("audit-panel-bleed-{key}"));
        let count = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).expect("the studio").features.len());
        let before = count(cx);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("revolve", w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let open = |cx: &mut TestAppContext| {
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            cx.update(|cx| (view.read(cx).edit.is_some(), view.read(cx).ui.analysis_tool.is_some()))
        };
        keys(cx, window, &[key]);
        let after_key = open(cx);
        keys(cx, window, &["escape"]);
        let after_esc = open(cx);
        keys(cx, window, &["escape"]);
        let after_esc2 = open(cx);
        got.push((key, after_key, after_esc, after_esc2, count(cx) == before));
    }
    let want = |k| (k, (true, true), (true, false), (false, false), true);
    assert_eq!(got, [want("shift-d"), want("shift-c")], "(key, (dialog, panel) after the key, one Esc, two, the feature list unchanged)");
}

/// Live 10-27 (spec: psui part-studio.md 8.8 / 8.10): the empty-space menu leads with the mode that's on: "Exit isolate",
/// "Exit make transparent", the section view's "Turn off section view" after Show all, and a feature dialog's "Confirm
/// <feature>" and "Cancel".
#[gpui::test]
fn the_empty_space_menu_leads_with_the_active_mode(cx: &mut TestAppContext) {
    let mut wrong = Vec::new();
    let cases: [(&str, &[&str], &[&str]); 5] = [
        ("nothing", &[], &["Show all", "Select", "Zoom to fit", "Isometric"]),
        ("isolate", &["shift-i"], &["Exit isolate", "Show all", "Select", "Zoom to fit", "Isometric"]),
        ("transparent", &["shift-t"], &["Exit make transparent", "Show all", "Select", "Zoom to fit", "Isometric"]),
        ("section", &["shift-x"], &["Show all", "Turn off section view", "Zoom to fit", "Isometric"]),
        ("extrude", &["shift-e"], &["Confirm Extrude 2", "Cancel", "Show all", "Curve/surface analysis…", "Select", "Zoom to fit", "Isometric"]),
    ];
    for (name, keys_in, want) in cases {
        let (window, view, _, _) = block_part(cx, &format!("audit-mode-menu-{name}"));
        let part = cx.update(|cx| Reference::Body(view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").id.clone()));
        // Live hid the sketch as its Extrude does; isolate and transparent act on the selected part.
        cx.update(|cx| view.update(cx, |v, _| v.selection = if keys_in.is_empty() || name == "section" || name == "extrude" { vec![] } else { vec![part.clone()] }));
        keys(cx, window, keys_in);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        cx.update(|cx| view.update(cx, |v, _| v.selection.clear()));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(None, gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        draw(cx, window);
        let labels: Vec<String> = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default()).into_iter().filter(|l| !l.starts_with('—')).collect();
        if labels != want {
            wrong.push(format!("{name}: {labels:?}, live {want:?}"));
        }
        if cx.update(|cx| crate::os_menu::is_open(cx)) {
            keys(cx, window, &["escape"]);
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Live 10-26 (psui loft entries): a Loft profile's › opens the item under its row and makes its own field the active
/// one; a second item opens too (more than one stays open); its ⌄ closes it again.
#[gpui::test]
fn a_loft_profiles_expander_opens_its_field_as_the_active_one(cx: &mut TestAppContext) {
    // (The second profile on Front: the entries' expanders don't need the Loft to succeed.)
    let doc = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[-10, -10], [10, 10]] }] },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Front", "shapes": [{ "circle": [[0, 30], [10, 30]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "audit-loft-expander", Some(doc));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.sketches.len() == 2);
    let regions: Vec<Reference> = cx.update(|cx| view.read(cx).display.sketches.iter().filter_map(|s| s.regions.first().map(|r| Reference::SketchRegion { sketch: s.feature, key: r.key.clone() })).collect());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("loft", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    for r in regions {
        cx.update(|cx| d.update(cx, |d, cx| { d.add_pick(r, cx); }));
    }
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let profiles = cx.update(|cx| d.read(cx).feature.params.array("sheetProfilesArray").len());
    let mut got = Vec::new();
    for sel in ["dqe-sheetProfilesArray-0", "dqe-sheetProfilesArray-1", "dqe-sheetProfilesArray-0"] {
        assert!(click_on(cx, window, Box::leak(sel.to_string().into_boxed_str())), "no {sel}");
        got.push((active(cx, &d), cx.update(|cx| d.read(cx).open_items_len())));
    }
    let item = |k: usize| Some(format!("sheetProfilesArray.{k}.sheetProfileEntities"));
    assert_eq!((profiles, got), (2, vec![(item(0), 1), (item(1), 2), (item(1), 1)]), "(profiles, (active field, items open) after › on 1, › on 2, ⌄ on 1)");
}
