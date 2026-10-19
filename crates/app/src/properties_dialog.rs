//! Onshape's metadata "Properties" modal (parts and assemblies; live
//! assembly/100–102, dom/dialog-assembly-part-properties.txt,
//! dom/dialog-assembly-top-properties.txt) and the small "Instance
//! properties" modal (dom/dialog-assembly-instance-properties.txt).
//!
//! An 800 px Bootstrap modal 15 px from the top: a 57 px header with the
//! 24 px title, a sub-header with the item's name (18 px, `#ccc` rule), a
//! scrolling form of bold 14 px labels over full-width 34 px fields, and a
//! footer with form undo / redo at the left and Apply, Save (disabled until
//! edited) and Close at the right.

use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::partstudio::{ItemMeta, RELEASE_STATES};
use std::rc::Rc;

/// Properties › Unit of measure, in Onshape's order (live assembly/131;
/// the first option is blank).
pub const UNITS_OF_MEASURE: [&str; 18] = [
    "", "Centimeter", "Foot", "Inch", "Meter", "Millimeter", "Yard", "Gram", "Kilogram", "Ounce", "Pound", "Liter", "Gallon", "Each", "Fluid ounce", "Milliliter", "Centiliter", "Package",
];
/// Properties › Mass override: "2.5 kg", "2.5", "5.5 lb" → kg.
pub fn parse_mass(text: &str) -> Option<f64> {
    let t = text.trim();
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len()));
    let v: f64 = num.trim().parse().ok()?;
    let k = match unit.trim() {
        "" | "kg" => 1.0,
        "g" => 1e-3,
        "lb" | "lbm" => 0.453_592_37,
        "oz" => 0.028_349_523_125,
        _ => return None,
    };
    (v > 0.0).then_some(v * k)
}

/// A new item's unit of measure.
pub const DEFAULT_UNIT: &str = "Each";

/// The values the Properties modal edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetaValues {
    pub name: String,
    pub description: String,
    pub part_number: String,
    pub revision: String,
    pub meta: ItemMeta,
    /// Mass › Override (text in the document's mass unit), `None` = computed.
    pub mass_override: Option<String>,
    /// Subassembly BOM behavior (assemblies), index into the options.
    pub bom: usize,
}

/// What the modal shows besides the values.
pub struct MetaSpec {
    /// "Onshape Part" / "Onshape Assembly".
    pub category: &'static str,
    /// Parts: the assigned material ("Unassigned") and the computed mass.
    pub material: Option<String>,
    pub computed_mass: Option<String>,
    /// Assemblies: the "Subassembly BOM behavior" options.
    pub bom_options: Option<Vec<String>>,
    pub values: MetaValues,
}

type SaveFn = Rc<dyn Fn(MetaValues, &mut Window, &mut App)>;
/// Opens the Create Release candidate dialog for the item.
type ReleaseFn = Rc<dyn Fn(&mut Window, &mut App)>;

/// The form inside the modal.
pub struct MetaForm {
    /// Release management: the release-create icon at the top left
    /// (Release__typical_release_workflow, "From any Properties dialog").
    on_release: Option<ReleaseFn>,
    /// Part Studio parts: "Reset all" restores these values (live
    /// dom/dialog-partstudio-part-properties.txt; absent for assembly parts).
    reset_to: Option<MetaValues>,
    spec_category: &'static str,
    material: Option<String>,
    computed_mass: Option<String>,
    bom_options: Option<Vec<String>>,
    name: Entity<InputState>,
    description: Entity<TextareaState>,
    part_number: Entity<InputState>,
    revision: Entity<InputState>,
    vendor: Entity<InputState>,
    project: Entity<InputState>,
    product_line: Entity<InputState>,
    titles: [Entity<InputState>; 3],
    mass: Entity<InputState>,
    /// Values not held by text inputs.
    state: Option<String>,
    not_revision_managed: bool,
    exclude_from_bom: bool,
    unit: Option<String>,
    mass_on: bool,
    /// Calculate value was clicked (the button goes away) / it failed.
    mass_computed: bool,
    mass_failed: bool,
    bom: usize,
    /// Saved values (Save / Apply enabled while the form differs).
    saved: MetaValues,
    /// Form undo history and the current position in it.
    history: Vec<MetaValues>,
    at: usize,
    restoring: bool,
    on_save: SaveFn,
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl MetaForm {
    fn new(spec: MetaSpec, on_save: SaveFn, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let v = spec.values.clone();
        let text = |s: &str, window: &mut Window, cx: &mut Context<Self>| {
            let s = s.to_string();
            cx.new(|cx| InputState::new(window, cx).default_value(s))
        };
        let opt = |s: &Option<String>| s.clone().unwrap_or_default();
        let name = text(&v.name, window, cx);
        let description = {
            let d = v.description.clone();
            cx.new(|cx| TextareaState::new(window, cx).rows(2).default_value(d))
        };
        let part_number = text(&v.part_number, window, cx);
        let revision = text(&v.revision, window, cx);
        let vendor = text(&opt(&v.meta.vendor), window, cx);
        let project = text(&opt(&v.meta.project), window, cx);
        let product_line = text(&opt(&v.meta.product_line), window, cx);
        let titles = [text(&opt(&v.meta.title1), window, cx), text(&opt(&v.meta.title2), window, cx), text(&opt(&v.meta.title3), window, cx)];
        let mass = {
            let m = v.mass_override.clone().unwrap_or_default();
            cx.new(|cx| InputState::new(window, cx).placeholder("Computed value").default_value(m))
        };
        let mut subs = Vec::new();
        for st in [&name, &part_number, &revision, &vendor, &project, &product_line, &titles[0], &titles[1], &titles[2], &mass] {
            subs.push(cx.subscribe(st, |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.changed(cx);
                }
            }));
        }
        subs.push(cx.subscribe(&description, |this: &mut Self, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.changed(cx);
            }
        }));
        let mass_on = v.mass_override.is_some();
        if !mass_on {
            mass.update(cx, |s, cx| s.set_disabled(true, cx));
        }
        Self {
            on_release: None,
            reset_to: None,
            spec_category: spec.category,
            material: spec.material,
            computed_mass: spec.computed_mass,
            bom_options: spec.bom_options,
            name,
            description,
            part_number,
            revision,
            vendor,
            project,
            product_line,
            titles,
            mass,
            state: v.meta.state.clone(),
            not_revision_managed: v.meta.not_revision_managed,
            exclude_from_bom: v.meta.exclude_from_bom,
            unit: v.meta.unit_of_measure.clone(),
            mass_on,
            mass_computed: false,
            mass_failed: false,
            bom: v.bom,
            saved: v.clone(),
            history: vec![v],
            at: 0,
            restoring: false,
            on_save,
            scroll: ScrollHandle::new(),
            _subs: subs,
        }
    }

    /// The form's current values.
    pub fn values(&self, cx: &App) -> MetaValues {
        let t = |s: &Entity<InputState>| s.read(cx).value().trim().to_string();
        let o = |s: &Entity<InputState>| Some(s.read(cx).value().trim().to_string()).filter(|x| !x.is_empty());
        MetaValues {
            name: t(&self.name),
            description: self.description.read(cx).value().to_string(),
            part_number: t(&self.part_number),
            revision: t(&self.revision),
            meta: ItemMeta {
                state: self.state.clone(),
                vendor: o(&self.vendor),
                project: o(&self.project),
                product_line: o(&self.product_line),
                title1: o(&self.titles[0]),
                title2: o(&self.titles[1]),
                title3: o(&self.titles[2]),
                not_revision_managed: self.not_revision_managed,
                exclude_from_bom: self.exclude_from_bom,
                unit_of_measure: self.unit.clone(),
            },
            mass_override: self.mass_on.then(|| t(&self.mass)),
            bom: self.bom,
        }
    }

    /// Record an edit in the form's undo history.
    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        let v = self.values(cx);
        if self.history.get(self.at) != Some(&v) {
            self.history.truncate(self.at + 1);
            self.history.push(v);
            self.at = self.history.len() - 1;
        }
        cx.notify();
    }

    fn restore(&mut self, v: MetaValues, window: &mut Window, cx: &mut Context<Self>) {
        self.restoring = true;
        let set = |st: &Entity<InputState>, s: String, window: &mut Window, cx: &mut Context<Self>| st.update(cx, |st, cx| st.set_value(s, window, cx));
        set(&self.name, v.name.clone(), window, cx);
        self.description.update(cx, |st, cx| st.set_value(v.description.clone(), window, cx));
        set(&self.part_number, v.part_number.clone(), window, cx);
        set(&self.revision, v.revision.clone(), window, cx);
        set(&self.vendor, v.meta.vendor.clone().unwrap_or_default(), window, cx);
        set(&self.project, v.meta.project.clone().unwrap_or_default(), window, cx);
        set(&self.product_line, v.meta.product_line.clone().unwrap_or_default(), window, cx);
        set(&self.titles[0], v.meta.title1.clone().unwrap_or_default(), window, cx);
        set(&self.titles[1], v.meta.title2.clone().unwrap_or_default(), window, cx);
        set(&self.titles[2], v.meta.title3.clone().unwrap_or_default(), window, cx);
        set(&self.mass, v.mass_override.clone().unwrap_or_default(), window, cx);
        self.state = v.meta.state.clone();
        self.not_revision_managed = v.meta.not_revision_managed;
        self.exclude_from_bom = v.meta.exclude_from_bom;
        self.unit = v.meta.unit_of_measure.clone();
        self.mass_on = v.mass_override.is_some();
        let on = self.mass_on;
        self.mass.update(cx, |s, cx| s.set_disabled(!on, cx));
        self.bom = v.bom;
        self.restoring = false;
        cx.notify();
    }

    fn undo(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let to = if redo { self.at + 1 } else { self.at.wrapping_sub(1) };
        let Some(v) = self.history.get(to).cloned() else { return };
        self.at = to;
        self.restore(v, window, cx);
    }

    fn dirty(&self, cx: &App) -> bool {
        let v = self.values(cx);
        v != self.saved && !v.name.is_empty()
    }

    /// Apply (keep the modal open) or Save (close it).
    fn save(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty(cx) {
            return;
        }
        let v = self.values(cx);
        self.saved = v.clone();
        (self.on_save)(v, window, cx);
        if close {
            window.close_dialog(cx);
        }
        cx.notify();
    }
}

/// A bold 14 px label, with a red "*" for required fields.
fn label(text: &'static str, required: bool) -> Div {
    h_flex()
        .h(px(21.))
        .gap(px(3.))
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0x333333))
        .child(text)
        .when(required, |d| d.child(div().text_color(rgb(0xaa252d)).child("*")))
}

/// A disabled-looking field (`#eee`, `#ccc` border) with an optional caret.
fn gray_box(text: String, caret: bool) -> Div {
    h_flex()
        .h(px(34.))
        .w_full()
        .pl(px(10.))
        .pr(px(10.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgb(0xcccccc))
        .bg(rgb(0xeeeeee))
        .text_size(px(14.))
        .text_color(rgb(0x333333))
        .child(div().flex_1().truncate().child(text))
        .when(caret, |d| d.child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x666666))))
}

fn text_field(st: &Entity<InputState>) -> AnyElement {
    Input::new(st).h(px(34.)).text_size(px(14.)).into_any_element()
}

/// A Bootstrap checkbox row ("☐ Not revision managed").
fn check_row(id: &'static str, text: &'static str, on: bool) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(px(21.))
        .gap(px(6.))
        .cursor_pointer()
        .text_size(px(14.))
        .text_color(rgb(0x333333))
        .child(
            div()
                .size(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.5))
                .border_1()
                .border_color(if on { rgb(0x1651b0) } else { rgb(0xaaaaaa) })
                .bg(if on { rgb(0x1651b0) } else { rgb(0xffffff) })
                .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(rgb(0xffffff)))),
        )
        .child(text)
}

/// A footer button: Apply / Save (`#1651b0`, 40 % until edited) or Close (`#d7d7d7`).
fn footer_button(id: &'static str, text: &'static str, primary: bool, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(34.))
        .px(px(10.5))
        .flex()
        .items_center()
        .rounded(px(5.25))
        .text_size(px(14.))
        .when(primary, |d| d.bg(rgb(0x1651b0)).text_color(rgb(0xffffff)))
        .when(!primary, |d| d.bg(rgb(0xd7d7d7)).text_color(rgb(0x333333)))
        .when(!enabled, |d| d.opacity(0.4))
        .when(enabled, |d| d.cursor_pointer())
        .child(text)
}

impl Render for MetaForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let dirty = self.dirty(cx);
        let view = cx.entity();
        let group = |l: Div, f: AnyElement| v_flex().w_full().gap(px(0.)).child(l).child(div().p(px(1.)).child(f));
        let state_ix = self.state.as_deref().and_then(|s| RELEASE_STATES.iter().position(|x| *x == s)).unwrap_or(0);
        let unit = self.unit.clone().unwrap_or_else(|| DEFAULT_UNIT.to_string());
        let unit_ix = UNITS_OF_MEASURE.iter().position(|u| *u == unit).unwrap_or(0);
        let release = self.on_release.clone().map(|f| {
            h_flex().child(
                div()
                    .id("props-release")
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                    .child(crate::assets::icon("release-create", 20., cx))
                    .tooltip(crate::ui::tooltip("Create release candidate"))
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        f(window, cx)
                    }),
            )
        });
        let mut form = v_flex()
            .w_full()
            .gap(px(15.))
            .children(release)
            .child(group(label("Name", true), text_field(&self.name)))
            .child(group(label("Description", false), Textarea::new(&self.description).h(px(56.)).text_size(px(14.)).into_any_element()))
            .child(group(label("Category", false), gray_box(self.spec_category.to_string(), true).into_any_element()))
            .child(group(label("Part number", false), text_field(&self.part_number)))
            .child(group(label("Revision", false), text_field(&self.revision)))
            // State is set by the release workflow (a greyed select).
            .child(group(label("State", true), gray_box(RELEASE_STATES[state_ix].to_string(), true).into_any_element()))
            .child(group(label("Vendor", false), text_field(&self.vendor)))
            .child(group(label("Project", false), text_field(&self.project)))
            .child(group(label("Product line", false), text_field(&self.product_line)));
        if let Some(m) = self.material.clone() {
            form = form.child(group(label("Material", false), gray_box(m, false).into_any_element()));
        }
        form = form
            .child(group(label("Title 1", false), text_field(&self.titles[0])))
            .child(group(label("Title 2", false), text_field(&self.titles[1])))
            .child(group(label("Title 3", false), text_field(&self.titles[2])))
            .child(check_row("meta-nrm", "Not revision managed", self.not_revision_managed).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.not_revision_managed = !this.not_revision_managed;
                this.changed(cx);
            })))
            .child(check_row("meta-bom", "Exclude from all BOMs", self.exclude_from_bom).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.exclude_from_bom = !this.exclude_from_bom;
                this.changed(cx);
            })))
            .child(group(
                label("Unit of measure", false),
                crate::os_ui::select_field("meta-unit", UNITS_OF_MEASURE.iter().map(|s| s.to_string()).collect(), unit_ix, {
                    let v = view.clone();
                    move |i, _, cx| {
                        v.update(cx, |this, cx| {
                            this.unit = Some(UNITS_OF_MEASURE[i].to_string());
                            this.changed(cx);
                        })
                    }
                })
                .into_any_element(),
            ));
        if self.material.is_some() {
            let mass_on = self.mass_on;
            let computed = self.computed_mass.clone();
            form = form.child(
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(label("Mass", false))
                            .child(svg().path(crate::assets::icon_path("function")).size(px(14.)).text_color(rgb(0x333333)))
                            .when(self.mass_failed, |d| d.child(div().text_size(px(14.)).text_color(rgb(0xaa252d)).child("Computation failed")))
                            .child(div().w(px(10.)))
                            .child(
                                check_row("meta-mass", "Override", mass_on).font_weight(FontWeight::NORMAL).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.mass_on = !this.mass_on;
                                    let on = this.mass_on;
                                    this.mass.update(cx, |s, cx| s.set_disabled(!on, cx));
                                    this.changed(cx);
                                })),
                            ),
                    )
                    .child(
                        h_flex()
                            .p(px(1.))
                            .child(div().flex_1().when(!mass_on, |d| d.bg(rgb(0xeeeeee))).child(text_field(&self.mass)))
                            .when(!self.mass_computed, |d| d.child(
                                div()
                                    .id("meta-calc")
                                    .w(px(36.))
                                    .h(px(34.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .border_color(rgb(0xaaaaaa))
                                    .rounded_r(px(3.5))
                                    .bg(rgb(0xffffff))
                                    .cursor_pointer()
                                    .tooltip(crate::ui::tooltip("Calculate value"))
                                    .child(svg().path(crate::assets::icon_path("calculate-button")).size(px(20.)).text_color(rgb(0x333333)))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                        // Without a material Onshape reports "Computation
                                        // failed" / "No material assigned." and hides the button.
                                        let text = computed.clone().unwrap_or_else(|| "No material assigned.".into());
                                        this.mass_failed = computed.is_none();
                                        this.mass_computed = true;
                                        this.mass.update(cx, |s, cx| s.set_placeholder(text, window, cx));
                                        cx.notify();
                                    })),
                            )),
                    ),
            );
        }
        if let Some(options) = self.bom_options.clone() {
            let v = view.clone();
            form = form.child(group(
                label("Subassembly BOM behavior", true),
                crate::os_ui::select_field("meta-bom-behavior", options, self.bom, move |i, _, cx| {
                    v.update(cx, |this, cx| {
                        this.bom = i;
                        this.changed(cx);
                    })
                })
                .into_any_element(),
            ));
        }
        let (can_undo, can_redo) = (self.at > 0, self.at + 1 < self.history.len());
        let undo_button = |id: &'static str, icon: &'static str, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .when(!on, |d| d.opacity(0.25))
                .when(on, |d| d.cursor_pointer().hover(|d| d.bg(rgb(0xeeeeee))))
                .tooltip(crate::ui::tooltip(tip))
                .child(svg().path(crate::assets::icon_path(icon)).size(px(20.)).text_color(rgb(0x333333)))
        };
        v_flex()
            .w_full()
            // The kit dialog puts 8 px between title and body.
            .mt(px(-8.))
            .text_color(pal.text)
            // The item's name under the title, over a #ccc rule.
            .child(
                div()
                    .mt(px(10.))
                    .h(px(31.))
                    .px(px(15.))
                    .border_b_1()
                    .border_color(rgb(0xcccccc))
                    .child(div().pl(px(4.)).text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).truncate().child(self.saved.name.clone())),
            )
            .child(div().id("meta-scroll").h(px(381.)).overflow_y_scroll().track_scroll(&self.scroll).p(px(15.)).child(form))
            .child(
                h_flex()
                    .h(px(62.))
                    .px(px(10.))
                    .border_t_1()
                    .border_color(rgb(0xdee2e6))
                    .child(
                        h_flex()
                            .gap(px(9.))
                            .pl(px(3.))
                            .child(undo_button("meta-undo", "undo", "Undo", can_undo).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(false, window, cx))))
                            .child(undo_button("meta-redo", "redo", "Redo", can_redo).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(true, window, cx))))
                            .when(self.reset_to.is_some(), |d| {
                                d.child(
                                    footer_button("meta-reset", "Reset all", true, true)
                                        .tooltip(crate::ui::tooltip("Reset all properties"))
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                            if let Some(v) = this.reset_to.clone() {
                                                this.restore(v, window, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .pr(px(4.))
                            .child(footer_button("meta-apply", "Apply", true, dirty).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(false, window, cx))))
                            .child(footer_button("meta-save", "Save", true, dirty).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(true, window, cx))))
                            .child(footer_button("meta-close", "Close", false, true).on_click(|_, window, cx| window.close_dialog(cx))),
                    ),
            )
    }
}

/// The 57 px Bootstrap modal header with a 24 px / 500 title.
fn modal_header(title: &'static str, width: f32) -> Div {
    h_flex()
        .w(px(width - 2.))
        .h(px(57.))
        .px(px(10.))
        .border_b_1()
        .border_color(rgb(0xdee2e6))
        .child(div().text_size(px(24.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).child(title))
}

fn bootstrap_modal(dialog: gpui_kit::component::dialog::Dialog, title: &'static str, width: f32, top: f32) -> gpui_kit::component::dialog::Dialog {
    dialog.w(px(width)).margin_top(px(top)).close_button(false).p_0().gap_0().rounded(px(6.)).border_color(rgb(0xdee2e6)).title(modal_header(title, width))
}

/// Open the Properties modal; `on_save` runs on Apply and Save.
pub fn open_properties(spec: MetaSpec, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let form = cx.new(|cx| MetaForm::new(spec, on_save, window, cx));
    window.open_dialog(cx, move |dialog, _, _| bootstrap_modal(dialog, "Properties", 800., 15.).child(form.clone()));
}

/// [`open_properties`] with the release-create icon (`on_release` opens the
/// Create Release candidate dialog for the item).
pub fn open_properties_with_release(spec: MetaSpec, reset_to: Option<MetaValues>, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, on_release: impl Fn(&mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let on_release: ReleaseFn = Rc::new(on_release);
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, on_save, window, cx);
        f.on_release = Some(on_release);
        f.reset_to = reset_to;
        f
    });
    window.open_dialog(cx, move |dialog, _, _| bootstrap_modal(dialog, "Properties", 800., 15.).child(form.clone()));
}

/// Instance properties…: a 600 px modal with the instance's name and its
/// "Reference designator" (focused); Save (disabled until edited), Close.
pub fn open_instance_properties(instance: String, designator: String, on_save: impl Fn(String, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let st = cx.new(|cx| InputState::new(window, cx).default_value(designator.clone()));
    let on_save = Rc::new(on_save);
    let focus = st.clone();
    // Save turns on as soon as the field differs.
    cx.subscribe(&st, |_, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            cx.refresh_windows();
        }
    })
    .detach();
    window.open_dialog(cx, move |dialog, _, cx| {
        let value = st.read(cx).value().trim().to_string();
        let dirty = value != designator;
        let (st2, on_save) = (st.clone(), on_save.clone());
        bootstrap_modal(dialog, "Instance properties", 600., 25.).child(
            v_flex()
                .w_full()
                .mt(px(-8.))
                .child(
                    v_flex()
                        .py(px(10.))
                        .gap(px(11.))
                        .child(div().px(px(16.)).pb(px(8.)).border_b_1().border_color(rgb(0xcccccc)).text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).child(instance.clone()))
                        .child(
                            v_flex()
                                .px(px(16.))
                                .gap(px(5.))
                                .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(rgb(0x333333)).child("Reference designator"))
                                .child(Input::new(&st).h(px(34.)).text_size(px(14.))),
                        ),
                )
                .child(
                    h_flex()
                        .h(px(58.))
                        .px(px(14.))
                        .gap(px(8.))
                        .justify_end()
                        .border_t_1()
                        .border_color(rgb(0xdee2e6))
                        .child(footer_button("ip-save", "Save", true, dirty).on_click(move |_, window, cx| {
                            if dirty {
                                let v = st2.read(cx).value().trim().to_string();
                                on_save(v, window, cx);
                                window.close_dialog(cx);
                            }
                        }))
                        .child(footer_button("ip-close", "Close", false, true).on_click(|_, window, cx| window.close_dialog(cx))),
                ),
        )
    });
    window.defer(cx, move |window, cx| focus.update(cx, |s, cx| s.focus(window, cx)));
}
