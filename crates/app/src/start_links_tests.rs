//! The documents page's links to where documents are on this computer (user 11-07, a Windows user who couldn't find
//! theirs): the footer's documents folder opens in the OS's file manager; a document's Location shows the file in its
//! folder.

use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Context, IntoElement, Modifiers, ParentElement as _, Point, Render, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, px, size};
use noshape_start_ui::start_window::folder_link;
use std::path::PathBuf;

struct Links {
    folder: PathBuf,
    file: PathBuf,
}

impl Render for Links {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(folder_link("documents-folder", self.folder.clone(), false)).child(folder_link("details-location", self.file.clone(), true))
    }
}

fn host(cx: &mut TestAppContext, folder: PathBuf, file: PathBuf) -> AnyWindowHandle {
    cx.update(gpui_kit::init);
    let bounds = Bounds { origin: Point::default(), size: size(px(800.), px(400.)) };
    let w = cx.update(|cx| cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |_, cx| cx.new(|_| Links { folder, file })).unwrap().into());
    cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
    w
}

#[gpui::test]
fn the_documents_folder_link_opens_the_folder_and_a_documents_shows_its_file(cx: &mut TestAppContext) {
    let folder = std::env::temp_dir().join("noshape-start-links").join("noshape");
    let file = folder.join("AppleWatchDock.noshape");
    let w = host(cx, folder.clone(), file.clone());
    let mut vcx = VisualTestContext::from_window(w, cx);
    let link = vcx.debug_bounds("documents-folder").expect("the footer's folder link");
    vcx.simulate_click(link.center(), Modifiers::default());
    assert_eq!(cx.opened_paths(), vec![folder], "the folder opens in the file manager");
    assert!(cx.revealed_paths().is_empty());
    let mut vcx = VisualTestContext::from_window(w, cx);
    let link = vcx.debug_bounds("details-location").expect("the document's Location link");
    vcx.simulate_click(link.center(), Modifiers::default());
    assert_eq!(cx.revealed_paths(), vec![file], "the document shows in its folder");
}
