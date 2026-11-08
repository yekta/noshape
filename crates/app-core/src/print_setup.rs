//! ☰ › Print… in a Part Studio or an Assembly (live studios 2349): the "Print setup" flyout beside the feature list,
//! the dashed frame over the graphics area, Download image (the framed view at about 300 dpi, named after the tab)
//! and Print (that image on a page of the paper, opened for the system's print dialog). The studios keep the open
//! panel's setup and draw [`panel`] and [`frame`] in their graphics area.

use crate::theme::{p, with_alpha};
use crate::viewport::Viewport;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_scene::print as pr;

pub use noshape_scene::print::PrintSetup;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Paper(usize),
    Orientation(usize),
    Print,
    Download,
    Close,
}

/// The open panel's state, kept by the studio view: the setup shown and the keyboard (live: the Paper select holds
/// it when the panel opens; Esc, the studio's Cancel, closes the panel).
pub struct PrintPanel {
    pub setup: PrintSetup,
    pub focus: FocusHandle,
}

impl PrintPanel {
    /// The panel opened: the last setup chosen, the Paper select focused.
    pub fn open(window: &mut Window, cx: &mut App) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        PrintPanel { setup: PrintSetup::load(), focus }
    }

    /// A select's change, remembered for next time (live: also in another tab and after a reload).
    pub fn apply(&mut self, a: Action) {
        match a {
            Action::Paper(i) => self.setup.paper = pr::PAPERS.get(i).map_or(pr::PAPERS[0].0, |p| p.0).into(),
            Action::Orientation(i) => self.setup.orientation = pr::ORIENTATIONS.get(i).copied().unwrap_or(pr::ORIENTATIONS[0]).into(),
            _ => return,
        }
        self.setup.save();
    }
}

/// The flyout (live dom/print-setup: 220 × 132 at the graphics area's top left, as a feature dialog): "Print setup"
/// and ✕, the Paper and Orientation selects, Print and Download image, ?.
pub fn panel<V: 'static>(state: &PrintPanel, view: &Entity<V>, on: fn(&mut V, Action, &mut Window, &mut Context<V>)) -> AnyElement {
    let pal = p();
    let act = |a: Action| {
        let v = view.clone();
        move |_: &ClickEvent, w: &mut Window, cx: &mut App| v.update(cx, |this, cx| on(this, a, w, cx))
    };
    let pick = |f: fn(usize) -> Action| {
        let v = view.clone();
        move |i: usize, w: &mut Window, cx: &mut App| v.update(cx, |this, cx| on(this, f(i), w, cx))
    };
    let paper = state.setup.paper_index().unwrap_or(0);
    let orientation = usize::from(state.setup.landscape());
    note_state(&state.setup);
    let small = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .map(|d| crate::ui_script::mark(d, || format!("button:{label}")))
            .h(px(20.))
            .px(px(5.))
            .flex()
            .items_center()
            .rounded(px(3.))
            .border_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .bg(gpui_kit::rgb(0xffffff))
            .text_size(px(11.))
            .text_color(pal.text)
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(label)
    };
    let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }];
    // (Esc: the studio's Cancel closes the panel first.)
    v_flex()
        .id("print-setup")
        .occlude()
        .track_focus(&state.focus)
        .w(px(220.))
        .h(px(132.))
        .bg(with_alpha(gpui_kit::rgb(0xffffff), 0.95))
        .shadow(shadow)
        .text_color(pal.text)
        .child(
            h_flex()
                .h(px(29.))
                .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Print setup"))
                .child(
                    div()
                        .id("print-close")
                        .map(|d| crate::ui_script::mark(d, || "button:Close print setup".into()))
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .opacity(0.5)
                        .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 23., gpui_kit::rgb(0x333333)))
                        .on_click(act(Action::Close)),
                ),
        )
        .child(div().w(px(194.)).child(crate::os_ui::select_field_flat("print-paper", pr::PAPERS.iter().map(|p| p.0.to_string()).collect(), Vec::new(), usize::MAX, paper, pick(Action::Paper))))
        .child(div().w(px(194.)).child(crate::os_ui::select_field_flat("print-orientation", pr::ORIENTATIONS.iter().map(|o| o.to_string()).collect(), Vec::new(), usize::MAX, orientation, pick(Action::Orientation))))
        .child(h_flex().mt(px(6.)).pl(px(6.)).gap(px(8.)).child(small("print-print", "Print").on_click(act(Action::Print))).child(small("print-download", "Download image").on_click(act(Action::Download))))
        .child(div().flex_1())
        .child(
            h_flex().h(px(22.)).justify_end().pr(px(4.)).child(
                div()
                    .id("print-help")
                    .map(|d| crate::ui_script::mark(d, || "button:Print setup help".into()))
                    .size(px(14.))
                    .cursor_pointer()
                    .child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa)))
                    .on_click(|_, _, cx| crate::help::open("Printing", cx)),
            ),
        )
        .into_any_element()
}

/// The dashed frame over the graphics area `viewport` fills: what is printed.
pub fn frame(setup: &PrintSetup, viewport: &Entity<Viewport>, cx: &App) -> AnyElement {
    let area = viewport.read(cx).size();
    let (x, y, w, h) = setup.frame((area.x, area.y));
    div().absolute().left(px(x as f32)).top(px(y as f32)).w(px(w as f32)).h(px(h as f32)).border_1().border_dashed().border_color(gpui_kit::rgb(0x333333)).into_any_element()
}

/// Live's recorder: the flyout as a panel, the Paper select holding the keyboard.
fn note_state(setup: &PrintSetup) {
    crate::ui_script::note_dialog(|| serde_json::json!({ "kind": "panel", "title": "Print setup", "text": ["Print", "Download image"] }));
    crate::ui_script::note_focus(|| serde_json::json!({ "kind": "field", "label": null, "text": setup.paper, "selection": [null, null], "all": false }));
}

/// Download image (`pdf` false: a PNG saved where the user says, named after the tab) or Print (`pdf`: the page
/// opened in the system viewer to print), rendered off the UI thread from what `viewport` shows; failures go to the
/// notifications.
pub fn export(pdf: bool, viewport: &Entity<Viewport>, setup: &PrintSetup, tab: &str, window: &mut Window, cx: &mut App) {
    let vp = viewport.read(cx);
    let (gpu, scene, camera, style) = vp.print_source();
    let area = vp.size();
    let (.., frame_h) = setup.frame((area.x, area.y));
    let camera = pr::frame_camera(&camera, area.y, frame_h);
    let (iw, ih) = setup.image_size();
    let (rw, rh, ss) = pr::render_plan(iw, ih);
    let (style, page) = (pr::print_style(&style), setup.clone());
    let render = cx.background_spawn(async move {
        let png = gpu.render_png_supersampled(scene, camera, style, (rw, rh), ss, pr::line_scale(rh, frame_h)).await.map_err(|e| format!("{e:#}"))?;
        let png = pr::to_size(png, iw, ih)?;
        if pdf { pr::page_pdf(&png, &page) } else { Ok(png) }
    });
    let failed = |cx: &mut App, what: &str, e: String| crate::notifications::push(cx, format!("{what} failed"), e, true);
    if pdf {
        let path = std::env::temp_dir().join("noshape-print").join(pr::file_name(tab, "pdf"));
        cx.spawn(async move |cx| {
            let written = render.await.and_then(|bytes| {
                std::fs::create_dir_all(path.parent().unwrap_or(&path)).map_err(|e| e.to_string())?;
                std::fs::write(&path, bytes).map_err(|e| e.to_string())
            });
            cx.update(|cx| match written {
                Ok(()) => cx.open_url(&format!("file://{}", path.display())),
                Err(e) => failed(cx, "Print", e),
            });
        })
        .detach();
        return;
    }
    crate::file_dialogs::request_save_path("Download image", pr::file_name(tab, "png"), "png", window, cx, move |path, cx| {
        cx.spawn(async move |cx| {
            let written = render.await.and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
            if let Err(e) = written {
                cx.update(|cx| failed(cx, "Download image", e));
            }
        })
        .detach();
    });
}
