//! Company settings › Release management › Revisions and part numbers ›
//! Revision table template (live bundle `osx-properties-table-template-list`
//! / `-dialog` / `-columns` with type REVISION_TABLE; behaviours "ui2
//! revision table template editor (bundle)", help image
//! relmgmt-workflows-14). The template sets the columns of drawings'
//! revision tables ([`noshape_model::drawing::tables::RevisionTemplate`]).

use crate::os_ui::{self as os, OsDropdownExt as _};
use crate::release_settings::{release_settings, set_release_settings};
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::tables::RevisionTemplate;
use std::rc::Rc;

/// The properties "Add column" offers: Onshape's standard Drawing
/// properties in the server's order, after the editor's filters (user and
/// date types allowed for revision tables; Category's type isn't). Live
/// dl-bundles/revtable-template/drawing-properties.json.
pub const COLUMN_CHOICES: [&str; 17] = [
    "Name",
    "Description",
    "Part number",
    "Revision",
    "State",
    "Title 1",
    "Title 2",
    "Title 3",
    "Drawn by",
    "Approver",
    "Date drawn",
    "Date approved",
    "Not revision managed",
    "Last changed date",
    "Last changed by",
    "Revision description",
    "Inspection count",
];

/// Told the template the editor saved.
pub type OnSaved = Rc<dyn Fn(RevisionTemplate, &mut App)>;

/// The Revision column: always there, it has no ×.
const REVISION: &str = "Revision";
/// Name input `maxlength`.
const NAME_MAX: usize = 256;
/// A column box: 130 px × 3em at 14 px.
const BOX_W: f32 = 130.;
const BOX_H: f32 = 42.;

/// A column being dragged to a new place in the editor's strip.
#[derive(Clone)]
struct ColumnDrag(usize, SharedString);

impl Render for ColumnDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        column_box(&self.1, false).opacity(0.85)
    }
}

/// A column box (`.property-column-cell`): `--os-background-quinary` fill,
/// 1 px `--os-outline-primary`, 14 px / 600 centred on one line with an
/// ellipsis; in capitals with "All uppercase in Drawings".
fn column_box(name: &str, caps: bool) -> Div {
    let text = if caps { name.to_uppercase() } else { name.to_string() };
    div()
        .relative()
        .w(px(BOX_W))
        .h(px(BOX_H))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .px(px(5.))
        .bg(rgb(0xeeeeee))
        .border_1()
        .border_color(rgb(0x333333))
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p().text)
        .child(div().min_w_0().truncate().child(crate::ui::user_text(text)))
}

/// The strip of column boxes, read-only (the settings list).
fn columns_strip(t: &RevisionTemplate) -> Div {
    h_flex().children(t.columns.iter().map(|c| column_box(c, t.all_caps)))
}

/// The "Revision table template" block of the section: its label with the
/// (?) hint, then the list (Name | Columns) with the template's row and
/// its [Edit] button. `on_saved` gets the template the editor saved.
pub fn template_list(t: &RevisionTemplate, on_saved: OnSaved, cx: &App) -> AnyElement {
    let pal = p();
    let label = h_flex()
        .items_end()
        .gap(px(4.))
        .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Revision table template"))
        .child(div().id("rt-template-help").child(crate::assets::icon("help-gray", 20., cx)).tooltip(crate::ui::tooltip("Edit the template for the revision table in a Drawing.")));
    // Bootstrap row: col-3 | col-7 | col-1 of the 1000 px list, 20 px in.
    let (name_w, cols_w) = (240., 560.);
    let header = h_flex()
        .min_h(px(34.))
        .px(px(20.))
        .border_b_1()
        .border_color(rgb(0xdddddd))
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .text_color(pal.text)
        .child(div().w(px(name_w)).child("Name"))
        .child(div().w(px(cols_w)).child("Columns"));
    let template = t.clone();
    let row = h_flex()
        .min_h(px(34.))
        .py(px(8.))
        .px(px(20.))
        .text_size(px(14.))
        .text_color(pal.text)
        .child(div().w(px(name_w)).pr(px(15.)).min_w_0().truncate().child(crate::ui::user_text(t.name.clone())))
        .child(div().w(px(cols_w)).overflow_hidden().child(columns_strip(t)))
        .child(
            Button::new("rt-template-edit")
                .primary()
                .child(div().text_size(px(12.)).child("Edit"))
                .h(px(24.))
                .px(px(8.))
                .rounded(px(3.))
                .on_click(move |_, window, cx| open_editor(template.clone(), on_saved.clone(), window, cx)),
        );
    v_flex().pt(px(14.)).child(label).child(v_flex().w(px(1000.)).mt(px(8.)).mb(px(25.)).child(header).child(row)).into_any_element()
}

/// The editor: "Edit revision table template".
fn open_editor(t: RevisionTemplate, on_saved: OnSaved, window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| TemplateEditor::new(t, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        let (form, on_saved) = (form.clone(), on_saved.clone());
        os::modal_doc_page(dialog, "Edit revision table template", 600.).child(form.clone()).on_ok(move |_, _, cx| {
            let t = form.read(cx).template(cx);
            let mut s = release_settings();
            s.revision_table = t.clone();
            set_release_settings(&s);
            crate::prefs_general::toast(format!("{} updated.", t.name), cx);
            on_saved(t, cx);
            true
        })
    });
}

struct TemplateEditor {
    initial: RevisionTemplate,
    name: Entity<InputState>,
    columns: Vec<String>,
    all_caps: bool,
    _subs: Vec<Subscription>,
}

impl TemplateEditor {
    fn new(t: RevisionTemplate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("New template").default_value(t.name.clone()));
        let sub = cx.subscribe_in(&name, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        // The Revision column is always there.
        let mut columns = t.columns.clone();
        if !columns.iter().any(|c| c == REVISION) {
            columns.insert(0, REVISION.to_string());
        }
        Self { all_caps: t.all_caps, columns, initial: t, name, _subs: vec![sub] }
    }

    fn template(&self, cx: &App) -> RevisionTemplate {
        RevisionTemplate { name: self.name.read(cx).value().trim().to_string(), columns: self.columns.clone(), all_caps: self.all_caps }
    }
}

impl Render for TemplateEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let current = self.template(cx);
        let name = self.name.read(cx).value().to_string();
        let name_ok = !name.trim().is_empty();
        let html = name.contains('<') && name.contains('>');
        // Save stays off until something changed, with a column and a valid name.
        let can_save = name_ok && !html && !self.columns.is_empty() && current != self.initial;
        let status: AnyElement = if html {
            div().text_size(px(14.)).text_color(rgb(0xaa252d)).child("Field cannot contain HTML.").into_any_element()
        } else if !name_ok {
            div().text_size(px(14.)).text_color(rgb(0xaa252d)).child("Valid template name is required.").into_any_element()
        } else {
            div().text_size(px(14.)).text_color(pal.text).child(format!("{} characters remaining", NAME_MAX.saturating_sub(name.trim().chars().count()))).into_any_element()
        };
        // Add column: the properties not used yet.
        let unused: Vec<&'static str> = COLUMN_CHOICES.iter().copied().filter(|c| !self.columns.iter().any(|x| x == c)).collect();
        let menu_view = cx.entity();
        let add = div().ml(px(20.)).id("rt-add-column").tooltip(crate::ui::tooltip("Add columns to the template")).child(
            Button::new("rt-add-column-btn")
                .child(h_flex().gap(px(4.)).text_size(px(13.)).child("Add column").child(crate::ui::glyph("drawing-tool-caret", 10., pal.text)))
                .h(px(22.))
                .px(px(10.))
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(rgb(0xffffff))
                .os_dropdown_menu("rt-add-column-menu", false, move |mut m, _, _| {
                    if unused.is_empty() {
                        return m.item(crate::os_menu::OsMenuItem::new("No columns available").disabled(true));
                    }
                    for c in &unused {
                        let (v, c) = (menu_view.clone(), c.to_string());
                        m = m.item(crate::os_menu::OsMenuItem::new(c.clone()).on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.columns.push(c.clone());
                                cx.notify();
                            })
                        }));
                    }
                    m
                }),
        );
        let caps = self.all_caps;
        let boxes = self.columns.iter().enumerate().map(|(i, c)| {
            let removable = c != REVISION;
            div()
                .id(("rt-col", i))
                .cursor_move()
                .child(column_box(c, caps).when(removable, |d| {
                    d.child(
                        div()
                            .id(("rt-col-x", i))
                            .absolute()
                            .top(px(2.))
                            .right(px(2.))
                            .opacity(0.5)
                            .hover(|d| d.opacity(0.3))
                            .cursor_pointer()
                            .child(crate::ui::glyph("close", 10., rgb(0x333333)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if i < this.columns.len() {
                                    this.columns.remove(i);
                                    cx.notify();
                                }
                            })),
                    )
                }))
                .on_drag(ColumnDrag(i, c.clone().into()), |d, _, _, cx| cx.new(|_| d.clone()))
                .drag_over::<ColumnDrag>(|d, _, _, _| d.opacity(0.6))
                .on_drop(cx.listener(move |this, d: &ColumnDrag, _, cx| {
                    let from = d.0;
                    if from != i && from < this.columns.len() && i < this.columns.len() {
                        let c = this.columns.remove(from);
                        this.columns.insert(i, c);
                        cx.notify();
                    }
                }))
        });
        let check = h_flex()
            .id("rt-all-caps")
            .gap(px(4.))
            .cursor_pointer()
            .text_size(px(14.))
            .text_color(pal.text)
            .child(crate::preferences_window::PreferencesView::checkbox_box(self.all_caps))
            .child("All uppercase in Drawings")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.all_caps = !this.all_caps;
                cx.notify();
            }));
        v_flex()
            .min_h(px(385. - 47.))
            .child(
                v_flex()
                    .p(px(15.))
                    .gap(px(15.))
                    .child(
                        v_flex()
                            .gap(px(5.))
                            .child(os::field_label("Name"))
                            .child(gpui_kit::base::input::input_frame(div().h(px(34.)).border_1().rounded(px(4.)).border_color(rgb(0xaaaaaa)).flex().items_center().child(Input::new(&self.name).appearance(false).px(px(10.)).text_size(px(14.)))))
                            .child(status),
                    )
                    .child(
                        v_flex()
                            .child(h_flex().mb(px(10.)).items_end().child(os::field_label("Property columns")).child(add))
                            .child(div().id("rt-columns").overflow_x_scroll().child(h_flex().children(boxes))),
                    )
                    .child(check),
            )
            .child(div().flex_1())
            .child(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Save changes").disabled(!can_save))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
    }
}
