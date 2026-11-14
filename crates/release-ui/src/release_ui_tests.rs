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

/// Live (release.json): "Create Release candidate", then "Review Release: <name>" once applied (and for a submitted
/// or released one); the obsoletion dialog's own title.
#[gpui::test]
fn the_release_dialogs_titles_are_lives(cx: &mut TestAppContext) {
    let (window, view, _) = dialog(cx, "titles", Mode::Release, "R1");
    cx.update(|cx| assert_eq!(view.read(cx).title(cx), "Create Release candidate"));
    cx.update(|cx| view.update(cx, |v, _| v.applied = true));
    draw(cx, window);
    cx.update(|cx| assert_eq!(view.read(cx).title(cx), "Review Release: R1"));
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
