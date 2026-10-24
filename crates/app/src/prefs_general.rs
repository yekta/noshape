//! My account › Preferences: Language, Theming, Environment profile settings
//! and Onshape Labs (live prefs/00-y0, 1860–1893, dk-900…904; behaviours "My
//! account › Preferences" and "ui2 10-22 05:00: Preferences"), plus the page's
//! message bubble (live: top centre, "Theme updated.").

use crate::os_ui as os;
use crate::preferences_window::{ListPrefs, PreferencesView};
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

fn tr(s: &str) -> SharedString {
    crate::i18n::tr(s).into()
}

// ----------------------------------------------------------------- toast

/// The Preferences page's success bubble and its generation.
#[derive(Default)]
struct PrefsToast(Option<String>, u64);

impl Global for PrefsToast {}

/// Show `text` in the page's success bubble for a few seconds.
pub fn toast(text: impl Into<String>, cx: &mut App) {
    let generation = cx.try_global::<PrefsToast>().map_or(0, |t| t.1) + 1;
    cx.set_global(PrefsToast(Some(text.into()), generation));
    cx.refresh_windows();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(5)).await;
        cx.update(|cx| {
            if cx.try_global::<PrefsToast>().is_some_and(|t| t.1 == generation) {
                cx.set_global(PrefsToast(None, generation));
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

/// The bubble, top centre (live: 8 px down), if one is showing.
pub fn toast_overlay(cx: &App) -> Option<AnyElement> {
    let text = cx.try_global::<PrefsToast>()?.0.clone()?;
    Some(
        div()
            .absolute()
            .top(px(8.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(os::message_bubble("prefs-toast", os::BubbleKind::Success, text, |_, _, cx| cx.set_global(PrefsToast::default()), cx))
            .into_any_element(),
    )
}

// ----------------------------------------------------------------- common

/// A section: heading, then the body 15 px in (as Decimal format).
fn section(title: &str, extra: Option<AnyElement>, body: impl IntoElement) -> Div {
    v_flex().mb(px(38.)).child(PreferencesView::heading(tr(title), extra)).child(v_flex().pl(px(15.)).pt(px(14.)).child(body))
}

/// A grey (?) with a hover text (live: the headings' help popovers).
fn help(id: &'static str, text: &'static str, cx: &App) -> AnyElement {
    div().id(id).child(crate::assets::icon("help-gray", 18., cx)).tooltip(crate::ui::tooltip(tr(text))).into_any_element()
}

/// A label over a control (14 px; the control's top 25 px under the
/// label's, live prefs/00, 02).
fn labelled(label: &str, control: impl IntoElement) -> Div {
    v_flex().gap(px(9.)).child(div().text_size(px(14.)).text_color(p().text).child(tr(label))).child(control)
}

/// A 250 × 34 browser select, or its greyed look when `enabled` is false.
fn select(id: &'static str, options: Vec<String>, at: usize, enabled: bool, w: f32, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> AnyElement {
    if enabled {
        return div().w(px(w)).child(os::select_field(id, options, at, on_select)).into_any_element();
    }
    let pal = p();
    h_flex()
        .w(px(w))
        .h(px(34.))
        .pl(px(11.))
        .pr(px(10.))
        .rounded(px(5.25))
        .border_1()
        .border_color(rgb(0xaaaaaa))
        .bg(rgb(0xeaeaea))
        .text_size(px(14.))
        .text_color(pal.text)
        .child(div().flex_1().truncate().child(options.get(at).cloned().unwrap_or_default()))
        .child(os::glyph(os::GLYPH_CARET, 8., rgb(0x777777)))
        .into_any_element()
}

// ----------------------------------------------------------------- Language

pub struct LanguagePrefsView {
    saved: &'static str,
    edited: &'static str,
}

impl Default for LanguagePrefsView {
    fn default() -> Self {
        let cur = crate::i18n::current();
        Self { saved: cur, edited: cur }
    }
}

impl Render for LanguagePrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changed = self.edited != self.saved;
        let at = crate::i18n::LANGUAGES.iter().position(|(c, _)| *c == self.edited).unwrap_or(2);
        let view = cx.entity();
        // The languages are listed by their own names in every language
        // (live: "Deutsch", "日本語"), so never translated.
        let pick = div().w(px(170.)).child(os::user_select_field("pref-language", crate::i18n::LANGUAGES.iter().map(|(_, n)| n.to_string()).collect(), at, move |i, _, cx| {
            view.update(cx, |this, cx| {
                this.edited = crate::i18n::LANGUAGES[i].0;
                cx.notify();
            })
        }));
        section(
            "Language",
            None,
            v_flex()
                .child(labelled("Display language", pick))
                .child(h_flex().mt(px(25.)).child(PreferencesView::save_button("pref-language-save", tr("Save language"), changed).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if this.edited == this.saved {
                        return;
                    }
                    crate::i18n::set_current(this.edited);
                    let mut prefs = ListPrefs::load();
                    prefs.language = Some(this.edited.to_string());
                    prefs.save();
                    this.saved = this.edited;
                    // Live: no reload; the page re-renders in the new language
                    // and says so in it ("Langue mise à jour.").
                    toast(crate::i18n::tr("Language updated."), cx);
                })))),
        )
    }
}

// ----------------------------------------------------------------- Theming

pub struct ThemingPrefsView {
    saved: bool,
    edited: bool,
}

impl Default for ThemingPrefsView {
    fn default() -> Self {
        let dark = ListPrefs::load().dark_mode;
        Self { saved: dark, edited: dark }
    }
}

/// Onshape's (Bootstrap) radio, live prefs/00 and dk-904: 14 px; off a 1 px
/// `#ababab` ring (`#878787` in dark).
fn radio(on: bool) -> Div {
    let dark = noshape_scene::palette::is_dark();
    let d = div().size(px(14.)).flex_none().rounded_full().flex().items_center().justify_center();
    if on {
        // Bootstrap's checked radio, both themes: a #0d6efd disc, white dot.
        d.bg(gpui_kit::rgb_static(0x0d6efd)).child(div().size(px(6.)).rounded_full().bg(gpui_kit::rgb_static(0xffffff)))
    } else {
        d.border_1().border_color(gpui_kit::rgb_static(if dark { 0x878787 } else { 0xababab }))
    }
}

impl Render for ThemingPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let edited = self.edited;
        let option = |id: &'static str, dark: bool, label: &'static str, note: &'static str, extra: Option<AnyElement>, cx: &mut Context<Self>| {
            v_flex()
                .child(
                    h_flex()
                        .id(id)
                        .h(px(21.))
                        .gap(px(8.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(radio(edited == dark))
                        .child(tr(label))
                        .children(extra)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.edited = dark;
                            cx.notify();
                        })),
                )
                .child(div().pl(px(22.)).pt(px(2.)).text_size(px(12.)).text_color(gpui_kit::rgb_static(if noshape_scene::palette::is_dark() { 0xbbbbbb } else { 0x777777 })).child(tr(note)))
        };
        // Live dk-900: a blue "beta" pill and a (?) after "Dark mode".
        let beta = h_flex()
            .gap(px(6.))
            .child(div().px(px(5.)).h(px(16.)).flex().items_center().rounded(px(8.)).bg(gpui_kit::rgb_static(0x1651b0)).text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("beta"))
            .child(help("pref-dark-help", "Dark mode is a limited feature and is currently not available in Drawings, PCB Studio and any external apps.", cx))
            .into_any_element();
        section(
            "Theming",
            None,
            v_flex()
                .pt(px(2.))
                .gap(px(16.))
                .child(option("pref-light", false, "Light mode", "Default color scheme for Onshape featuring dark elements on light backgrounds.", None, cx))
                .child(option("pref-dark", true, "Dark mode", "Low luminance color scheme mainly featuring light elements on dark backgrounds.", Some(beta), cx))
                .child(h_flex().child(PreferencesView::save_button("pref-theme-save", tr("Save theme"), changed).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if this.edited == this.saved {
                        return;
                    }
                    let mut prefs = ListPrefs::load();
                    prefs.dark_mode = this.edited;
                    prefs.save();
                    this.saved = this.edited;
                    // Live: applies at once, no reload.
                    crate::theme::set_dark(this.edited);
                    // The viewports rebuild their scenes in the new colors.
                    crate::graphics_prefs::set(crate::graphics_prefs::get(cx), false, cx);
                    crate::apply_kit_theme(cx);
                    cx.refresh_windows();
                    toast(crate::i18n::tr("Theme updated."), cx);
                })))),
        )
    }
}

// ----------------------------------------------------------------- Environment profiles

/// "Match pixel density on high resolution displays (can affect performance)".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelDensity {
    #[default]
    Automatic,
    On,
    Off,
}

impl PixelDensity {
    const ALL: [PixelDensity; 3] = [PixelDensity::Automatic, PixelDensity::On, PixelDensity::Off];

    fn label(self) -> &'static str {
        match self {
            PixelDensity::Automatic => "Automatic (default)",
            PixelDensity::On => "On",
            PixelDensity::Off => "Off",
        }
    }
}

/// A user environment profile: live holds only the pixel density in one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvProfile {
    pub name: String,
    #[serde(default)]
    pub density: PixelDensity,
}

/// The default profile's name.
const DEFAULT_PROFILE: &str = "Onshape (default)";

#[derive(Default)]
pub struct EnvProfilePrefsView {
    /// The density being edited for the selected profile (None: unchanged).
    density: Option<PixelDensity>,
}

impl EnvProfilePrefsView {
    fn create_dialog(view: Entity<Self>, window: &mut Window, cx: &mut App) {
        let name = cx.new(|cx| InputState::new(window, cx));
        // Focused once the dialog has opened (it takes the focus itself first).
        let focused = std::rc::Rc::new(std::cell::Cell::new(false));
        window
            .subscribe(&name, cx, |_, ev: &InputEvent, window, _| {
                if let InputEvent::Change = ev {
                    window.refresh();
                }
            })
            .detach();
        window.open_dialog(cx, move |dialog, window, cx| {
            if !focused.replace(true) {
                name.update(cx, |s, cx| s.focus(window, cx));
            }
            let text = name.read(cx).value().trim().to_string();
            let taken = text == DEFAULT_PROFILE || ListPrefs::load().env_profiles.iter().any(|p| p.name == text);
            // Live 1862–1863: the message shows at once; Create stays pale until valid.
            let error = if text.is_empty() {
                Some("A unique profile name is required.")
            } else if taken {
                Some("A profile with this name already exists.")
            } else {
                None
            };
            let (name2, view2) = (name.clone(), view.clone());
            os::modal_doc_page(dialog, tr("New environment profile"), 600.)
                .child(
                    // Live 1862: the box 12 px under the header, the message 6 px under it.
                    os::modal_body()
                        .pt(px(3.))
                        .gap(px(2.))
                        .child(os::input_h(Input::new(&name), 34.))
                        .when_some(error, |d, e| d.child(div().text_size(px(14.)).text_color(rgb(0xaa252d)).child(tr(e)))),
                )
                .footer(
                    os::modal_footer()
                        // Live: the primary blue at 40 % until the name is valid.
                        .child(os::modal_ok(os::primary_button("env-create", tr("Create")).when(error.is_some(), |b| b.opacity(0.4))))
                        .child(os::modal_cancel(os::cancel_button("env-create-cancel", tr("Cancel"), cx))),
                )
                .on_ok(move |_, _, cx| {
                    let text = name2.read(cx).value().trim().to_string();
                    let mut prefs = ListPrefs::load();
                    if text.is_empty() || text == DEFAULT_PROFILE || prefs.env_profiles.iter().any(|p| p.name == text) {
                        return false;
                    }
                    prefs.env_profiles.push(EnvProfile { name: text.clone(), density: PixelDensity::Automatic });
                    prefs.env_profile = Some(text.clone());
                    prefs.save();
                    view2.update(cx, |this, cx| {
                        this.density = None;
                        cx.notify();
                    });
                    toast(crate::i18n::tr("Profile __name__ is created.").replace("__name__", &text), cx);
                    true
                })
        });
    }

    fn delete_dialog(view: Entity<Self>, profile: String, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let (view2, profile2) = (view.clone(), profile.clone());
            let body = crate::i18n::tr("This will permanently delete profile __name__ from your preferences and cannot be restored.");
            let (before, after) = body.split_once("__name__").unwrap_or((body.as_str(), ""));
            os::modal_doc_page(dialog, tr("Delete profile?"), 600.)
                .child(os::modal_body().child(div().child(before.to_string()).child(div().font_weight(FontWeight::BOLD).child(profile.clone())).child(after.to_string()).flex().flex_wrap()))
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("env-delete-ok", tr("OK")))).child(os::modal_cancel(os::cancel_button("env-delete-cancel", tr("Cancel"), cx))))
                .on_ok(move |_, _, cx| {
                    let mut prefs = ListPrefs::load();
                    prefs.env_profiles.retain(|p| p.name != profile2);
                    prefs.env_profile = None;
                    prefs.save();
                    view2.update(cx, |this, cx| {
                        this.density = None;
                        cx.notify();
                    });
                    toast(crate::i18n::tr("Profile __name__ is deleted.").replace("__name__", &profile2), cx);
                    true
                })
        });
    }
}

impl Render for EnvProfilePrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = ListPrefs::load();
        let selected = prefs.env_profile.clone().filter(|n| prefs.env_profiles.iter().any(|p| &p.name == n));
        let saved_density = selected.as_ref().and_then(|n| prefs.env_profiles.iter().find(|p| &p.name == n)).map_or(PixelDensity::Automatic, |p| p.density);
        let density = self.density.unwrap_or(saved_density);
        let changed = selected.is_some() && density != saved_density;
        let mut names: Vec<String> = vec![DEFAULT_PROFILE.to_string()];
        names.extend(prefs.env_profiles.iter().map(|p| p.name.clone()));
        let at = selected.as_ref().and_then(|n| names.iter().position(|x| x == n)).unwrap_or(0);
        let view = cx.entity();
        // Picking a profile applies it at once (live 1867: "Profile … is selected.").
        let pick = select("env-profile", names.clone(), at, true, 250., {
            let names = names.clone();
            move |i, _, cx| {
                let mut prefs = ListPrefs::load();
                prefs.env_profile = (i > 0).then(|| names[i].clone());
                prefs.save();
                view.update(cx, |this, cx| {
                    this.density = None;
                    cx.notify();
                });
                toast(crate::i18n::tr("Profile __name__ is selected.").replace("__name__", &names[i]), cx);
            }
        });
        let view_c = cx.entity();
        let view_d = cx.entity();
        let to_delete = selected.clone();
        let buttons = h_flex()
            .gap(px(4.))
            .child(os::primary_button("env-create-profile", tr("Create profile")).on_click(move |_, window, cx| Self::create_dialog(view_c.clone(), window, cx)))
            .child(
                os::secondary_button("env-delete-profile", tr("Delete profile"), cx).disabled(to_delete.is_none()).on_click(move |_, window, cx| {
                    if let Some(name) = to_delete.clone() {
                        Self::delete_dialog(view_d.clone(), name, window, cx);
                    }
                }),
            );
        let view_s = cx.entity();
        // The default profile's density is fixed (live 1860: the select greyed).
        let density_pick = select("env-density", PixelDensity::ALL.iter().map(|d| crate::i18n::tr(d.label())).collect(), PixelDensity::ALL.iter().position(|d| *d == density).unwrap_or(0), selected.is_some(), 250., move |i, _, cx| {
            view_s.update(cx, |this, cx| {
                this.density = Some(PixelDensity::ALL[i]);
                cx.notify();
            })
        });
        section(
            "Environment profile settings",
            Some(help("env-help", "An environment profile is defined by settings tied to a specific browser on a specific device. Creating or selecting a profile will tie these settings to your current environment.", cx)),
            v_flex()
                .gap(px(25.))
                .child(labelled("Environment profile", h_flex().gap(px(5.)).child(pick).child(buttons)))
                .child(labelled("Match pixel density on high resolution displays (can affect performance)", density_pick))
                .child(h_flex().child(PreferencesView::save_button("env-save", tr("Save profile settings"), changed).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let Some(d) = this.density else { return };
                    let mut prefs = ListPrefs::load();
                    let Some(name) = prefs.env_profile.clone() else { return };
                    if let Some(p) = prefs.env_profiles.iter_mut().find(|p| p.name == name) {
                        p.density = d;
                    }
                    prefs.save();
                    this.density = None;
                    toast(crate::i18n::tr("Profile __name__ is updated.").replace("__name__", &name), cx);
                })))),
        )
    }
}

// ----------------------------------------------------------------- Onshape Labs

#[derive(Default)]
pub struct LabsPrefsView;

impl LabsPrefsView {
    /// Live 1873–1874: ticking opens this consent; only "Enable" ticks the box.
    fn consent_dialog(window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            // Live 1873: 12 px body text, 14 px semibold subheadings, a disc list.
            let para = |t: &str| div().text_size(px(12.)).line_height(px(17.)).text_color(pal.text).child(tr(t));
            let bold = |t: &str| div().mt(px(4.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(tr(t));
            let bullet = |t: &str| h_flex().items_start().pl(px(10.)).gap(px(8.)).child(div().text_size(px(12.)).line_height(px(17.)).child("●")).child(para(t));
            os::modal_doc_page(dialog, tr("Enable access to Onshape Labs features?"), 600.)
                .child(
                    os::modal_body()
                        .gap(px(8.))
                        .child(h_flex().gap(px(4.)).child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(tr("Onshape Labs"))).child(div().text_size(px(12.)).italic().child(tr("Experimental features — opt-in preview"))))
                        .child(para("Features in Onshape Labs are experimental, may break, and are subject to change or removal at any time. Onshape Labs features are made available for testing and evaluation only and must not be used in production. Use at your own discretion."))
                        .child(bold("How Onshape Labs features use your data"))
                        .child(para("When you enable a Labs feature, it may:"))
                        .child(bullet("Read or write data within your Onshape documents, only in response to your actions."))
                        .child(bullet("Perform actions within Onshape on your behalf during an active session."))
                        .child(bullet("Transmit relevant data to third-party AI providers to generate outputs, where applicable."))
                        .child(bold("Limitations of experimental AI features"))
                        .child(para("Some Onshape Labs features use generative or automated technologies, including third-party AI models, that produce outputs based on statistical models. These outputs may be inaccurate, incomplete, or inappropriate, and are not guaranteed to be correct."))
                        .child(para("You are responsible for independently reviewing, validating, and approving any outputs before relying on them for design, engineering, manufacturing, or other purposes. Onshape Labs features should not be used as a substitute for professional judgment or established controls."))
                        .child(para("Onshape does not warrant that generated outputs are free from third-party rights or suitable for any particular purpose."))
                        .child(para("You can disable Onshape Labs and revoke access at any time from Account Settings → Labs.")),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("labs-enable", tr("Enable Onshape Labs")))).child(os::modal_cancel(os::cancel_button("labs-cancel", tr("Cancel"), cx))))
                .on_ok(|_, window, cx| {
                    let mut prefs = ListPrefs::load();
                    prefs.labs = true;
                    prefs.save();
                    window.refresh();
                    toast(crate::i18n::tr("Access to the Onshape Labs features is enabled."), cx);
                    true
                })
        });
    }
}

impl Render for LabsPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let prefs = ListPrefs::load();
        let (on, co) = (prefs.labs, prefs.labs_co_complete);
        let check_row = |id: &'static str, checked: bool, label: SharedString| h_flex().id(id).h(px(21.)).gap(px(3.)).text_size(px(13.33)).text_color(pal.text).cursor_pointer().child(PreferencesView::checkbox_box(checked)).child(label);
        let external = |id: &'static str, label: &'static str, url: &'static str, cx: &App| {
            div()
                .id(id)
                .ml(px(4.))
                .cursor_pointer()
                .child(crate::assets::icon("open-external", 16., cx))
                .tooltip(crate::ui::tooltip(tr(label)))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(url);
                })
        };
        section(
            "Onshape Labs",
            // Live 1870, 1919: the blue flask, Labs on or off.
            Some(crate::assets::icon("onshape-labs-blue", 20., cx).into_any_element()),
            v_flex()
                .child(
                    check_row("pref-labs", on, tr("Enable users to access experimental, proof of concept features."))
                        .child(div().id("pref-labs-learn").ml(px(4.)).font_weight(FontWeight::BOLD).text_color(rgb(0x1651b0)).underline().child(tr("Learn more.")).on_click(|_, _, cx| {
                            cx.stop_propagation();
                            cx.open_url("https://cad.onshape.com/help/index.htm#cshid=onshape_labs");
                        }))
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            if on {
                                // Live: no confirmation.
                                let mut prefs = ListPrefs::load();
                                prefs.labs = false;
                                prefs.save();
                                toast(crate::i18n::tr("Access to the Onshape Labs features is disabled."), cx);
                                cx.notify();
                            } else {
                                Self::consent_dialog(window, cx);
                            }
                        })),
                )
                // The Labs features (live 1902; Descriptive image search is
                // left out: it searches Onshape's Public documents only).
                .when(on, |d| {
                    d.child(
                        v_flex().pl(px(14.)).pt(px(23.)).child(
                            check_row("pref-labs-co", co, tr("FeatureScript Co-complete"))
                                .child(external("pref-labs-co-help", "FeatureScript Co-complete", "https://cad.onshape.com/help/index.htm#cshid=co_complete", cx))
                                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                    let mut prefs = ListPrefs::load();
                                    prefs.labs_co_complete = !co;
                                    prefs.save();
                                    toast(crate::i18n::tr("Onshape Labs features updated."), cx);
                                    cx.notify();
                                })),
                        ),
                    )
                }),
        )
    }
}
