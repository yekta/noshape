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

/// Live 10-28 (request 1562): with Sketch 1, the Top plane and the part hidden, the empty-space menu's Show all brings
/// back all three; Show all parts (from the same setup) brings back only the part.
#[gpui::test]
fn show_all_shows_sketches_planes_and_parts_show_all_parts_only_the_parts(cx: &mut TestAppContext) {
    let mut got = Vec::new();
    for item in ["Show all", "Show all parts"] {
        let (window, view, _, _) = block_part(cx, &format!("audit-show-all-{}", item.len()));
        let refs = cx.update(|cx| {
            let d = &view.read(cx).display;
            let sketch = Reference::Feature(d.sketches.iter().find(|s| s.name == "Sketch 1").expect("Sketch 1").feature);
            let top = d.planes.iter().find(|p| p.name == "Top").expect("Top").reference.clone();
            let part = Reference::Body(d.bodies.first().expect("the part").id.clone());
            vec![sketch, top, part]
        });
        cx.update(|cx| view.update(cx, |v, cx| v.set_visibility(&refs, true, cx)));
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.first().is_some_and(|b| b.hidden));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(None, gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        let sel: &'static str = Box::leak(format!("menu:{item}").into_boxed_str());
        assert!(click_on(cx, window, sel), "no {item} row");
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.first().is_some_and(|b| !b.hidden));
        draw(cx, window);
        let shown = cx.update(|cx| {
            let v = view.read(cx);
            let d = &v.display;
            let sketch = d.sketches.iter().find(|s| s.name == "Sketch 1").is_some_and(|s| !s.hidden) && v.opts.show_sketches;
            let top = d.planes.iter().find(|p| p.name == "Top").is_some_and(|p| !p.hidden) && v.opts.show_planes;
            let part = d.bodies.first().is_some_and(|b| !b.hidden);
            (sketch, top, part)
        });
        got.push((item, shown));
    }
    assert_eq!(got, vec![("Show all", (true, true, true)), ("Show all parts", (false, false, true))], "(Sketch 1, Top, Part 1) shown after each");
}

/// Live 10-27 (MENU-TRUNCATE, request 1509): with Extrude 1 renamed "Linear pattern with a very long name 1", an
/// edge menu is at most 250 px wide and the rows naming it (Edit, Show dependencies of, Delete) stay one 22 px line,
/// cut with "…", never wrapped.
#[gpui::test]
fn a_long_feature_name_keeps_its_menu_250_px_wide_with_one_line_rows(cx: &mut TestAppContext) {
    let long = "Linear pattern with a very long name 1";
    let doc = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0.0, 0.0], [20.0, 20.0]] }] },
        { "type": "extrude", "name": long, "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
    ] });
    let (window, view) = part_studio_from(cx, "audit-menu-truncate", Some(doc));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 1);
    draw(cx, window);
    let edge = cx.update(|cx| view.read(cx).display.bodies.first().and_then(|b| b.edge_refs.first().cloned())).expect("an edge of the part");
    let target = noshape_scene::builder::PickTarget { reference: edge, kind: noshape_scene::builder::PickKind::Edge, anchor: None };
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(Some(target), gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
    draw(cx, window);
    let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
    let naming: Vec<&String> = labels.iter().filter(|l| l.contains(long)).collect();
    assert!(naming.len() >= 3, "rows naming the feature: {labels:?}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    // (label, row width, row height): the rows fill the menu less its 1 px borders (live 248 in 250).
    let rows: Vec<(String, f32, f32)> = naming.iter().map(|l| {
        let b = vcx.debug_bounds(Box::leak(format!("menu:{l}").into_boxed_str()));
        ((*l).clone(), b.map_or(0., |b| f32::from(b.size.width)), b.map_or(0., |b| f32::from(b.size.height)))
    }).collect();
    assert!(rows.iter().all(|(_, w, h)| *w > 0. && *w <= 248.5 && (*h - 22.).abs() < 0.5), "rows {rows:?}, live 248 wide (menu 250 at most), 22 high (one line)");
}

/// Live 2212 (11-07): the view cube's Camera menu has no greyed row, Previous view included, in a freshly opened Part
/// Studio (no view change yet) and with a part selected.
#[gpui::test]
fn the_camera_menu_greys_no_row_not_even_previous_view_at_a_start(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "audit-camera-menu");
    let part = cx.update(|cx| Reference::Body(view.read(cx).display.bodies.first().expect("the part").id.clone()));
    let mut wrong = Vec::new();
    for (case, selection) in [("at a start", vec![]), ("a part selected", vec![part])] {
        cx.update(|cx| view.update(cx, |v, _| v.selection = selection));
        assert!(click_on(cx, window, "camera-menu"), "no camera menu button");
        let state = cx.update(|cx| crate::os_menu::ui_state(cx)).expect("the camera menu open");
        let rows: Vec<String> = serde_json::from_value(state["rows"].clone()).unwrap_or_default();
        let flags: Vec<bool> = serde_json::from_value(state["disabled"].clone()).unwrap_or_default();
        let greyed: Vec<&String> = rows.iter().zip(&flags).filter(|(_, d)| **d).map(|(r, _)| r).collect();
        if !rows.iter().any(|r| r == "Previous view") || !greyed.is_empty() {
            wrong.push(format!("{case}: greyed {greyed:?} of {rows:?}"));
        }
        keys(cx, window, &["escape"]);
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Live 10-27 (PANEL-BLEED follow-up): over a new Revolve at its default place, Shift+D's Dihedral panel opens at the
/// same top-left corner and width (220), 226 high (empty), shorter than the dialog, so the Revolve's bottom rows stay visible.
#[gpui::test]
fn the_analysis_panel_opens_on_the_feature_dialogs_corner(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "audit-panel-corner");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("revolve", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    keys(cx, window, &["shift-d"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let dialog = vcx.debug_bounds("feature-dialog").expect("the Revolve dialog");
    let panel = vcx.debug_bounds("analysis-tool").expect("the Dihedral panel");
    let corner = |b: Bounds<gpui_kit::Pixels>| (f32::from(b.origin.x).round(), f32::from(b.origin.y).round(), f32::from(b.size.width).round());
    assert_eq!(corner(panel), corner(dialog), "(x, y, width) of the panel and the dialog");
    assert_eq!((f32::from(panel.size.width).round(), f32::from(panel.size.height).round()), (220., 226.), "the empty Dihedral panel's size");
}

/// Live 10-21 (ui, "Input behaviour outside feature dialogs"): Zebra's Stripe count takes a bare integer only ("20"
/// stays "20"; "1 in", "1+1" and "abc" are invalid); Draft analysis's Minimum draft angle appends its unit ("5" ->
/// "5 deg", "1+1" -> "(1+1) deg"), keeps another unit as typed ("0.1 rad") and marks "abc" invalid.
#[gpui::test]
fn analysis_panel_number_fields_commit_as_live(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let (window, view, _, _) = block_part(cx, "audit-panel-fields");
    let commit = |cx: &mut TestAppContext, field: &Entity<crate::quantity_field::QuantityInput>, text: &str| {
        cx.update_window(window, |_, w, cx| {
            field.update(cx, |q, cx| {
                q.state().update(cx, |s, cx| s.set_value(text.to_string(), w, cx));
                q.commit_now(w, cx);
            })
        })
        .unwrap();
        cx.update(|cx| (field.read(cx).text().to_string(), field.read(cx).value().is_some()))
    };
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_analysis_tool(ToolKind::Zebra, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let stripes = cx.update(|cx| view.read(cx).ui.analysis_tool.as_ref().and_then(|t| t.inputs().first().cloned())).expect("Stripe count");
    let mut got = Vec::new();
    for text in ["20", "1 in", "1+1", "abc"] {
        got.push(("Stripe count", text, commit(cx, &stripes, text)));
    }
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_draft(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let angle = cx.update(|cx| view.read(cx).ui.draft.as_ref().map(|d| d.angle_input().clone())).expect("Minimum draft angle");
    for text in ["5", "0.1 rad", "1+1", "abc"] {
        got.push(("Minimum draft angle", text, commit(cx, &angle, text)));
    }
    let s = |t: &str| t.to_string();
    let want = vec![
        ("Stripe count", "20", (s("20"), true)),
        ("Stripe count", "1 in", (s("1 in"), false)),
        ("Stripe count", "1+1", (s("1+1"), false)),
        ("Stripe count", "abc", (s("abc"), false)),
        ("Minimum draft angle", "5", (s("5 deg"), true)),
        ("Minimum draft angle", "0.1 rad", (s("0.1 rad"), true)),
        ("Minimum draft angle", "1+1", (s("(1+1) deg"), true)),
        ("Minimum draft angle", "abc", (s("abc"), false)),
    ];
    assert_eq!(got, want, "(field, typed, (text after Enter, valid))");
}

/// Live 10-21 (ui, "Panels vs feature dialogs at the top-left"): Isolate is a mode panel, it stays open under a new
/// Sketch's dialog and after its cancel; Mass properties and Named views are tool panels, starting the sketch closes them
/// and they don't come back after the cancel.
#[gpui::test]
fn a_new_sketch_keeps_the_isolate_panel_and_closes_tool_panels(cx: &mut TestAppContext) {
    let mut got = Vec::new();
    for panel in ["Isolate", "Mass properties", "Named views"] {
        let (window, view, _, _) = block_part(cx, &format!("audit-panels-{}", panel.len()));
        let part = cx.update(|cx| Reference::Body(view.read(cx).display.bodies.first().expect("the part").id.clone()));
        cx.update(|cx| view.update(cx, |v, _| v.selection = vec![part.clone()]));
        match panel {
            "Isolate" => keys(cx, window, &["shift-i"]),
            "Mass properties" => cx.update(|cx| view.update(cx, |v, cx| v.open_mass(cx))),
            _ => cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_named_views(w, cx))).unwrap(),
        }
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let open = |cx: &mut TestAppContext| {
            draw(cx, window);
            cx.update(|cx| {
                let v = view.read(cx);
                match panel {
                    "Isolate" => v.ui.isolate.is_some(),
                    "Mass properties" => v.show_mass,
                    _ => v.named_views.is_some(),
                }
            })
        };
        let before = open(cx);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            v.selection.clear();
            v.start_feature("sketch", w, cx);
        }))
        .unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let sketching = open(cx);
        cx.update(|cx| view.update(cx, |v, cx| v.sketch_cancel(cx)));
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).sketch.is_none());
        got.push((panel, before, sketching, open(cx)));
    }
    assert_eq!(got, [("Isolate", true, true, true), ("Mass properties", true, false, false), ("Named views", true, false, false)], "(panel, open before, during the new sketch, after its cancel)");
}

// A dialog checkbox row (live 10-27 p1dlg-checkbox-label-widths): the label starts 20 px in from the dialog's left
// edge (x 266 against Entities to fillet's list at 250..462 of a 246..466 dialog), a group header's (after its 20 px
// expander column) 20 px further, and a label too long for the row (Derived's "Preserve active sheet metal models",
// 209 px of text) is cut at 174 px: up to 26 px short of the dialog's right edge.
#[gpui::test]
fn checkbox_labels_start_20_px_in_and_cut_at_174_px(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "audit-cb-labels");
    let mut got = Vec::new();
    for (feature, rows) in [
        ("fillet", &[("Tangent propagation", "cbl-tangentPropagation"), ("Partial fillet", "cbl-isPartial"), ("Allow edge overflow", "cbl-allowEdgeOverflow"), ("Smooth fillet corners", "cbl-smoothCorners")][..]),
        ("importDerived", &[("Preserve active sheet metal models", "cbl-preserveActiveSheetMetal")][..]),
    ] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(feature, w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let dialog = vcx.debug_bounds("feature-dialog").map(|b| f32::from(b.left())).expect("the dialog");
        for &(name, sel) in rows {
            let b = vcx.debug_bounds(sel).unwrap_or_else(|| panic!("{name} not drawn"));
            let half = |x: f32| (x * 2.).round() / 2.;
            let width = (feature == "importDerived").then(|| half(f32::from(b.size.width)));
            got.push((name, half(f32::from(b.left()) - dialog), width));
        }
        keys(cx, window, &["escape"]);
        wait_until(cx, window, |cx| view.read(cx).edit.is_none());
    }
    let want = [
        ("Tangent propagation", 20., None),
        ("Partial fillet", 40., None),
        ("Allow edge overflow", 40., None),
        ("Smooth fillet corners", 40., None),
        ("Preserve active sheet metal models", 20., Some(174.)),
    ];
    assert_eq!(got, want, "(label, px in from the dialog's left, the cut label's width)");
}
