//! Company settings › Release management (Help
//! enterprise_settings_release_management, accounts/co-rm-02.png): the
//! settings for objects in the company's documents, saved with [Save release
//! settings] (at the top and at the bottom). Revisions and part numbers:
//! Revision scheme, Unreleased revision suffix (help images
//! revision-part-numbers-01.png, revisionSchemeId.png) and the link to the
//! Numbering schemes page.

use crate::release_dialog::{ReleaseSettings, release_settings, set_release_settings};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, h_flex, v_flex};
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
        Self { edited: saved.clone(), saved, scheme_open: false, custom, suffix, _subs: subs }
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

/// A section: an 18 px / 500 heading over a 1 px `#333` rule from the content
/// edge, the section's content 15 px in (as the account pages' sections).
fn heading(title: &'static str) -> Div {
    let pal = p();
    v_flex().pl(px(15.)).child(h_flex().ml(px(-15.)).h(px(32.)).pl(px(15.)).border_b_1().border_color(pal.text).child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title)))
}

impl Render for ReleasePrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let label = |t: &'static str| div().pt(px(12.)).pb(px(4.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(t);
        // Help: "This button is also duplicated at the bottom of the Release
        // management settings page."
        let save = |id: &'static str, cx: &mut Context<Self>| {
            crate::os_ui::primary_button(id, "Save release settings").disabled(!changed).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if this.edited != this.saved {
                    set_release_settings(&this.edited);
                    this.saved = this.edited.clone();
                    cx.notify();
                }
            }))
        };
        let (save_top, save_bottom) = (save("save-release-settings-top", cx), save("save-release-settings", cx));
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
        let owner = noshape_store::local_user_name();
        v_flex()
            .w_full()
            .child(div().pt(px(16.)).text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(format!("Release management settings for objects in documents owned by {owner}.")))
            .child(h_flex().mt(px(18.)).child(save_top))
            .child(
                heading("Revisions and part numbers")
                    .mt(px(20.))
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
                        div()
                            .id("rm-numbering-link")
                            .mt(px(16.))
                            .text_size(px(14.))
                            .text_color(pal.accent)
                            .cursor_pointer()
                            .hover(|d| d.underline())
                            .child("Click here to go to release management part number generation settings")
                            .on_click(|_, _, cx| crate::company_settings::show(crate::company_settings::Section::NumberingSchemes, cx)),
                    ),
            )
            .child(h_flex().mt(px(30.)).child(save_bottom))
    }
}
