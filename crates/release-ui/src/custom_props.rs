//! Company settings › Custom properties (help Plans/enterprise_settings:
//! "create new/custom properties for use in company-owned documents"): the
//! list of definitions (Name, Type, Applies to, State, Description), Create
//! custom property and a row's Edit / Delete, in a form above the list.
//! Active properties show at the bottom of the Properties dialogs of what
//! they apply to and in drawings' Insert property lists (drawing.md A15).

use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_ops::custom_properties::{CustomKind, CustomProperty, CustomState, CustomTarget, custom_properties, set_custom_properties};

/// The Create / Edit form.
struct Form {
    /// The row edited (None: a new property).
    row: Option<usize>,
    name: Entity<InputState>,
    description: Entity<InputState>,
    /// Enum values, comma-separated.
    options: Entity<InputState>,
    default: Entity<InputState>,
    kind: usize,
    applies: Vec<CustomTarget>,
    state: CustomState,
    error: Option<String>,
}

const KINDS: [&str; 6] = ["String", "Integer", "Double", "Boolean", "Date", "Enum"];

pub struct CustomPropsView {
    list: Vec<CustomProperty>,
    form: Option<Form>,
}

impl CustomPropsView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { list: custom_properties(), form: None }
    }

    fn open_form(&mut self, row: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let p = row.and_then(|r| self.list.get(r)).cloned().unwrap_or_default();
        let input = |v: String, ph: &'static str, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(v));
        let options = match &p.kind {
            CustomKind::List(o) => o.join(", "),
            _ => String::new(),
        };
        let kind = KINDS.iter().position(|k| *k == p.kind.label()).unwrap_or(0);
        self.form = Some(Form {
            row,
            name: input(p.name.clone(), "Name", window, cx),
            description: input(p.description.clone(), "Description", window, cx),
            options: input(options, "Values, separated by commas", window, cx),
            default: input(p.default.clone(), "Default value", window, cx),
            kind,
            applies: p.applies.clone(),
            state: p.state,
            error: None,
        });
        cx.notify();
    }

    fn save_form(&mut self, cx: &mut Context<Self>) {
        let Some(f) = &mut self.form else { return };
        let t = |st: &Entity<InputState>| st.read(cx).value().trim().to_string();
        let name = t(&f.name);
        let kind = match f.kind {
            1 => CustomKind::Integer,
            2 => CustomKind::Number,
            3 => CustomKind::Boolean,
            4 => CustomKind::Date,
            5 => CustomKind::List(t(&f.options).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()),
            _ => CustomKind::Text,
        };
        let mut p = CustomProperty { name: name.clone(), description: t(&f.description), kind, applies: f.applies.clone(), state: f.state, default: String::new() };
        let err = if name.is_empty() {
            Some("Enter a name.".to_string())
        } else if self.list.iter().enumerate().any(|(i, q)| Some(i) != f.row && q.name.eq_ignore_ascii_case(&name)) {
            Some(format!("A property named \"{name}\" already exists."))
        } else if p.applies.is_empty() {
            Some("Choose what it applies to.".to_string())
        } else if matches!(&p.kind, CustomKind::List(o) if o.is_empty()) {
            Some("Enter its values.".to_string())
        } else {
            match p.check(&t(&f.default)) {
                Ok(d) => {
                    p.default = d;
                    None
                }
                Err(e) => Some(e),
            }
        };
        if let Some(e) = err {
            f.error = Some(e);
            cx.notify();
            return;
        }
        let created = f.row.is_none();
        match f.row {
            Some(r) if r < self.list.len() => self.list[r] = p,
            _ => self.list.push(p),
        }
        set_custom_properties(&self.list);
        self.form = None;
        crate::prefs_general::toast(if created { format!("Custom property {name} is created.") } else { format!("Custom property {name} is updated.") }, cx);
        cx.notify();
    }

    fn render_form(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.form.as_ref()?;
        let pal = p();
        let row = |label: &'static str, field: AnyElement| h_flex().gap(px(10.)).child(div().w(px(130.)).text_size(px(14.)).child(label)).child(div().w(px(320.)).child(field));
        let field = |st: &Entity<InputState>| Input::new(st).h(px(30.)).text_size(px(14.)).into_any_element();
        let kinds: Vec<String> = KINDS.iter().map(|s| s.to_string()).collect();
        let states: Vec<String> = CustomState::ALL.iter().map(|s| s.label().to_string()).collect();
        let state_at = CustomState::ALL.iter().position(|s| *s == f.state).unwrap_or(1);
        let view = cx.entity();
        let (v1, v2) = (view.clone(), view.clone());
        let applies = h_flex().gap(px(14.)).children(CustomTarget::ALL.iter().map(|&t| {
            let on = f.applies.contains(&t);
            h_flex()
                .id(SharedString::from(format!("cp-applies-{}", t.label())))
                .gap(px(6.))
                .cursor_pointer()
                .text_size(px(14.))
                .child(crate::preferences_window::PreferencesView::checkbox_box(on))
                .child(t.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(f) = &mut this.form {
                        if let Some(i) = f.applies.iter().position(|x| *x == t) {
                            f.applies.remove(i);
                        } else {
                            f.applies.push(t);
                        }
                    }
                    cx.notify();
                }))
        }));
        Some(
            v_flex()
                .gap(px(10.))
                .p(px(15.))
                .mb(px(15.))
                .border_1()
                .border_color(rgb(0xdde5ec))
                .rounded(px(4.))
                .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).child(if f.row.is_some() { "Edit custom property" } else { "Create custom property" }))
                .child(row("Name", field(&f.name)))
                .child(row("Description", field(&f.description)))
                .child(row(
                    "Type",
                    crate::os_ui::select_field("cp-kind", kinds, f.kind, move |i, _, cx| {
                        v1.update(cx, |this, cx| {
                            if let Some(f) = &mut this.form {
                                f.kind = i;
                            }
                            cx.notify();
                        })
                    })
                    .into_any_element(),
                ))
                .when(f.kind == 5, |e| e.child(row("Values", field(&f.options))))
                .child(row("Applies to", applies.into_any_element()))
                .child(row(
                    "State",
                    crate::os_ui::select_field("cp-state", states, state_at, move |i, _, cx| {
                        v2.update(cx, |this, cx| {
                            if let Some(f) = &mut this.form {
                                f.state = CustomState::ALL[i];
                            }
                            cx.notify();
                        })
                    })
                    .into_any_element(),
                ))
                .child(row("Default value", field(&f.default)))
                .children(f.error.clone().map(|e| div().text_size(px(13.)).text_color(rgb(0xaa252d)).child(e)))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(button("cp-save", if f.row.is_some() { "Save" } else { "Create" }, true).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save_form(cx))))
                        .child(button("cp-cancel", "Cancel", false).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.form = None;
                            cx.notify();
                        }))),
                )
                .text_color(pal.text)
                .into_any_element(),
        )
    }
}

fn button(id: &'static str, label: &'static str, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(12.))
        .py(px(6.))
        .rounded(px(4.))
        .text_size(px(14.))
        .cursor_pointer()
        .when(primary, |d| d.bg(rgb(0x1651b0)).text_color(rgb(0xffffff)))
        .when(!primary, |d| d.border_1().border_color(rgb(0xcccccc)).text_color(rgb(0x333333)))
        .child(label)
}

impl Render for CustomPropsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let head = |t: &'static str, w: f32| div().w(px(w)).flex_none().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(t);
        let mut table = v_flex().child(
            h_flex().h(px(32.)).px(px(8.)).gap(px(8.)).border_b_1().border_color(rgb(0x333333)).child(head("Name", 200.)).child(head("Type", 90.)).child(head("Applies to", 200.)).child(head("State", 80.)).child(head("Description", 260.)),
        );
        if self.list.is_empty() {
            table = table.child(div().p(px(10.)).text_size(px(14.)).text_color(pal.text_muted).child("No custom properties. Create one to add it to Properties dialogs."));
        }
        for (i, q) in self.list.iter().enumerate() {
            let cell = |s: String, w: f32| div().w(px(w)).flex_none().truncate().text_size(px(14.)).child(s);
            table = table.child(
                h_flex()
                    .id(("cp-row", i))
                    .h(px(32.))
                    .px(px(8.))
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(rgb(0xeeeeee))
                    .hover(|d| d.bg(rgb(0xf3f8fc)))
                    .child(cell(q.name.clone(), 200.))
                    .child(cell(q.kind.label().into(), 90.))
                    .child(cell(q.applies.iter().map(|t| t.label()).collect::<Vec<_>>().join(", "), 200.))
                    .child(cell(q.state.label().into(), 80.))
                    .child(cell(q.description.clone(), 260.))
                    .child(div().id(("cp-edit", i)).text_size(px(13.)).text_color(pal.accent).cursor_pointer().child("Edit").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_form(Some(i), window, cx))))
                    .child(div().id(("cp-delete", i)).pl(px(10.)).text_size(px(13.)).text_color(rgb(0xaa252d)).cursor_pointer().child("Delete").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if i < this.list.len() {
                            this.list.remove(i);
                            set_custom_properties(&this.list);
                            this.form = None;
                        }
                        cx.notify();
                    }))),
            );
        }
        v_flex()
            .gap(px(12.))
            .child(
                h_flex()
                    .h(px(32.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(pal.text)
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).child("Custom properties"))
                    .child(div().flex_1())
                    .when(self.form.is_none(), |e| e.child(button("cp-create", "Create custom property", true).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, window, cx))))),
            )
            .children(self.render_form(cx))
            .child(table)
    }
}
