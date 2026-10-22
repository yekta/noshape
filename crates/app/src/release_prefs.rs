//! Account preferences sections owned by the release and material work:
//! "Release management" (Onshape's Company settings › Release management ›
//! Revisions and part numbers: Revision scheme, Unreleased revision suffix;
//! help images revision-part-numbers-01.png, revisionSchemeId.png) and
//! "Material libraries" (the libraries added from the Material panel, with
//! Remove; Help: "Manage my material libraries in account preferences").

use crate::release_dialog::{ReleaseSettings, release_settings, set_release_settings};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::release::RevisionScheme;

pub struct ReleasePrefsView {
    saved: ReleaseSettings,
    edited: ReleaseSettings,
    scheme_open: bool,
    /// Custom scheme labels, comma separated.
    custom: Entity<InputState>,
    suffix: Entity<InputState>,
    numbering: Entity<crate::numbering_prefs::NumberingPrefsView>,
    _subs: Vec<Subscription>,
}

impl ReleasePrefsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = release_settings();
        let custom_text = match &saved.scheme {
            RevisionScheme::Custom(l) => l.join(", "),
            _ => String::new(),
        };
        let custom = cx.new(|cx| InputState::new(window, cx).placeholder("A, B, C…").default_value(custom_text));
        let suffix = cx.new(|cx| InputState::new(window, cx).default_value(saved.unreleased_suffix.clone()));
        let mut subs = Vec::new();
        for st in [&custom, &suffix] {
            subs.push(cx.subscribe_in(st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.sync(cx);
                }
            }));
        }
        let numbering = cx.new(|cx| crate::numbering_prefs::NumberingPrefsView::new(window, cx));
        Self { edited: saved.clone(), saved, scheme_open: false, custom, suffix, numbering, _subs: subs }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        if let RevisionScheme::Custom(_) = self.edited.scheme {
            let labels: Vec<String> = self.custom.read(cx).value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            self.edited.scheme = RevisionScheme::Custom(labels);
        }
        self.edited.unreleased_suffix = self.suffix.read(cx).value().to_string();
        cx.notify();
    }
}

/// A Preferences section: the page's 18 px / 500 heading over a 1 px `#333`
/// rule from the content edge, the section's content 15 px in (live prefs/04–05).
fn heading(title: &'static str) -> Div {
    let pal = p();
    v_flex().pl(px(15.)).child(h_flex().ml(px(-15.)).h(px(32.)).pl(px(15.)).border_b_1().border_color(pal.text).child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title)))
}

/// Material libraries table: three 425 px columns (cells 22 px in), the
/// action column right-aligned (live prefs/05).
const LIB_COL: f32 = 425.;
const LIB_RULE: u32 = 0xd0dbe5;

impl Render for ReleasePrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let libs = crate::material_libraries::added();
        // Live's 425 px columns (1919 px window) share the width here, so the
        // Remove buttons stay in view in narrower windows.
        let cell = |t: String| div().flex_1().max_w(px(LIB_COL)).min_w_0().pl(px(22.)).pr(px(8.)).truncate().child(t);
        // Live (prefs/05): every row has a small primary Remove (26 px, 12 px
        // text, 15 px from the right), the Onshape library's too.
        let remove = |id: SharedString, cx: &mut Context<Self>, f: Box<dyn Fn()>| {
            div()
                .id(id)
                .mr(px(15.))
                .h(px(26.))
                .px(px(8.))
                .flex_none()
                .flex()
                .items_center()
                .rounded(px(3.5))
                .bg(rgb(0x1651b0))
                .text_size(px(12.))
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0x2b65c4)))
                .child("Remove")
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    f();
                    cx.notify();
                }))
        };
        // The header has the same trailing spacer and action column, so
        // the cells line up with their titles at every width.
        let action = || h_flex().flex_none().w(px(90.));
        let row = |name: String, document: String, owner: String, button: Stateful<Div>| h_flex().h(px(48.)).text_size(px(14.)).text_color(pal.text).child(cell(name)).child(cell(document)).child(cell(owner)).child(div().flex_1()).child(action().justify_end().child(button));
        let mut lib_rows: Vec<AnyElement> = Vec::new();
        if crate::material_libraries::onshape_listed() {
            let b = remove("matlib-remove-onshape".into(), cx, Box::new(|| crate::material_libraries::set_onshape_listed(false)));
            lib_rows.push(row("Onshape Material Library".into(), "std.mat".into(), "Onshape".into(), b).into_any_element());
        }
        for (i, l) in libs.iter().enumerate() {
            let b = remove(
                SharedString::from(format!("matlib-remove-{i}")),
                cx,
                Box::new(move || {
                    let mut list = crate::material_libraries::added();
                    if i < list.len() {
                        list.remove(i);
                        crate::material_libraries::set_added(&list);
                    }
                }),
            );
            lib_rows.push(row(l.name.clone(), l.document.clone(), l.owner.clone(), b).into_any_element());
        }
        let label = |t: &'static str| div().pt(px(12.)).pb(px(4.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(t);
        let current = self.edited.scheme.label();
        let options = RevisionScheme::CHOICES.iter().enumerate().map(|(i, c)| {
            div()
                .id(SharedString::from(format!("rev-scheme-{i}")))
                .px(px(10.))
                .py(px(4.))
                .text_size(px(14.))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0x1e90ff)).text_color(rgb(0xffffff)))
                .child(*c)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.edited.scheme = match i {
                        0 => RevisionScheme::Alphabetical,
                        1 => RevisionScheme::Numeric { width: 1 },
                        2 => RevisionScheme::Numeric { width: 2 },
                        3 => RevisionScheme::Numeric { width: 3 },
                        _ => RevisionScheme::Custom(Vec::new()),
                    };
                    this.scheme_open = false;
                    this.sync(cx);
                }))
        });
        let custom = matches!(self.edited.scheme, RevisionScheme::Custom(_));
        v_flex()
            .w_full()
            .child(
                heading("Release management")
                    .mt(px(38.))
                    .child(div().pt(px(10.)).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Revisions and part numbers"))
                    .child(label("Revision scheme"))
                    .child(
                        v_flex()
                            .w(px(600.))
                            .child(
                                h_flex()
                                    .id("rev-scheme")
                                    .h(px(34.))
                                    .px(px(12.))
                                    .border_1()
                                    .rounded(px(4.))
                                    .border_color(rgb(0xcccccc))
                                    .cursor_pointer()
                                    .child(div().flex_1().text_size(px(14.)).child(current))
                                    .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.scheme_open = !this.scheme_open;
                                        cx.notify();
                                    })),
                            )
                            .when(self.scheme_open, |d| d.child(v_flex().border_1().border_color(rgb(0x767676)).bg(rgb(0xffffff)).children(options))),
                    )
                    .when(custom, |d| d.child(label("Custom revisions")).child(gpui_kit::base::input::input_frame(div().w(px(600.)).border_1().rounded(px(4.)).border_color(rgb(0xcccccc)).px(px(6.)).child(Input::new(&self.custom).appearance(false)))))
                    .child(label("Unreleased revision suffix:"))
                    .child(gpui_kit::base::input::input_frame(div().w(px(188.)).border_1().rounded(px(4.)).border_color(rgb(0xcccccc)).px(px(6.)).child(Input::new(&self.suffix).appearance(false))))
                    .child(
                        h_flex().mt(px(20.)).child(
                            div()
                                .id("save-release-settings")
                                .px(px(14.))
                                .py(px(7.))
                                .rounded(px(5.))
                                .text_size(px(14.))
                                .text_color(rgb(0xffffff))
                                .bg(rgb(0x1651b0))
                                .when(!changed, |d| d.opacity(0.4))
                                .when(changed, |d| d.cursor_pointer())
                                .child("Save release settings")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if this.edited != this.saved {
                                        set_release_settings(&this.edited);
                                        this.saved = this.edited.clone();
                                        cx.notify();
                                    }
                                })),
                        ),
                    ),
            )
            .child(self.numbering.clone())
            .child(
                heading("Material libraries")
                    .mt(px(38.))
                    .child(
                        v_flex()
                            .mt(px(18.))
                            // Live: the table stops 15 px before the rule's end.
                            .mr(px(15.))
                            .child(h_flex().h(px(44.)).border_b_1().border_color(rgb(LIB_RULE)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(cell("Library".into())).child(cell("Document".into())).child(cell("Owner".into())).child(div().flex_1()).child(action()))
                            .children(lib_rows),
                    )
                    // Live (prefs/05): [Add material library…] under the table
                    // opens the "Add material library" modal.
                    // Live 1902: a primary (blue) button, 34 px under the last row.
                    .child(h_flex().mt(px(34.)).mb(px(38.)).child(crate::os_ui::primary_button("matlib-add-library", "Add material library…").on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                        let view = cx.entity();
                        crate::material_libraries::open(move |_, _, cx| view.update(cx, |_, cx| cx.notify()), window, cx);
                    })))),
            )
    }
}
