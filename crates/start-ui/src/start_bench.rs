//! T2 bench: the documents page with 2,000 documents in 50 folders (docs/TESTS.md "UI benches"): its frames as it
//! scrolls, filters and selects many rows.

use super::StartView;
use gpui_kit::Focusable as _;
use noshape_ui_test_support::bench;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Keystroke, Modifiers, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, WindowBounds, WindowOptions, point, px, size};
use std::path::PathBuf;

const CASE: &str = "documents page (2,000 documents, 50 folders)";

/// A frame with the whole page drawn again (the test window doesn't redraw the page's cached body when it is told to:
/// a full redraw each step, the most a step can cost).
fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| {
        w.refresh();
        w.draw(cx).clear(cx)
    })
    .unwrap();
}

fn page(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<StartView>) {
    cx.update(gpui_kit::init);
    crate::start_hooks::register(crate::start_hooks::Hooks {
        open_document_window: |_, _, _, _| Ok(()),
        open_document_at: |_, _, _, _| Ok(()),
        open_document_history: |_, _, _| Ok(()),
        build_label: String::new,
        import_dialog: |_, _, _, _| {},
        help_items: |m| m,
    });
    cx.update(|cx| gpui_kit::component::Theme::global_mut(cx).font_family = "Roboto".into());
    let root = crate::file_dialogs::documents_dir();
    let folders: Vec<PathBuf> = (0..50).map(|i| root.join(format!("Project {i:02}"))).collect();
    for f in &folders {
        std::fs::create_dir_all(f).unwrap();
    }
    let root = super::canon(&root);
    let folders: Vec<PathBuf> = folders.iter().map(|f| super::canon(f)).collect();
    let docs: Vec<super::DocEntry> = (0..2000)
        .map(|i| super::DocEntry {
            path: folders[i % 50].join(format!("Bracket assembly {i:04}.noshape")),
            name: format!("Bracket assembly {i:04}"),
            modified: 1_760_000_000_000 + i as i64 * 60_000,
            modified_by: "me".into(),
            owner: "me".into(),
            description: format!("Revision {} of the mounting bracket", i % 7),
            workspace: "Main".into(),
            thumbnail: None,
            recent_rank: (i < 20).then_some(i),
            broken: false,
            trashed: false,
        })
        .collect();
    let (window, view) = cx.update(|cx| {
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| StartView::new(window, cx));
            view = Some(v.clone());
            v
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.source = super::Source::All;
            // List view: the folders are rows above the documents (as tiles, 50 of them fill the page).
            v.view = super::ViewMode::List;
            v.loaded = true;
            v.docs = docs;
            v.dirs = folders.clone();
            v.refresh_body(cx);
        })
    });
    let _ = root;
    draw(cx, window);
    cx.run_until_parked();
    draw(cx, window);
    (window, view)
}

/// The page's steps, each a handler and the frame after it, within a frame (16 ms).
#[test]
#[ignore = "T2: bench, the documents page at 2,000 documents (prints BENCH lines)"]
fn documents_page_at_scale() {
    bench::run(|cx| {
        let (window, view) = page(cx);
        let mut fits = Vec::new();
        let mut step = |name: &str, t: bench::Times| fits.push((name.to_string(), bench::report(CASE, name, &t)));
        let frame = bench::time(20, || {
            cx.update(|cx| view.update(cx, |v, cx| v.refresh_body(cx)));
            draw(cx, window);
        });
        step("a frame of the list", frame);
        // The middle of the list as it was last painted (debug selectors are only in debug builds).
        let clip = cx.update(|cx| view.read(cx).rows_clip.get()).expect("the list painted");
        let over_list = point(px(900.), px((clip.clip.0 + clip.clip.1) / 2.));
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let scroll = bench::time(40, || {
            vcx.simulate_event(ScrollWheelEvent { position: over_list, delta: ScrollDelta::Pixels(point(px(0.), px(-120.))), ..Default::default() });
            vcx.update_window(window, |_, w, cx| {
                w.refresh();
                w.draw(cx).clear(cx)
            })
            .unwrap();
        });
        step("scroll (120 px a step)", scroll);
        // 4,800 px down: the rows on screen are built (the list is in a cached view).
        draw(cx, window);
        let (seen, built) = cx.update(|cx| (super::rows_within(view.read(cx).rows_clip.get(), 0.), view.read(cx).rows_built.get()));
        assert!(seen.0 > 100 && built.0 <= seen.0 && seen.1 <= built.1, "rows {seen:?} shown, {built:?} built");
        // A typed filter, a key at a time, then cleared.
        cx.update_window(window, |_, w, cx| {
            let fh = view.read(cx).search.focus_handle(cx);
            fh.focus(w, cx);
        })
        .unwrap();
        let keys = ["b", "r", "a", "c", "k", "e", "t", "space", "1", "backspace", "backspace", "backspace"];
        let mut k = keys.iter().cycle();
        let typing = bench::time(24, || {
            let key = *k.next().unwrap();
            cx.update_window(window, |_, w, cx| {
                let mut ks = Keystroke::parse(key).unwrap();
                if key.len() == 1 {
                    ks.key_char = Some(key.into());
                }
                w.dispatch_keystroke(ks, cx);
                w.refresh();
                w.draw(cx).clear(cx);
            })
            .unwrap();
        });
        step("filter: a key typed", typing);
        cx.update(|cx| view.update(cx, |v, cx| {
            v.query.clear();
            v.refresh_body(cx);
        }));
        draw(cx, window);
        // Shift+click a range of 1,000 rows, then Ctrl+A (every row selected), then scroll with them selected.
        let order = cx.update(|cx| view.read(cx).page_order());
        let shift = Modifiers { shift: true, ..Default::default() };
        let mut i = 0;
        let range = bench::time(10, || {
            i += 1;
            cx.update(|cx| view.update(cx, |v, cx| {
                v.click_row(&order[50], &Modifiers::default());
                v.click_row(&order[(1000 + i) % order.len()], &shift);
                v.refresh_body(cx);
            }));
            draw(cx, window);
        });
        step("shift+click: 1,000 rows selected", range);
        let all = bench::time(10, || {
            cx.update(|cx| view.update(cx, |v, cx| {
                v.select_all_rows(cx);
                v.refresh_body(cx);
            }));
            draw(cx, window);
        });
        step("select all (2,050 rows)", all);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let scroll_sel = bench::time(20, || {
            vcx.simulate_event(ScrollWheelEvent { position: over_list, delta: ScrollDelta::Pixels(point(px(0.), px(120.))), ..Default::default() });
            vcx.update_window(window, |_, w, cx| {
                w.refresh();
                w.draw(cx).clear(cx)
            })
            .unwrap();
        });
        step("scroll with every row selected", scroll_sel);
        let slow: Vec<&str> = fits.iter().filter(|(_, ok)| !ok).map(|(n, _)| n.as_str()).collect();
        assert!(slow.is_empty(), "over a frame: {slow:?}");
    });
}
