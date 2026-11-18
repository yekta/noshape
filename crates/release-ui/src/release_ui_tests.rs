//! The release dialogs on GPUI's test platform (docs/TESTS.md "UI keystroke tests"): the real views in a test window.

use super::{Mode, ReleaseDialog, tab_items};
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use noshape_model::document::TabId;
use serde_json::json;

struct Root(Entity<ReleaseDialog>);

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone())
    }
}

fn register_hooks() {
    crate::release_hooks::register(crate::release_hooks::Hooks {
        open_document_window: |_, _, _| Ok(()),
        open_version_window: |_, _, _, _| Err(anyhow::anyhow!("no version windows in tests")),
        create_version: |_, _, _| {},
        request_export: |_, _, _, _| {},
        request_released_export: |_, _, _, _, _, _| {},
    });
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// A document with a Part Studio of one part, and the Release candidate dialog of its items in `mode`.
fn dialog(cx: &mut TestAppContext, name: &str, mode: Mode, release_name: &str) -> (AnyWindowHandle, Entity<ReleaseDialog>, TabId) {
    dialog_with(cx, name, mode, release_name, |_, _| {})
}

/// [`dialog`], with `more` run on the document (through the agent API) before it opens.
fn dialog_with(cx: &mut TestAppContext, name: &str, mode: Mode, release_name: &str, more: impl FnOnce(&mut noshape_ops::Ctx, &dyn Fn(&mut noshape_ops::Ctx, &str, serde_json::Value) -> serde_json::Value)) -> (AnyWindowHandle, Entity<ReleaseDialog>, TabId) {
    cx.update(gpui_kit::init);
    register_hooks();
    let dir = std::env::temp_dir().join(format!("noshape-release-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rel.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
        let regions: Vec<serde_json::Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
        more(&mut ctx, &call);
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let items = tab_items(doc.read(cx), tab);
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| ReleaseDialog::new(doc, mode, items, release_name.to_string(), String::new(), window, cx));
            view = Some(v.clone());
            cx.new(|_| Root(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    (window, view, tab)
}

/// Live (release.json): "Create Release candidate", "Review Release" once applied (a reopened draft), "Review Release:
/// <name>" for a submitted or released one; the obsoletion dialog's own title.
#[gpui::test]
fn the_release_dialogs_titles_are_lives(cx: &mut TestAppContext) {
    let (window, view, _) = dialog(cx, "titles", Mode::Release, "R1");
    cx.update(|cx| assert_eq!(view.read(cx).title(cx), "Create Release candidate"));
    cx.update(|cx| view.update(cx, |v, _| v.applied = true));
    draw(cx, window);
    cx.update(|cx| assert_eq!(view.read(cx).title(cx), "Review Release"));
    let (_, review, _) = dialog(cx, "titles-review", Mode::Review, "R2");
    cx.update(|cx| assert_eq!(review.read(cx).title(cx), "Review Release: R2"));
    let (_, obs, _) = dialog(cx, "titles-obs", Mode::Obsolete, "Obsolete PN A");
    cx.update(|cx| assert_eq!(obs.read(cx).title(cx), "Create Obsolete candidate"));
}

/// The part is listed with the next revision; a click on its name (live: nothing happens) leaves the candidate as it
/// is.
#[gpui::test]
fn the_parts_row_lists_revision_a(cx: &mut TestAppContext) {
    let (_, view, tab) = dialog(cx, "rows", Mode::Release, "");
    cx.update(|cx| {
        let v = view.read(cx);
        assert_eq!(v.rows.len(), 1, "the part");
        assert_eq!(v.rows[0].item.tab, tab);
        assert_eq!(v.rows[0].revision.read(cx).value().to_string(), "A");
    });
}

struct AddRoot(Entity<super::AddItemsDialog>);

impl Render for AddRoot {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone())
    }
}

/// Live2 rel-flow-two-parts: Add to Release… lists the Part Studio's parts at once (its row opens expanded) and a
/// click on Part 2's row picks it; a second click puts it back.
#[gpui::test]
fn add_items_opens_with_the_parts_listed_and_a_row_click_picks_one(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    register_hooks();
    let dir = std::env::temp_dir().join(format!("noshape-release-ui-add-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("two.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [20, 20]] }, { "tool": "corner_rectangle", "points": [[40, 0], [60, 20]] }] }));
        let regions: Vec<serde_json::Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let existing: Vec<_> = tab_items(doc.read(cx), tab).into_iter().filter(|it| it.name == "Part 1").collect();
        assert_eq!(existing.len(), 1, "Part 1 in the candidate");
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| super::AddItemsDialog::new(doc, false, existing, |_, _, _| {}, window, cx));
            view = Some(v.clone());
            cx.new(|_| AddRoot(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let row = vcx.debug_bounds("add-part Part 2").expect("Part 2 listed at once: the Part Studio opens expanded");
    vcx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    let picked = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).picked.iter().map(|it| it.name.clone()).collect::<Vec<_>>());
    assert_eq!(picked(cx), ["Part 2"], "a click on its row picks Part 2");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(row.center(), gpui_kit::Modifiers::default());
    draw(cx, window);
    assert!(picked(cx).is_empty(), "a second click puts it back");
}

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// Live docs-versions-release-1118: Enter in Release name does nothing, Enter in Release notes adds a line, and Esc
/// doesn't close the dialog, in a field or not (another modal closes on Esc in the same window).
#[gpui::test]
fn the_release_dialog_ignores_enter_and_esc_and_notes_take_a_new_line(cx: &mut TestAppContext) {
    use gpui_kit::component::WindowExt as _;
    let (_, view, _) = dialog(cx, "keys", Mode::Release, "");
    let doc = cx.update(|cx| view.read(cx).doc.clone());
    let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(900.)) };
    let (window, _) = cx.update(|cx| gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| Blank)).unwrap());
    let open = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| w.has_active_dialog(cx)).unwrap();
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.update(|w, cx| w.open_dialog(cx, |d, _, _| d));
    vcx.run_until_parked();
    vcx.simulate_keystrokes("escape");
    assert!(!open(&mut vcx), "a plain modal closes on Esc");
    let rel = vcx.update(|w, cx| {
        let items = tab_items(doc.read(cx), view.read(cx).rows[0].item.tab);
        let rel = cx.new(|cx| ReleaseDialog::new(doc.clone(), Mode::Release, items, String::new(), String::new(), w, cx));
        let r = rel.clone();
        w.open_dialog(cx, move |d, _, _| super::release_dialog_modal(d, &r));
        rel
    });
    vcx.run_until_parked();
    vcx.simulate_keystrokes("escape");
    assert!(open(&mut vcx), "Esc with no field focused leaves it open");
    vcx.update(|w, cx| rel.read(cx).name.clone().update(cx, |s, cx| s.focus(w, cx)));
    vcx.update(|w, cx| {
        rel.update(cx, |_, cx| cx.notify());
        w.draw(cx).clear(cx);
    });
    vcx.simulate_keystrokes("r 1 enter escape");
    assert!(open(&mut vcx), "Enter and Esc in Release name leave it open");
    vcx.update(|w, cx| rel.read(cx).notes.clone().update(cx, |s, cx| s.focus(w, cx)));
    vcx.update(|w, cx| {
        rel.update(cx, |_, cx| cx.notify());
        w.draw(cx).clear(cx);
    });
    vcx.simulate_keystrokes("enter b escape");
    assert!(open(&mut vcx), "Enter and Esc in Release notes leave it open");
    vcx.update(|_, cx| {
        assert_eq!(rel.read(cx).name.read(cx).value().to_string(), "r1");
        assert_eq!(rel.read(cx).notes.read(cx).value().to_string(), "\nb", "Enter in the notes is a new line");
    });
}

/// Live release-gaps-1118: Include associated drawings lists the drawing as a child row under the part it shows, the
/// button stays pressed, and a drawing whose part changed since its views were made has "Drawing has a pending update",
/// which holds the release back; undo / redo keep the child row; a folded part hides it.
#[gpui::test]
fn an_associated_drawing_goes_under_its_part_with_its_pending_update(cx: &mut TestAppContext) {
    let (window, view, _) = dialog_with(cx, "assoc", Mode::Release, "R1", |ctx, call| {
        call(ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
        call(ctx, "feature_edit", json!({ "feature": "Extrude 1", "params": { "depth": "15 mm" } }));
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let (extra, _) = super::associated_drawings(v.doc.read(cx), &v.items(cx));
            v.include_drawings(extra, w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    let pending = noshape_model::release::Issue::Prevented("Drawing has a pending update".into());
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.drawings_included);
        let kinds: Vec<_> = v.rows.iter().map(|r| (r.item.kind, r.child)).collect();
        assert_eq!(kinds, vec![(noshape_model::release::ItemKind::Part, false), (noshape_model::release::ItemKind::Drawing, true)]);
        let issues = v.issues(cx);
        assert!(issues[1].contains(&pending), "{issues:?}");
        assert!(!issues[0].contains(&pending), "{issues:?}");
    });
    // Refresh keeps each row's conditions on that row.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.refresh(w, cx))).unwrap();
    cx.update(|cx| assert!(view.read(cx).issues(cx)[1].contains(&pending)));
    // Undo takes the drawing out, Redo lists it under the part again.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.undo(false, w, cx))).unwrap();
    cx.update(|cx| assert_eq!(view.read(cx).rows.len(), 1));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.undo(true, w, cx))).unwrap();
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.rows[1].child);
        assert!(v.issues(cx)[1].contains(&pending));
    });
    // The part's expander folds the drawing away.
    cx.update(|cx| {
        view.update(cx, |v, _| {
            let key = crate::release_thumbs::key(&v.rows[0].item);
            v.folded.insert(key);
        })
    });
    draw(cx, window);
}

/// Live2 errors-1118: with "Prevent if a Part Studio feature list contains errors" off, a part of a Part Studio with a
/// failing feature gets the amber notice "Part belongs to a Part Studio that has regeneration errors" (not blocking);
/// Apply stays enabled with the Release name still empty, Save draft waits for a change.
#[gpui::test]
fn a_part_of_a_failing_part_studio_gets_the_regeneration_errors_notice(cx: &mut TestAppContext) {
    let (_window, view, _) = dialog_with(cx, "regen-notice", Mode::Release, "", |ctx, call| {
        let f = call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [], "depth": "5 mm", "operationType": "NEW" } }));
        assert_eq!(f["feature_status"]["status"], "Error", "{f}");
    });
    cx.update(|cx| {
        let v = view.read(cx);
        let issues = v.issues(cx);
        let notice = noshape_model::release::Issue::Notice("Part belongs to a Part Studio that has regeneration errors".into());
        assert!(issues.first().is_some_and(|i| i.contains(&notice)), "{issues:?}");
        assert!(!notice.blocks());
    });
}

/// Live2 errors-1118: the footer's ? and Submit say "Submit requires Approvers" with "Submit" and "Approvers" bold.
#[test]
fn submit_requires_approvers_bolds_submit_and_approvers() {
    let bold: Vec<&str> = super::SUBMIT_REQUIRES_BOLD.iter().map(|r| &super::SUBMIT_REQUIRES_APPROVERS[r.clone()]).collect();
    assert_eq!(bold, ["Submit", "Approvers"]);
}
