//! A running job's entry in the Notifications bell (the documents page's import, live "STEP import flow"): its
//! steps as they run, the last row's link, and a failure.

use crate::notifications::{self as bell, Step, StepState};
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Context, IntoElement, Modifiers, ParentElement as _, Point, Render, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, px, size};
use std::cell::Cell;
use std::rc::Rc;

struct Panel;

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(bell::panel(|_, _| {}, cx))
    }
}

fn host(cx: &mut TestAppContext) -> AnyWindowHandle {
    let bounds = Bounds { origin: Point::default(), size: size(px(500.), px(500.)) };
    let w = cx.update(|cx| cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |_, cx| cx.new(|_| Panel)).unwrap().into());
    cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
    w
}

fn item(cx: &mut TestAppContext, id: u64) -> bell::Notification {
    cx.update(|cx| cx.global::<bell::Notifications>().items.iter().find(|i| i.id == id).cloned().unwrap())
}

#[gpui::test]
fn an_import_entry_walks_its_steps_and_links_to_the_document(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    // The click's feedback: an unread entry at once.
    let id = cx.update(|cx| bell::start_job(cx, "Importing a.step…", vec![Step::running("Uploading…")]));
    assert_eq!(cx.update(|cx| bell::unread(cx)), 1);
    let i = item(cx, id);
    assert_eq!((i.title.as_str(), i.steps[0].state), ("Importing a.step…", StepState::Running));
    let opened = Rc::new(Cell::new(false));
    let o = opened.clone();
    let action: bell::LinkAction = Rc::new(move |_, _| o.set(true));
    cx.update(|cx| bell::update_job(cx, id, "Import a.step", vec![Step::done("Uploaded"), Step::done("Translated"), Step::done("Post-processing done")], false, Some(("Created link to a.step".into(), action))));
    let i = item(cx, id);
    assert_eq!(i.title, "Import a.step");
    assert!(i.steps.iter().all(|s| s.state == StepState::Done));
    assert_eq!(i.link.as_deref(), Some("Created link to a.step"));
    // Opened in the panel, its link opens the document.
    cx.update(|cx| cx.global_mut::<bell::Notifications>().items.iter_mut().for_each(|i| i.open = true));
    let w = host(cx);
    let mut vcx = VisualTestContext::from_window(w, cx);
    let link = vcx.debug_bounds(Box::leak(format!("notification-link-{id}").into_boxed_str())).expect("the link row");
    vcx.simulate_click(link.center(), Modifiers::default());
    assert!(opened.get(), "the link's action ran");
}

#[gpui::test]
fn a_failed_job_says_why(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let id = cx.update(|cx| bell::start_job(cx, "Importing b.step…", vec![Step::running("Uploading…")]));
    cx.update(|cx| bell::update_job(cx, id, "Import b.step failed", vec![Step::failed("b.step could not be read: no answer from the file system in 30 s"), Step::failed("No post-processing")], true, None));
    let i = item(cx, id);
    assert!(i.failed && i.link.is_none() && i.unread);
    assert_eq!(i.steps[0].text, "b.step could not be read: no answer from the file system in 30 s");
}
