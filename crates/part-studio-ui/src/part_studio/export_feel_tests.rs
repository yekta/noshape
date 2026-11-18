//! Export feel (live2 export-feel-1118, behaviours "dx (lead 11-18 ~02:38, feel)"): a part's Export with Options ›
//! Store file in a new tab shows the blue bubble "<file name> is being prepared for export…", puts "Export completed" /
//! "<file name> was exported successfully." (no extension) in the Notifications bell, and keeps the file as a tab right
//! after the Part Studio's.

use super::*;

#[gpui::test]
fn a_stored_export_says_it_is_prepared_and_rings_the_bell(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
    ] });
    let (window, view) = part_studio_from(cx, "export feel", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let s = noshape_ops::export::ExportSettings { format: noshape_ops::export::PartFormat::Stl, options: 2, ..Default::default() };
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.run_part_export(s, "l2 exp - Part 1".into(), Vec::new(), w, cx))).unwrap();
    let shown = cx.update(|cx| view.read(cx).ui.flashed.clone().map(|f| f.1));
    assert_eq!(shown.as_deref(), Some("l2 exp - Part 1 is being prepared for export. You will receive a notification with its status."));
    // Written off the UI thread: the bell's result comes when the file is done.
    wait_until(cx, window, |cx| cx.try_global::<crate::notifications::Notifications>().is_some_and(|n| !n.items.is_empty()));
    let bell: Vec<(String, String, bool)> = cx.update(|cx| cx.try_global::<crate::notifications::Notifications>().map(|n| n.items.iter().map(|i| (i.title.clone(), i.body.clone(), i.failed)).collect()).unwrap_or_default());
    assert!(bell.contains(&("Export completed".into(), "l2 exp - Part 1 was exported successfully.".into(), false)), "{bell:?}");
    let (tabs, studio) = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.doc.read(cx);
        // In the tab bar's order (the document's tab list is in the order tabs were made).
        let ordered = d.session.doc.ordered_tabs();
        (ordered.iter().map(|t| t.name.clone()).collect::<Vec<_>>(), ordered.iter().position(|t| t.id == v.tab))
    });
    let studio = studio.expect("the Part Studio's tab");
    assert_eq!(tabs.get(studio + 1).map(String::as_str), Some("l2 exp - Part 1.stl"), "{tabs:?}");
}
