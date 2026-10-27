//! Company settings › Release management (Help
//! enterprise_settings_release_management; images accounts/co-rm-02,
//! release/relmgmt-workflows-01b, 13–15, 18–22, release-conditions-01,
//! watermark-options-01, watermarks-dialog, Capture-01): "Release management
//! settings for objects in documents owned by …", [Save release settings]
//! (at the top and at the bottom), then Workflows, Revisions and part
//! numbers, Access, Actions, Watermarks, Release conditions, Create Release
//! candidate dialog and Obsoletion. A new custom revision scheme is saved
//! with its own button.

use crate::os_ui as os;
use crate::preferences_window::PreferencesView;
use crate::release_settings::{self as rs, CustomScheme, ReleaseSettings, release_settings, set_release_settings};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::release::RevisionScheme;
use std::sync::Arc;

/// The scheme select's built-in entries (custom schemes and "New custom
/// revision scheme…" follow).
const BUILT_IN: [&str; 4] = [RevisionScheme::CHOICES[0], RevisionScheme::CHOICES[1], RevisionScheme::CHOICES[2], RevisionScheme::CHOICES[3]];
const NEW_SCHEME: &str = "New custom revision scheme…";

/// "New custom revision scheme…" being filled in (help image Capture-01).
struct NewScheme {
    name: Entity<InputState>,
    labels: Vec<String>,
}

pub struct ReleasePrefsView {
    saved: ReleaseSettings,
    edited: ReleaseSettings,
    suffix: Entity<InputState>,
    names: [Entity<InputState>; 4],
    /// Workflows: 0 Release workflow, 1 Obsoletion workflow.
    workflow_tab: usize,
    new_scheme: Option<NewScheme>,
    /// The shown workflow tab's chart, rasterized at a scale: (scale, tab, image).
    chart: Option<(f32, usize, Arc<RenderImage>)>,
    _subs: Vec<Subscription>,
}

impl ReleasePrefsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = release_settings();
        let suffix = cx.new(|cx| InputState::new(window, cx).default_value(saved.unreleased_suffix.clone()));
        let names = std::array::from_fn(|i| {
            let v = saved.display_names[i].clone();
            cx.new(|cx| InputState::new(window, cx).default_value(v))
        });
        let mut subs = Vec::new();
        for st in std::iter::once(&suffix).chain(names.iter()) {
            subs.push(cx.subscribe_in(st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.sync(cx);
                }
            }));
        }
        Self { edited: saved.clone(), saved, suffix, names, workflow_tab: 0, new_scheme: None, chart: None, _subs: subs }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        self.edited.unreleased_suffix = self.suffix.read(cx).value().to_string();
        for (i, st) in self.names.iter().enumerate() {
            self.edited.display_names[i] = st.read(cx).value().to_string();
        }
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.edited != self.saved {
            set_release_settings(&self.edited);
            self.saved = self.edited.clone();
            cx.notify();
        }
    }

    /// The scheme select's entries and the current one.
    fn scheme_options(&self) -> (Vec<String>, usize) {
        let mut options: Vec<String> = BUILT_IN.iter().map(|s| s.to_string()).collect();
        options.extend(self.edited.custom_schemes.iter().map(|c| c.name.clone()));
        options.push(NEW_SCHEME.into());
        let current = if self.new_scheme.is_some() {
            options.len() - 1
        } else {
            match &self.edited.scheme {
                RevisionScheme::Custom(labels) => self.edited.custom_schemes.iter().position(|c| &c.labels == labels).map_or(0, |i| BUILT_IN.len() + i),
                s => BUILT_IN.iter().position(|l| *l == s.label()).unwrap_or(0),
            }
        };
        (options, current)
    }

    fn choose_scheme(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let customs = self.edited.custom_schemes.len();
        self.new_scheme = None;
        match i {
            0 => self.edited.scheme = RevisionScheme::Alphabetical,
            1 => self.edited.scheme = RevisionScheme::Numeric { width: 1 },
            2 => self.edited.scheme = RevisionScheme::Numeric { width: 2 },
            3 => self.edited.scheme = RevisionScheme::Numeric { width: 3 },
            i if i < BUILT_IN.len() + customs => self.edited.scheme = RevisionScheme::Custom(self.edited.custom_schemes[i - BUILT_IN.len()].labels.clone()),
            _ => {
                let name = cx.new(|cx| InputState::new(window, cx));
                self._subs.push(cx.subscribe_in(&name, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
                    if matches!(ev, InputEvent::Change) {
                        cx.notify();
                    }
                }));
                self.new_scheme = Some(NewScheme { name, labels: Vec::new() });
            }
        }
        cx.notify();
    }

    fn select_revision_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        crate::file_dialogs::request_open_path("Select revision list", "Text files", &["txt"], window, cx, move |path, cx| {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let labels: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
            let _ = view.update(cx, |this, cx| {
                if let Some(n) = &mut this.new_scheme {
                    n.labels = labels;
                }
                cx.notify();
            });
        });
    }

    fn download_sample(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::file_dialogs::request_save_path("Download sample revision list", "sample-revision-list.txt".into(), "txt", window, cx, |path, _| {
            let sample: Vec<String> = (1..=rs::CUSTOM_SCHEME_MIN).map(|i| format!("R{i:02}")).collect();
            if let Err(e) = std::fs::write(&path, sample.join("\n") + "\n") {
                log::warn!("could not write {}: {e}", path.display());
            }
        });
    }

    /// [Save revision scheme]: kept in the list and selected (saved at once).
    fn save_new_scheme(&mut self, cx: &mut Context<Self>) {
        let Some(n) = &self.new_scheme else { return };
        let name = n.name.read(cx).value().trim().to_string();
        if name.is_empty() || n.labels.len() < rs::CUSTOM_SCHEME_MIN {
            return;
        }
        let scheme = CustomScheme { name, labels: n.labels.clone() };
        for s in [&mut self.saved, &mut self.edited] {
            s.custom_schemes.retain(|c| c.name != scheme.name);
            s.custom_schemes.push(scheme.clone());
        }
        set_release_settings(&self.saved);
        self.edited.scheme = RevisionScheme::Custom(scheme.labels);
        self.new_scheme = None;
        cx.notify();
    }

    fn edit_watermarks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let marks = self.edited.watermarks.clone();
        let form = cx.new(|cx| WatermarksForm::new(marks, window, cx));
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let (view, form) = (view.clone(), form.clone());
            os::modal_doc_page(dialog, "Watermarks", 592.).child(form.clone()).on_ok(move |_, _, cx| {
                let texts: Vec<String> = form.read(cx).inputs.iter().map(|st| watermark_text(&st.read(cx).value())).collect();
                view.update(cx, |this, cx| {
                    for (i, t) in texts.into_iter().enumerate() {
                        this.edited.watermarks.set(i, t);
                    }
                    cx.notify();
                });
                true
            })
        });
    }
}

/// The Watermarks dialog's rows and footer (help image watermarks-dialog):
/// a bold right-aligned state name and its text; [Save changes] once one
/// changed.
struct WatermarksForm {
    initial: rs::Watermarks,
    inputs: Vec<Entity<InputState>>,
    _subs: Vec<Subscription>,
}

impl WatermarksForm {
    fn new(initial: rs::Watermarks, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let inputs: Vec<Entity<InputState>> = (0..6).map(|i| cx.new(|cx| InputState::new(window, cx).default_value(initial.get(i).to_string()))).collect();
        let subs = inputs
            .iter()
            .map(|st| {
                cx.subscribe_in(st, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
                    if matches!(ev, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        Self { initial, inputs, _subs: subs }
    }
}

impl Render for WatermarksForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.inputs.iter().enumerate().any(|(i, st)| st.read(cx).value() != self.initial.get(i));
        let rows = self.inputs.iter().enumerate().map(|(i, st)| {
            h_flex()
                .gap(px(20.))
                .h(px(48.))
                .child(div().w(px(230.)).flex().justify_end().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(rs::Watermarks::LABELS[i]))
                .child(gpui_kit::base::input::input_frame(div().w(px(230.)).h(px(34.)).border_1().rounded(px(4.)).border_color(rgb(0xaaaaaa)).flex().items_center().child(Input::new(st).appearance(false).px(px(10.)).text_size(px(14.)))))
        });
        v_flex()
            .child(v_flex().py(px(10.)).children(rows))
            .child(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Save changes").disabled(!changed))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
    }
}

/// A watermark as stored: at most 64 characters, no Unicode (Help).
fn watermark_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii()).take(rs::Watermarks::MAX_LEN).collect()
}

/// A section: an 18 px / 500 heading over a 1 px `#333` rule, the content
/// at the heading's text (help images: checkboxes line up with the title).
fn section(title: &'static str, extra: Option<AnyElement>) -> Div {
    PreferencesView::heading(title, extra).mt(px(30.))
}

/// The account pages' checkbox: 13 px box, 14 px `#333` label.
fn check(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool, enabled: bool) -> Stateful<Div> {
    let pal = p();
    h_flex()
        .id(id)
        .min_h(px(25.))
        .gap(px(6.))
        .text_size(px(14.))
        .text_color(pal.text)
        .when(enabled, |d| d.cursor_pointer())
        .when(!enabled, |d| d.opacity(0.5))
        .child(PreferencesView::checkbox_box(on))
        .child(label.into())
}

impl Render for ReleasePrefsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let save = |id: &'static str, cx: &mut Context<Self>| os::primary_button(id, "Save release settings").disabled(!changed).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx)));
        let (save_top, save_bottom) = (save("save-release-settings-top", cx), save("save-release-settings", cx));
        let label = |t: &'static str| div().pt(px(12.)).pb(px(4.)).text_size(px(14.)).text_color(pal.text).child(t);
        let toggle = |id: &'static str, label: &str, on: bool, set: fn(&mut ReleaseSettings), cx: &mut Context<Self>| {
            check(id, label.to_string(), on, true).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                set(&mut this.edited);
                cx.notify();
            }))
        };
        let e = self.edited.clone();
        let owner = noshape_store::local_user_name();

        // ---- Workflows
        let sf = window.scale_factor();
        if self.chart.as_ref().is_none_or(|(s, tab, _)| *s != sf || *tab != self.workflow_tab) {
            let svg = if self.workflow_tab == 0 { release_chart_svg() } else { obsoletion_chart_svg() };
            self.chart = crate::release_dialog::workflow::rasterize(&svg, CHART_W, CHART_H, sf).map(|i| (sf, self.workflow_tab, i));
        }
        let tab = |i: usize, t: &'static str, cx: &mut Context<Self>| {
            let active = self.workflow_tab == i;
            div()
                .id(SharedString::from(format!("rm-workflow-tab-{i}")))
                .pb(px(2.))
                .text_size(px(14.))
                .cursor_pointer()
                .when(active, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(pal.emphasis).border_b_2().border_color(pal.accent))
                .when(!active, |d| d.text_color(pal.text_muted))
                .child(t)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.workflow_tab = i;
                    cx.notify();
                }))
        };
        let workflow = if self.workflow_tab == 0 {
            v_flex()
                .child(div().pt(px(12.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Onshape default release workflow"))
                .children(self.chart.as_ref().map(|(_, _, img)| img_el(img.clone()).mt(px(8.))))
                .child(div().pt(px(8.)).text_size(px(13.)).text_color(pal.text).child("This is Onshape's default release workflow."))
                .child(div().pt(px(14.)).max_w(px(700.)).text_size(px(13.)).text_color(pal.text).child("The creator of a Release candidate specifies a set of approvers and a set of observers who will be notified when the Release candidate is transitioned. Observers are optional, and approvers may be optional or required depending on company settings."))
        } else {
            v_flex()
                .child(div().pt(px(12.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Onshape default obsoletion workflow"))
                .children(self.chart.as_ref().map(|(_, _, img)| img_el(img.clone()).mt(px(8.))))
                .child(div().pt(px(8.)).text_size(px(13.)).text_color(pal.text).child("This is Onshape's default obsoletion workflow."))
        };
        let workflows = section("Workflows", Some(crate::assets::icon("help-gray", 20., cx)))
            .child(
                v_flex()
                    .pl(px(15.))
                    .pt(px(8.))
                    .child(toggle("rm-managed", "Enable managed workflows", e.managed_workflows, |s| s.managed_workflows = !s.managed_workflows, cx))
                    .when(e.managed_workflows, |d| {
                        d.child(h_flex().pt(px(10.)).pl(px(175.)).gap(px(90.)).child(tab(0, "Release workflow", cx)).child(tab(1, "Obsoletion workflow", cx))).child(workflow)
                    }),
            );

        // ---- Revisions and part numbers
        let (options, current) = self.scheme_options();
        let view = cx.entity();
        let select = os::select_field("rm-scheme", options, current, move |i, window, cx| view.update(cx, |this, cx| this.choose_scheme(i, window, cx)));
        let new_scheme = self.new_scheme.as_ref().map(|n| {
            let name_empty = n.name.read(cx).value().trim().is_empty();
            let preview = if n.labels.is_empty() { None } else { Some(n.labels.join("\n")) };
            let can_save = !name_empty && n.labels.len() >= rs::CUSTOM_SCHEME_MIN;
            v_flex()
                .w(px(530.))
                .child(
                    h_flex()
                        .pt(px(8.))
                        .gap(px(6.))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(rgb(0xaa252d)).child("Name *"))
                        .when(name_empty, |d| d.child(div().text_size(px(12.)).text_color(rgb(0xaa252d)).child("Required field must not be empty."))),
                )
                .child(gpui_kit::base::input::input_frame(div().mt(px(4.)).h(px(32.)).border_1().rounded(px(4.)).border_color(rgb(0xaaaaaa)).flex().items_center().child(Input::new(&n.name).appearance(false).px(px(10.)))))
                .child(h_flex().pt(px(8.)).gap(px(4.)).text_size(px(14.)).text_color(pal.text).child(div().font_weight(FontWeight::BOLD).child("Revision list")).child("(Select a text file with one revision per line)"))
                .child(
                    div()
                        .id("rm-scheme-preview")
                        .mt(px(4.))
                        .h(px(46.))
                        .overflow_y_scroll()
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(rgb(0xcccccc))
                        .bg(rgb(0xeeeeee))
                        .text_size(px(14.))
                        .text_color(pal.text_muted)
                        .map(|d| match preview {
                            Some(t) => d.whitespace_normal().child(t),
                            None => d.italic().child("Select a file to preview revisions"),
                        }),
                )
                .when(!n.labels.is_empty() && n.labels.len() < rs::CUSTOM_SCHEME_MIN, |d| {
                    d.child(div().pt(px(4.)).text_size(px(12.)).text_color(rgb(0xaa252d)).child(format!("The revision list must have at least {} revisions.", rs::CUSTOM_SCHEME_MIN)))
                })
                .child(
                    div()
                        .id("rm-scheme-sample")
                        .pt(px(6.))
                        .text_size(px(14.))
                        .text_color(pal.accent)
                        .cursor_pointer()
                        .hover(|d| d.underline())
                        .child("Download sample revision list")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.download_sample(window, cx))),
                )
                .child(
                    h_flex()
                        .pt(px(10.))
                        .gap(px(4.))
                        .child(os::primary_button("rm-scheme-file", "Select file").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.select_revision_file(window, cx))))
                        .child(os::primary_button("rm-scheme-save", "Save revision scheme").disabled(!can_save).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save_new_scheme(cx)))),
                )
        });
        let scheme_id = rs::scheme_id(&e.scheme);
        // The template editor saves at once (live: its own REST call).
        let saved_view = cx.entity();
        let template_saved: crate::revision_template::OnSaved = std::rc::Rc::new(move |t, cx| {
            saved_view.update(cx, |this, cx| {
                this.saved.revision_table = t.clone();
                this.edited.revision_table = t;
                cx.notify();
            })
        });
        let id_copy = scheme_id.clone();
        let revisions = section("Revisions and part numbers", None).child(
            v_flex()
                .pl(px(15.))
                .child(label("Revision scheme"))
                .child(div().w(px(525.)).child(select))
                .children(new_scheme)
                .when(self.new_scheme.is_none(), |d| {
                    d.child(
                        h_flex()
                            .pt(px(10.))
                            .gap(px(6.))
                            .child(div().text_size(px(13.)).text_color(pal.text).child("Revision scheme id:"))
                            .child(div().px(px(10.)).py(px(6.)).rounded(px(4.)).bg(rgb(0xeeeeee)).border_1().border_color(rgb(0xcccccc)).text_size(px(13.)).text_color(pal.text_muted).child(scheme_id))
                            .child(os::primary_button("rm-scheme-copy", "Copy to clipboard").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(id_copy.clone())))),
                    )
                })
                .child(label("Unreleased revision suffix:"))
                .child(gpui_kit::base::input::input_frame(div().w(px(167.)).h(px(32.)).border_1().rounded(px(4.)).border_color(rgb(0xaaaaaa)).flex().items_center().child(Input::new(&self.suffix).appearance(false).px(px(10.)))))
                .child(crate::revision_template::template_list(&self.saved.revision_table, template_saved, cx))
                .child(
                    div()
                        .id("rm-numbering-link")
                        .mt(px(18.))
                        .text_size(px(14.))
                        .text_color(pal.accent)
                        .cursor_pointer()
                        .hover(|d| d.underline())
                        .child("Click here to go to release management part number generation settings")
                        .on_click(|_, _, cx| crate::company_settings::show(crate::company_settings::Section::NumberingSchemes, cx)),
                ),
        );

        // ---- Access, Actions
        let access = section("Access", None).child(v_flex().pl(px(15.)).pt(px(8.)).children(rs::ACCESS.iter().enumerate().map(|(i, t)| {
            check(SharedString::from(format!("rm-access-{i}")), *t, e.access[i], true).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.edited.access[i] = !this.edited.access[i];
                cx.notify();
            }))
        })));
        let actions = section("Actions", None).child(v_flex().pl(px(15.)).pt(px(8.)).child(toggle(
            "rm-new-revision",
            "Require a new revision when an item's properties have changed",
            e.require_new_revision,
            |s| s.require_new_revision = !s.require_new_revision,
            cx,
        )));

        // ---- Watermarks
        let watermarks = section("Watermarks", None).child(
            v_flex()
                .pl(px(15.))
                .pt(px(8.))
                // Turning watermarks off also clears the second option (live).
                .child(toggle("rm-watermark", "Show watermark for drawings", e.show_watermark, |s| {
                    s.show_watermark = !s.show_watermark;
                    if !s.show_watermark {
                        s.hide_watermark_unmanaged = false;
                    }
                }, cx))
                // Live (bundle): the second option only shows while watermarks
                // are on, and Edit watermarks is disabled without them.
                .when(e.show_watermark, |d| {
                    d.child(toggle("rm-watermark-unmanaged", "Hide watermark for drawings that are not revision managed", e.hide_watermark_unmanaged, |s| s.hide_watermark_unmanaged = !s.hide_watermark_unmanaged, cx))
                })
                .child(h_flex().pt(px(8.)).child(os::primary_button("rm-edit-watermarks", "Edit watermarks").disabled(!e.show_watermark).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.edit_watermarks(window, cx))))),
        );

        // ---- Release conditions
        let conditions = section("Release conditions", Some(div().pl(px(6.)).text_size(px(13.)).text_color(pal.text_muted).child("Selected conditions will prevent a Release from being created").into_any_element())).child(
            v_flex().pl(px(15.)).pt(px(8.)).children(rs::CONDITIONS.iter().enumerate().map(|(i, t)| {
                check(SharedString::from(format!("rm-condition-{i}")), *t, e.conditions[i], true).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.edited.conditions[i] = !this.edited.conditions[i];
                    cx.notify();
                }))
            })),
        );

        // ---- Create Release candidate dialog
        let approver = e.require_approver;
        let names = e.custom_display_names.then(|| {
            v_flex().pl(px(19.)).pb(px(4.)).gap(px(6.)).children(self.names.iter().enumerate().map(|(i, st)| {
                h_flex()
                    .gap(px(10.))
                    .child(div().w(px(130.)).text_size(px(14.)).text_color(pal.text).child(rs::DISPLAY_NAME_FIELDS[i]))
                    .child(gpui_kit::base::input::input_frame(div().w(px(300.)).h(px(32.)).border_1().rounded(px(4.)).border_color(rgb(0xaaaaaa)).flex().items_center().child(Input::new(st).appearance(false).px(px(10.)))))
            }))
        });
        let candidate = section("Create Release candidate dialog", None).child(
            v_flex()
                .pl(px(15.))
                .pt(px(8.))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .child(toggle("rm-require-approver", "Require an approver in the Release dialog", approver, |s| s.require_approver = !s.require_approver, cx))
                        .child(crate::assets::icon("help-gray", 20., cx)),
                )
                .child(check("rm-no-creator", "Do not allow creator as approver", e.no_creator_approver, approver).when(approver, |d| {
                    d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.edited.no_creator_approver = !this.edited.no_creator_approver;
                        cx.notify();
                    }))
                }))
                .child(toggle("rm-all-approvers", "Require approval from all approvers", e.require_all_approvers, |s| s.require_all_approvers = !s.require_all_approvers, cx))
                .child(toggle("rm-require-note", "Require a note in the Release dialog", e.require_note, |s| s.require_note = !s.require_note, cx))
                .child(toggle("rm-other-documents", "Allow adding items from other documents", e.allow_other_documents, |s| s.allow_other_documents = !s.allow_other_documents, cx))
                .child(toggle("rm-display-names", "Enter display names for Release & Obsoletion management properties", e.custom_display_names, |s| s.custom_display_names = !s.custom_display_names, cx))
                .children(names),
        );

        // ---- Obsoletion
        let obsoletion = section("Obsoletion", None).child(v_flex().pl(px(15.)).pt(px(8.)).child(toggle(
            "rm-auto-obsolete",
            "Automatically obsolete previous released revisions",
            e.auto_obsolete,
            |s| s.auto_obsolete = !s.auto_obsolete,
            cx,
        )));

        v_flex()
            .w_full()
            .child(div().pt(px(16.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(format!("Release management settings for objects in documents owned by {owner}.")))
            .child(h_flex().mt(px(18.)).child(save_top))
            .child(workflows)
            .child(revisions)
            .child(access)
            .child(actions)
            .child(watermarks)
            .child(conditions)
            .child(candidate)
            .child(obsoletion)
            .child(h_flex().mt(px(30.)).child(save_bottom))
    }
}

fn img_el(image: Arc<RenderImage>) -> Img {
    img(ImageSource::Render(image)).w(px(CHART_W)).h(px(CHART_H)).flex_none()
}

/// The workflow charts' size (the release chart: help image relmgmt-workflows-13, 1:1).
const CHART_W: f32 = 720.;
const CHART_H: f32 = 285.;

/// Chart drawing (both workflow charts): polylines, arrowheads, labels,
/// state cards and the approval diamond, in the help images' style.
mod chart {
    use std::fmt::Write as _;

    pub fn open(w: f32, h: f32) -> String {
        format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Roboto" font-size="13" fill="#1f1f1f"><rect width="100%" height="100%" fill="#ffffff"/>"##)
    }

    pub fn line(s: &mut String, pts: &[(f32, f32)]) {
        let d: Vec<String> = pts.iter().map(|(x, y)| format!("{x},{y}")).collect();
        let _ = write!(s, r##"<polyline points="{}" fill="none" stroke="#1f1f1f" stroke-width="1.2"/>"##, d.join(" "));
    }

    /// An arrowhead with its tip at `(x, y)`, pointing right, down or up.
    pub fn arrow(s: &mut String, (x, y): (f32, f32), dir: char) {
        let pts = match dir {
            'r' => format!("{x},{y} {},{} {},{}", x - 7., y - 3.5, x - 7., y + 3.5),
            'd' => format!("{x},{y} {},{} {},{}", x - 3.5, y - 7., x + 3.5, y - 7.),
            _ => format!("{x},{y} {},{} {},{}", x - 3.5, y + 7., x + 3.5, y + 7.),
        };
        let _ = write!(s, r##"<polygon points="{pts}" fill="#1f1f1f"/>"##);
    }

    pub fn text(s: &mut String, x: f32, y: f32, t: &str, anchor: &str) {
        let _ = write!(s, r#"<text x="{x}" y="{y}" text-anchor="{anchor}">{t}</text>"#);
    }

    /// The approval diamond centred at `(x, y)`.
    pub fn diamond(s: &mut String, x: f32, y: f32) {
        let _ = write!(s, r##"<polygon points="{x},{} {},{y} {x},{} {},{y}" fill="#f5d664" stroke="#1f1f1f" stroke-width="1.2"/>"##, y - 45., x + 46., y + 45., x - 46.);
        text(s, x, y - 3., "Approval", "middle");
        text(s, x, y + 13., "required?", "middle");
    }

    /// A state card: frame, colour panel, label (lines split at `\n`) and
    /// the caption's marker: "circle", "solid" ▲, "open" △ or "crossed"
    /// (△ struck through, Obsolete).
    pub fn card(s: &mut String, (x, y): (f32, f32), h: f32, fill: &str, label: &str, caption: &str, marker: &str) {
        let w = 80.;
        let _ = write!(s, r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="4" fill="#ffffff" stroke="#1f1f1f" stroke-width="1.5"/>"##);
        let _ = write!(s, r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" stroke="#898d92"/>"##, x + 6., y + 7., w - 12., h - 31.);
        let lines: Vec<&str> = label.split('\n').collect();
        let mid = y + (h - 24.) / 2. + 8.;
        for (i, l) in lines.iter().enumerate() {
            let ly = mid + 16. * (i as f32 - (lines.len() - 1) as f32 / 2.);
            let _ = write!(s, r#"<text x="{}" y="{ly}" text-anchor="middle" font-size="13">{l}</text>"#, x + w / 2.);
        }
        // The caption and its marker centred under the panel (Roboto 11 px:
        // "Workspace" about 52 px, "Version" 38 px).
        let cy = y + h - 10.;
        let text_w = if caption == "Workspace" { 52. } else { 38. };
        let start = x + w / 2. - (text_w + 15.) / 2.;
        let _ = write!(s, r#"<text x="{start}" y="{}" font-size="11">{caption}</text>"#, cy + 4.);
        let mx = start + text_w + 9.5;
        let triangle = format!("{mx},{} {},{} {},{}", cy - 6., mx - 6., cy + 5., mx + 6., cy + 5.);
        match marker {
            "circle" => {
                let _ = write!(s, r##"<circle cx="{mx}" cy="{cy}" r="5" fill="none" stroke="#1f1f1f" stroke-width="1.2"/>"##);
            }
            "solid" => {
                let _ = write!(s, r##"<polygon points="{triangle}" fill="#1f1f1f"/>"##);
            }
            "crossed" => {
                let _ = write!(s, r##"<polygon points="{triangle}" fill="none" stroke="#1f1f1f" stroke-width="1.2"/>"##);
                let _ = write!(s, r##"<path d="M{} {}L{} {}M{} {}L{} {}" stroke="#1f1f1f" stroke-width="1.2"/>"##, mx - 6., cy - 6., mx + 6., cy + 5., mx + 6., cy - 6., mx - 6., cy + 5.);
            }
            _ => {
                let _ = write!(s, r##"<polygon points="{triangle}" fill="none" stroke="#1f1f1f" stroke-width="1.2"/>"##);
            }
        }
    }
}

/// The default release workflow chart: the states as cards, the approval
/// diamond, arrows and labels.
fn release_chart_svg() -> String {
    use chart::{arrow, card, line, text};
    let mut s = chart::open(CHART_W, CHART_H);
    // Revise loops (top: from Released; bottom: from Rejected).
    line(&mut s, &[(668., 48.), (668., 30.), (166., 30.), (166., 101.)]);
    arrow(&mut s, (166., 108.), 'd');
    text(&mut s, 405., 20., "Create workspace from version to revise your design", "middle");
    line(&mut s, &[(628., 258.), (166., 258.), (166., 222.)]);
    arrow(&mut s, (166., 215.), 'u');
    text(&mut s, 405., 276., "Create workspace from version to revise your design", "middle");
    // Entry.
    for (i, t) in ["Create Part,", "Assembly,", "or Drawing"].iter().enumerate() {
        text(&mut s, 37., 148. + 18. * i as f32, t, "start");
    }
    line(&mut s, &[(104., 163.), (116., 163.)]);
    arrow(&mut s, (123., 163.), 'r');
    card(&mut s, (123., 108.), 107., "#d1dced", "In progress", "Workspace", "circle");
    // Release → diamond.
    line(&mut s, &[(205., 163.), (220., 163.)]);
    text(&mut s, 249., 168., "Release", "middle");
    line(&mut s, &[(278., 163.), (289., 163.)]);
    arrow(&mut s, (296., 163.), 'r');
    chart::diamond(&mut s, 342., 163.);
    // Yes → Pending.
    line(&mut s, &[(388., 163.), (402., 163.)]);
    text(&mut s, 418., 168., "Yes", "middle");
    line(&mut s, &[(433., 163.), (445., 163.)]);
    arrow(&mut s, (452., 163.), 'r');
    card(&mut s, (452., 112.), 100., "#bde0f5", "Pending", "Version", "open");
    // No → Released.
    line(&mut s, &[(342., 118.), (342., 102.)]);
    text(&mut s, 342., 96., "No", "middle");
    line(&mut s, &[(342., 82.), (342., 65.), (621., 65.)]);
    arrow(&mut s, (628., 65.), 'r');
    card(&mut s, (628., 48.), 102., "#b8e986", "Released", "Version", "solid");
    // Approved / Rejected.
    line(&mut s, &[(532., 127.), (544., 127.)]);
    text(&mut s, 574., 131., "Approved", "middle");
    line(&mut s, &[(604., 127.), (621., 127.)]);
    arrow(&mut s, (628., 127.), 'r');
    line(&mut s, &[(532., 196.), (544., 196.)]);
    text(&mut s, 574., 200., "Rejected", "middle");
    line(&mut s, &[(604., 196.), (621., 196.)]);
    arrow(&mut s, (628., 196.), 'r');
    card(&mut s, (628., 173.), 102., "#e98d86", "Rejected", "Version", "open");
    s.push_str("</svg>");
    s
}

/// The default obsoletion workflow chart (help image relmgmt-workflows-04,
/// at the release chart's scale): Released → Obsolete → approval? No:
/// Obsolete; Yes: Released (pending) → Approved: Obsolete, Rejected: back
/// to Released.
fn obsoletion_chart_svg() -> String {
    use chart::{arrow, card, line, text};
    let mut s = chart::open(CHART_W, CHART_H);
    card(&mut s, (13., 73.), 104., "#b8e986", "Released", "Version", "solid");
    // Obsolete → diamond.
    line(&mut s, &[(93., 124.), (108., 124.)]);
    text(&mut s, 136., 129., "Obsolete", "middle");
    line(&mut s, &[(164., 124.), (179., 124.)]);
    arrow(&mut s, (186., 124.), 'r');
    chart::diamond(&mut s, 232., 124.);
    // No → Obsolete (up and across).
    line(&mut s, &[(232., 79.), (232., 62.)]);
    text(&mut s, 232., 56., "No", "middle");
    line(&mut s, &[(232., 42.), (232., 26.), (518., 26.)]);
    arrow(&mut s, (525., 26.), 'r');
    card(&mut s, (525., 10.), 104., "#cbcfda", "Obsolete", "Version", "crossed");
    // Yes → Released (pending).
    line(&mut s, &[(278., 124.), (296., 124.)]);
    text(&mut s, 312., 129., "Yes", "middle");
    line(&mut s, &[(328., 124.), (340., 124.)]);
    arrow(&mut s, (347., 124.), 'r');
    card(&mut s, (347., 73.), 104., "#bde0f5", "Released\n(pending)", "Version", "open");
    // Approved → Obsolete; Rejected → back to Released along the bottom.
    line(&mut s, &[(427., 89.), (441., 89.)]);
    text(&mut s, 470., 93., "Approved", "middle");
    line(&mut s, &[(500., 89.), (518., 89.)]);
    arrow(&mut s, (525., 89.), 'r');
    line(&mut s, &[(427., 159.), (441., 159.)]);
    text(&mut s, 470., 163., "Rejected", "middle");
    line(&mut s, &[(500., 159.), (511., 159.), (511., 220.), (53., 220.), (53., 184.)]);
    arrow(&mut s, (53., 177.), 'u');
    s.push_str("</svg>");
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_workflow_charts_parse() {
        for svg in [super::release_chart_svg(), super::obsoletion_chart_svg()] {
            assert!(resvg::usvg::Tree::from_str(&svg, &Default::default()).is_ok());
        }
        assert_eq!(super::watermark_text("Entwurf – ok"), "Entwurf  ok");
        assert_eq!(super::watermark_text(&"x".repeat(80)).len(), 64);
    }
}
