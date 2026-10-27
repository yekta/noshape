//! Model definitions › default tolerances and the Characteristics filters
//! (live qa4 10-20 07:40, part-studio/596–599): the "Isolate active part"
//! button, the "Select default tolerances…" field (its picker applies
//! ISO 2768-c as it opens; the field then becomes a removable chip), the
//! "▾ Default tolerances" section, and the Characteristics header icons
//! (Hide default tolerances, Types ▾, Show derived tolerances, Reset
//! filters).

use super::PartStudioView;
use super::behaviors::{IsolateMode, Isolation};
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::annotations::{self as ann, Characteristic};

/// The Types ▾ checklist (live 599-click-types).
pub const CHARACTERISTIC_TYPES: [&str; 9] = ["Distance", "Angle", "Radius", "Diameter", "Chamfer", "Fillet", "Hole", "Weld", "GTol"];

/// The panel's view state (not saved with the document).
#[derive(Default)]
pub struct InspectionView {
    /// The "Select default tolerances" picker is open.
    pub picker: bool,
    /// Its tab: 0 Current document, 1 Other documents, 2 Default tolerances.
    pub picker_tab: u8,
    /// "Hide default tolerances" pressed.
    pub hide_default: bool,
    /// "Show derived tolerances" released (pressed by default).
    pub derived_off: bool,
    /// Ticked Types (none ticked: every type shows).
    pub types: Vec<&'static str>,
    pub types_open: bool,
    /// Characteristics: the hovered cell (row, column) and its feature, the
    /// clicked cell.
    pub hover_cell: Option<(usize, usize)>,
    pub hover_feature: Option<noshape_model::reference::FeatureId>,
    pub focus_cell: Option<(usize, usize)>,
    /// The Types button's bounds (the list drops from its bottom-left).
    pub types_at: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
}

impl InspectionView {
    /// A filter differs from the defaults (Reset filters turns active).
    pub fn filtered(&self) -> bool {
        self.hide_default || self.derived_off || !self.types.is_empty()
    }

    pub fn shows(&self, c: &Characteristic) -> bool {
        // Live (psb 10-21 d): a "Fillet radius" row matches both the Fillet
        // and the Radius type.
        let kinds: &[&str] = if c.kind == "Fillet radius" { &["Fillet", "Radius"] } else { std::slice::from_ref(&c.kind) };
        (self.types.is_empty() || kinds.iter().any(|k| self.types.contains(k))) && !(self.hide_default && c.is_default())
    }
}

/// A 26 px icon button of the panel; `pressed` is Onshape's light-blue
/// pressed look.
fn icon_button(id: &'static str, icon: &'static str, pressed: bool, cx: &App) -> Stateful<Div> {
    let pal = p();
    div()
        .id(id)
        .size(px(26.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .cursor_pointer()
        .when(pressed, |d| d.bg(rgb(0x9fd0f5)).border_1().border_color(rgb(0x5aa9e6)))
        .when(!pressed, |d| d.hover(|d| d.bg(pal.row_hover)))
        .child(crate::assets::icon(icon, 20., cx))
}

impl PartStudioView {
    fn applied_default_tolerances(&self, cx: &App) -> Option<String> {
        self.studio(cx).and_then(|s| s.default_tolerances.clone())
    }

    fn set_default_tolerances(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if self.applied_default_tolerances(cx) == name {
            return;
        }
        let desc = if name.is_some() { "Select default tolerances" } else { "Remove default tolerances" };
        self.edit_config(desc, cx, |ps| ps.default_tolerances = name);
        cx.notify();
    }

    /// Row 1 after "Model Part 1 ▾": Isolate active part (tooltip "Show
    /// entire Part Studio" while isolated) and the default tolerances field
    /// or chip (live 598, 599-click-copy).
    pub(super) fn render_insp_model_tools(&mut self, part: Option<noshape_model::reference::BodyId>, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let isolated = self.ui.isolate.is_some();
        let isolate = icon_button("insp-isolate", "isolate", isolated, cx)
            .tooltip(crate::ui::tooltip(if isolated { "Show entire Part Studio" } else { "Isolate active part" }))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if !this.exit_isolate(window, cx)
                    && let Some(b) = part.clone()
                {
                    this.ui.isolate = Some(Isolation { mode: IsolateMode::Isolate, bodies: vec![b], opacity: 0.0, select_transparent: false, expand: 0 });
                    let d = this.display.clone();
                    this.apply_display(d, window, cx);
                }
            }));
        let field = match self.applied_default_tolerances(cx) {
            Some(name) => h_flex()
                .id("insp-default-tol")
                .group("dtol-chip")
                .h(px(26.))
                .pl(px(4.))
                .gap(px(4.))
                .border_1()
                .border_color(rgb(0xcccccc))
                .rounded(px(2.))
                .text_size(px(13.33))
                .text_color(pal.text)
                .child(crate::assets::icon("insp-default-tol", 18., cx))
                .child(div().w(px(100.)).child(name))
                .child(
                    div()
                        .id("insp-default-tol-remove")
                        .w(px(22.))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(rgb(0x666666))
                        // Live: the × shows on hover only.
                        .invisible()
                        .group_hover("dtol-chip", |d| d.visible())
                        .child("×")
                        .tooltip(crate::ui::tooltip("Remove"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.set_default_tolerances(None, cx);
                        })),
                ),
            None => h_flex()
                .id("insp-default-tol")
                .w(px(199.))
                .h(px(26.))
                .pl(px(4.))
                .gap(px(4.))
                .border_1()
                .border_color(rgb(0xcccccc))
                .rounded(px(2.))
                .cursor_pointer()
                .text_size(px(13.33))
                .text_color(rgb(0x999999))
                .hover(|d| d.border_color(pal.accent))
                .child(crate::assets::icon("insp-default-tol", 18., cx))
                .child("Select default tolerances…"),
        }
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_default_tolerances_picker(cx)));
        h_flex().gap(px(10.)).child(isolate).child(field)
    }

    /// Live 599: opening the picker already applies ISO 2768-c.
    fn open_default_tolerances_picker(&mut self, cx: &mut Context<Self>) {
        self.ui.insp.picker = true;
        self.ui.insp.picker_tab = 2;
        if self.applied_default_tolerances(cx).is_none() {
            self.set_default_tolerances(Some(ann::DEFAULT_TOLERANCE_CLASS.into()), cx);
        }
        cx.notify();
    }

    /// The Characteristics header icons (live 598–599): Hide / Show default
    /// tolerances, Types ▾, Show derived tolerances (pressed by default) |
    /// Reset filters (greyed until a filter changes).
    pub(super) fn render_characteristics_tools(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let v = &self.ui.insp;
        let hide = v.hide_default;
        let filtered = v.filtered();
        let types_open = v.types_open;
        let ticked = v.types.clone();
        let probe = v.types_at.clone();
        let at = v.types_at.get();
        let default_tol = icon_button("insp-hide-default", "insp-default-tol", hide, cx)
            .tooltip(crate::ui::tooltip(if hide { "Show default tolerances" } else { "Hide default tolerances" }))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.ui.insp.hide_default = !this.ui.insp.hide_default;
                cx.notify();
            }));
        let types = div()
            .id("insp-types")
            .relative()
            .h(px(26.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(3.))
            .border_1()
            .border_color(if types_open { pal.accent } else { rgb(0xbbbbbb) })
            // Live 10-21 c: a ticked type turns the button blue with a count
            // badge.
            .bg(if ticked.is_empty() { rgb(0xffffff) } else { rgb(0x9fd0f5) })
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(pal.text)
            .child("Types")
            .when(!ticked.is_empty(), |d| {
                d.child(div().min_w(px(16.)).h(px(16.)).px(px(4.)).rounded(px(8.)).bg(pal.accent).text_color(gpui_kit::rgb_static(0xffffff)).text_size(px(11.)).flex().items_center().justify_center().child(ticked.len().to_string()))
            })
            .child(crate::ui::glyph("ui-caret-down", 6., rgb(0x666666)))
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .when(!types_open, |d| d.tooltip(crate::ui::tooltip("Select dimension and annotation types to show")))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.ui.insp.types_open = !this.ui.insp.types_open;
                cx.notify();
            }))
            .when(types_open, |d| {
                d.child(
                    deferred(anchored().position(point(at.origin.x, at.bottom() + px(2.))).child(
                        v_flex()
                            .id("insp-types-list")
                            .occlude()
                            .w(px(138.))
                            .py(px(2.))
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(pal.menu_outline)
                            .shadow_md()
                            .children(CHARACTERISTIC_TYPES.iter().map(|t| {
                                let t: &'static str = t;
                                crate::os_ui::checkbox(SharedString::from(format!("insp-type-{t}")), t, ticked.contains(&t)).h(px(32.)).text_size(px(14.)).text_color(pal.text).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let list = &mut this.ui.insp.types;
                                    if let Some(i) = list.iter().position(|x| *x == t) {
                                        list.remove(i);
                                    } else {
                                        list.push(t);
                                    }
                                    cx.notify();
                                }))
                            }))
                            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                this.ui.insp.types_open = false;
                                cx.notify();
                            })),
                    ))
                    .with_priority(3),
                )
            });
        let derived_on = !self.ui.insp.derived_off;
        // Live: pressed "Hide derived tolerances", released "Show derived tolerances".
        let derived = icon_button("insp-derived", "insp-derived", derived_on, cx).tooltip(crate::ui::tooltip(if derived_on { "Hide derived tolerances" } else { "Show derived tolerances" })).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.insp.derived_off = !this.ui.insp.derived_off;
            cx.notify();
        }));
        let reset = div()
            .id("insp-reset-filters")
            .size(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .when(!filtered, |d| d.opacity(0.4))
            .when(filtered, |d| d.cursor_pointer().hover(|d| d.bg(pal.row_hover)))
            .child(crate::assets::icon("reset-default", 20., cx))
            .tooltip(crate::ui::tooltip("Reset filters"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                let v = &mut this.ui.insp;
                v.hide_default = false;
                v.derived_off = false;
                v.types.clear();
                cx.notify();
            }));
        h_flex().ml(px(16.)).gap(px(8.)).child(default_tol).child(types).child(derived).child(div().w(px(1.)).h(px(20.)).bg(rgb(0xcccccc))).child(reset)
    }

    /// "▾ Default tolerances" (live 599): each group's title and sizes
    /// right-aligned to the panel's middle, the values left-aligned after
    /// it, groups split by a rule.
    pub(super) fn render_default_tolerances_section(&self, cx: &App) -> Option<AnyElement> {
        let name = self.applied_default_tolerances(cx)?;
        let groups = ann::default_tolerance_groups(&name)?;
        let pal = p();
        let row = |l: String, r: String, bold: bool| {
            h_flex()
                .h(px(21.))
                .text_size(px(14.))
                .text_color(pal.text)
                .child(div().flex_1().flex().justify_end().pr(px(10.)).when(bold, |d| d.font_weight(FontWeight::SEMIBOLD)).child(l))
                .child(div().flex_1().pl(px(10.)).child(r))
        };
        let n = groups.len();
        Some(
            v_flex()
                .children(groups.into_iter().enumerate().map(|(i, g)| {
                    v_flex()
                        .pt(px(22.))
                        .pb(px(20.))
                        .when(i + 1 < n, |d| d.border_b_1().border_color(rgb(0x333333)))
                        .child(row(g.title.to_string(), String::new(), true))
                        .children(g.rows.into_iter().map(|(l, r)| row(l, r, false)))
                }))
                .into_any_element(),
        )
    }

    /// "Select default tolerances ✕" (live 599-click-defaultTol): 300 px,
    /// docked at the viewport's right edge, tabs Current document | Other
    /// documents | Default tolerances, search, Library / Type / Default
    /// tolerances selects, the preview, [Select].
    pub(super) fn render_default_tolerances_picker(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.ui.insp.picker {
            return None;
        }
        let pal = p();
        let tab = self.ui.insp.picker_tab;
        let applied = self.applied_default_tolerances(cx).unwrap_or_default();
        let precision = applied == ann::PRECISION_STANDARD;
        let tab_button = |i: u8, l1: &'static str, l2: &'static str, cx: &mut Context<Self>| {
            v_flex()
                .id(SharedString::from(format!("dtol-tab-{i}")))
                .flex_1()
                .h(px(46.))
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_size(px(12.))
                .line_height(px(22.))
                .text_color(if tab == i { pal.accent } else { pal.text })
                .when(tab == i, |d| d.border_b_2().border_color(pal.accent))
                .child(l1)
                .child(l2)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.insp.picker_tab = i;
                    cx.notify();
                }))
        };
        let select = |id: &'static str, label: &'static str, value: String, options: Vec<&'static str>, cx: &mut Context<Self>| {
            let v = cx.entity();
            let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let probe = bounds.clone();
            h_flex()
                .h(px(32.))
                .px(px(10.))
                .gap(px(8.))
                .text_size(px(11.))
                .text_color(pal.text)
                .child(label)
                .child(
                    h_flex()
                        .id(id)
                        .flex_1()
                        .h(px(24.))
                        .justify_between()
                        .border_b_1()
                        .border_color(rgb(0xcccccc))
                        .cursor_pointer()
                        .text_size(px(13.33))
                        .relative()
                        .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                        .child(value.clone())
                        .child(crate::ui::glyph("ui-caret-down", 7., rgb(0x666666)))
                        .on_click(move |_: &ClickEvent, window, cx| {
                            let (v, b) = (v.clone(), bounds.get());
                            let options = options.clone();
                            let value = value.clone();
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                                for o in options {
                                    let v = v.clone();
                                    m = m.item(crate::os_menu::OsMenuItem::new(o).checked(o == value).on_click(move |_, _, cx| {
                                        // A grade, or a Type: Precision applies at once, ISO 2768-1 its
                                        // default grade (live: changing a select applies it).
                                        let standard = match o {
                                            ann::ISO_2768_1 => Some(ann::DEFAULT_TOLERANCE_CLASS),
                                            o if o == ann::PRECISION_STANDARD || ann::DEFAULT_TOLERANCES.contains(&o) => Some(o),
                                            _ => None,
                                        };
                                        if let Some(sd) = standard {
                                            v.update(cx, |this, cx| {
                                                if this.applied_default_tolerances(cx).as_deref().is_some_and(|a| a == sd || (o == ann::ISO_2768_1 && a != ann::PRECISION_STANDARD)) {
                                                    return;
                                                }
                                                this.set_default_tolerances(Some(sd.into()), cx)
                                            });
                                        }
                                    }));
                                }
                                m.native_select(f32::from(b.size.width))
                            });
                            crate::os_menu::open_anchored(point(b.origin.x, b.bottom() + px(1.)), Anchor::TopLeft, menu, window, cx);
                        }),
                )
        };
        let close = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.insp.picker = false;
            cx.notify();
        });
        let mut body = v_flex().flex_1().child(
            div()
                .mx(px(6.))
                .mt(px(8.))
                .h(px(30.))
                .px(px(6.))
                .flex()
                .items_center()
                .border_1()
                .border_color(rgb(0xcccccc))
                .text_size(px(11.))
                .italic()
                .text_color(rgb(0x999999))
                .child("Search by name, folder, or configuration"),
        );
        if tab == 2 {
            body = body
                .child(div().h(px(8.)))
                .child(select("dtol-library", "Library", "Onshape default tolerances".into(), vec!["Onshape default tolerances"], cx))
                .when(!precision, |d| {
                    d.child(select("dtol-type", "Type", ann::ISO_2768_1.into(), ann::TOLERANCE_TYPES.to_vec(), cx))
                        .child(select("dtol-class", "Default tolerances", applied.clone(), ann::DEFAULT_TOLERANCES.to_vec(), cx))
                })
                // Live: with the Precision type, a stacked "Default
                // tolerances" label over a select of the two types.
                .when(precision, |d| {
                    d.child(div().px(px(10.)).pt(px(6.)).text_size(px(11.)).text_color(pal.text).child("Default tolerances"))
                        .child(select("dtol-type", "", ann::PRECISION_STANDARD.into(), ann::TOLERANCE_TYPES.to_vec(), cx))
                })
                .child(h_flex().mt(px(16.)).justify_center().child(crate::assets::icon("insp-default-tol", 140., cx)));
        }
        Some(
            v_flex()
                .id("dtol-picker")
                .occlude()
                .absolute()
                .top(px(4.))
                .right(px(8.))
                .w(px(300.))
                .h(px(560.))
                .bg(rgb(0xffffff))
                .shadow_md()
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(30.))
                        .pl(px(6.))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().text_size(px(13.33)).font_weight(FontWeight::SEMIBOLD).child("Select default tolerances"))
                        .child(div().id("dtol-close").w(px(28.)).h_full().flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 12., rgb(0x666666))).on_click(close)),
                )
                .child(h_flex().px(px(4.)).child(tab_button(0, "Current", "document", cx)).child(tab_button(1, "Other", "documents", cx)).child(tab_button(2, "Default", "tolerances", cx)))
                .child(body)
                .child(
                    h_flex().h(px(48.)).justify_center().child(crate::os_ui::secondary_button("dtol-select", "Select", cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ui.insp.picker = false;
                        cx.notify();
                    }))),
                )
                .into_any_element(),
        )
    }
}
