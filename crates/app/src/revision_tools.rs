//! Company settings › Revision tools › Reset part number type (Help
//! revision_tools and its images accounts/revision-tools-find-01,
//! revision-tools-reset-dialog-01): a part number and an object type, [Find
//! revisions] lists that number's revisions of that type across the
//! documents on this computer, and [Reset part number type] (confirmed in a
//! dialog) obsoletes them as re-releasable, so the number can be released
//! again with another type.

use crate::os_ui as os;
use crate::preferences_window::PreferencesView;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::release::{ItemKind, ReleaseLog, TypedRevision};
use std::path::PathBuf;

/// The release log's key in a document's store (as the release dialog's).
const META_KEY: &str = crate::release_dialog::META_KEY;

const KINDS: [ItemKind; 5] = [ItemKind::Part, ItemKind::Assembly, ItemKind::Drawing, ItemKind::VariableStudio, ItemKind::File];

/// One row of the search: a revision and the document it was released in.
struct Found {
    path: PathBuf,
    document: String,
    revision: TypedRevision,
}

pub struct RevisionToolsView {
    part_number: Entity<InputState>,
    /// Index into [`KINDS`]; none until chosen (the help image's empty select).
    kind: Option<usize>,
    /// The last search: its part number and type, and the rows.
    found: Option<(String, ItemKind, Vec<Found>)>,
    _subs: Vec<Subscription>,
}

impl RevisionToolsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let part_number = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![cx.subscribe_in(&part_number, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self { part_number, kind: None, found: None, _subs: subs }
    }

    fn find(&mut self, cx: &mut Context<Self>) {
        let number = self.part_number.read(cx).value().trim().to_string();
        let Some(kind) = self.kind.map(|k| KINDS[k]) else { return };
        if number.is_empty() {
            return;
        }
        let rows = search(&number, kind);
        self.found = Some((number, kind, rows));
        cx.notify();
    }

    fn confirm_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let view = view.clone();
            os::modal_doc_page(dialog, "Reset part number type", 590.)
                .child(
                    os::modal_body()
                        .child(crate::ui::alert(crate::ui::AlertKind::Warning).py(px(12.)).text_size(px(14.)).child(crate::assets::icon("warning", 18., cx)).child("This action cannot be undone."))
                        .child(div().pt(px(6.)).text_color(pal.text).child("Resetting the part number type of a releasable object will mark all previous revisions as obsolete and re-releasable. Revision history will still be visible for any previous revisions.")),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::danger_button("ok", "Reset part number type", cx))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, _, cx| {
                    view.update(cx, |this, cx| this.reset(cx));
                    true
                })
        });
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        let Some((number, kind, rows)) = &self.found else { return };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let mut paths: Vec<&PathBuf> = rows.iter().map(|r| &r.path).collect();
        paths.dedup();
        for path in paths {
            if let Err(e) = reset_in(path, number, *kind, now) {
                log::warn!("could not reset part number type in {}: {e:#}", path.display());
            }
        }
        self.find(cx);
    }
}

/// The revisions of `number` released as `kind` in every local document,
/// newest first.
fn search(number: &str, kind: ItemKind) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    for path in crate::variable_insert::other_documents(std::path::Path::new("")) {
        let Ok(store) = noshape_store::Document::open_read_only(&path) else { continue };
        let log = store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
        let revisions = log.typed_revisions(number, kind);
        if revisions.is_empty() {
            continue;
        }
        let document = crate::derived::external_doc_info(&path).map(|i| i.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
        out.extend(revisions.into_iter().map(|revision| Found { path: path.clone(), document: document.clone(), revision }));
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.revision.created));
    out
}

fn reset_in(path: &std::path::Path, number: &str, kind: ItemKind, now: i64) -> anyhow::Result<()> {
    let mut store = noshape_store::Document::open(path)?;
    let mut log = store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
    if log.reset_part_number_type(number, kind, now) {
        store.set_meta(META_KEY, Some(&log.to_json()))?;
    }
    Ok(())
}

/// Help image (1:1): bold 14 px labels, 380 x 32 fields, the table's columns.
const FIELD_W: f32 = 380.;
const COLS: [(&str, f32); 9] = [("Part number", 110.), ("Type", 52.), ("Revision", 80.), ("Document name", 138.), ("Release name", 121.), ("State", 79.), ("Created", 111.), ("Re-releasable?", 129.), ("Can change type?", 140.)];
const TABLE_RULE: u32 = 0xdee2e6;

impl Render for RevisionToolsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let label = |t: &'static str| div().pt(px(16.)).pb(px(6.)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(t);
        let can_find = self.kind.is_some() && !self.part_number.read(cx).value().trim().is_empty();
        let kinds: Vec<String> = std::iter::once(String::new()).chain(KINDS.iter().map(|k| k.label().to_string())).collect();
        let view = cx.entity();
        let select = os::select_field("revtools-kind", kinds, self.kind.map_or(0, |k| k + 1), move |i, _, cx| {
            view.update(cx, |this, cx| {
                this.kind = i.checked_sub(1);
                cx.notify();
            })
        });
        let bullet = |t: &'static str| h_flex().pl(px(18.)).gap(px(10.)).child(div().size(px(5.)).rounded_full().bg(pal.text)).child(t);
        let table = self.found.as_ref().map(|(number, kind, rows)| {
            let cell = |w: f32, t: SharedString| div().w(px(w)).flex_none().px(px(6.)).truncate().child(t);
            let header = h_flex().h(px(38.)).border_b_1().border_color(rgb(TABLE_RULE)).font_weight(FontWeight::BOLD).children(COLS.iter().map(|(t, w)| cell(*w, (*t).into())));
            let yes_no = |b: bool| SharedString::from(if b { "Yes" } else { "No" });
            let body = rows.iter().map(|r| {
                let v = &r.revision;
                let texts: [SharedString; 9] = [
                    number.clone().into(),
                    kind.label().into(),
                    v.revision.clone().into(),
                    r.document.clone().into(),
                    v.release_name.clone().into(),
                    v.state.label().into(),
                    crate::start_library::format_onshape_time(v.created).into(),
                    yes_no(v.re_releasable),
                    yes_no(v.re_releasable),
                ];
                h_flex().h(px(33.)).border_b_1().border_color(rgb(TABLE_RULE)).children(COLS.iter().zip(texts).map(|((_, w), t)| cell(*w, t)))
            });
            let can_reset = rows.iter().any(|r| !r.revision.re_releasable);
            v_flex()
                .mt(px(36.))
                .text_size(px(14.))
                .text_color(pal.text)
                .child(header)
                .children(body)
                .when(rows.is_empty(), |d| d.child(div().py(px(8.)).px(px(6.)).text_color(pal.text_muted).child("No revisions found.")))
                .child(
                    h_flex().mt(px(14.)).child(
                        os::danger_button("revtools-reset", "Reset part number type", cx)
                            .disabled(!can_reset)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.confirm_reset(window, cx))),
                    ),
                )
        });
        v_flex()
            .w_full()
            .child(
                PreferencesView::heading("Reset part number type", None).child(
                    v_flex()
                        .pl(px(15.))
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(div().pt(px(10.)).child("Part numbers are tied to the type of released object. Resetting the released object type of a part number will:"))
                        .child(bullet("Obsolete all previous revisions related to the previous object type and part number"))
                        .child(bullet("Allow the part number to be released again with a new object type"))
                        .child(label("Part number:"))
                        .child(gpui_kit::base::input::input_frame(div().w(px(FIELD_W)).h(px(32.)).border_1().rounded(px(4.)).border_color(rgb(0xcccccc)).flex().items_center().child(Input::new(&self.part_number).appearance(false).px(px(9.)))))
                        .child(label("Object type:"))
                        .child(div().w(px(FIELD_W)).child(select))
                        .child(h_flex().mt(px(14.)).child(os::primary_button("revtools-find", "Find revisions").disabled(!can_find).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.find(cx)))))
                        .children(table),
                ),
            )
    }
}
