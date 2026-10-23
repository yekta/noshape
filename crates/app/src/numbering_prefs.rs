//! Company settings › Numbering schemes: "Numbering scheme settings".
//! Help Plans/numbering_schemes and its image accounts/co-numschemes-03.png:
//! tabs Release objects | Standard content | Items | Release names; a
//! "Number generation" select (Manual entry / Sequential part number
//! generation); for sequential numbers a flow chart of Prefix per object
//! type → Min length → Next → Preview; Release objects add Part number
//! uniqueness (radios) and Part number propagation (select); "Save changes"
//! saves the current tab only.

use crate::release_dialog::{numbering_schemes, set_numbering_schemes};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::release::{NumberScheme, NumberingSchemes, Propagation};

const TABS: [&str; 4] = ["Release objects", "Standard content", "Items", "Release names"];
/// Object types of the Release objects tab, in the flow chart's order.
const RELEASE_TYPES: [&str; 5] = ["Parts", "Assemblies", "Drawings", "Variable Studios", "Files"];
const GENERATION: [&str; 2] = ["Manual entry", "Sequential part number generation"];

/// The inputs of one tab.
struct TabInputs {
    prefixes: Vec<Entity<InputState>>,
    min_length: Entity<InputState>,
    next: Entity<InputState>,
}

pub struct NumberingPrefsView {
    saved: NumberingSchemes,
    edited: NumberingSchemes,
    tab: usize,
    /// Open select: 0 = Number generation, 1 = Part number propagation.
    select_open: Option<usize>,
    inputs: Vec<TabInputs>,
    _subs: Vec<Subscription>,
}

fn scheme(n: &NumberingSchemes, tab: usize) -> &NumberScheme {
    match tab {
        0 => &n.release_objects,
        1 => &n.standard_content,
        2 => &n.items,
        _ => &n.release_names,
    }
}

fn scheme_mut(n: &mut NumberingSchemes, tab: usize) -> &mut NumberScheme {
    match tab {
        0 => &mut n.release_objects,
        1 => &mut n.standard_content,
        2 => &mut n.items,
        _ => &mut n.release_names,
    }
}

impl NumberingPrefsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = numbering_schemes();
        let mut subs = Vec::new();
        let mut field = |text: String, window: &mut Window, cx: &mut Context<Self>| {
            let st = cx.new(|cx| InputState::new(window, cx).default_value(text));
            subs.push(cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.sync(cx);
                }
            }));
            st
        };
        let inputs = (0..TABS.len())
            .map(|t| {
                let s = scheme(&saved, t).clone();
                TabInputs { prefixes: s.prefixes.iter().map(|p| field(p.clone(), window, cx)).collect(), min_length: field(s.min_length.to_string(), window, cx), next: field(s.next.to_string(), window, cx) }
            })
            .collect();
        Self { edited: saved.clone(), saved, tab: 0, select_open: None, inputs, _subs: subs }
    }

    /// Reads the current tab's inputs into `edited` (invalid numbers keep
    /// the last valid value).
    fn sync(&mut self, cx: &mut Context<Self>) {
        let tab = self.tab;
        let inp = &self.inputs[tab];
        let prefixes: Vec<String> = inp.prefixes.iter().map(|p| p.read(cx).value().to_string()).collect();
        let min = inp.min_length.read(cx).value().trim().parse::<usize>().ok();
        let next = inp.next.read(cx).value().trim().parse::<u64>().ok();
        let s = scheme_mut(&mut self.edited, tab);
        s.prefixes = prefixes;
        if let Some(m) = min {
            s.min_length = m.min(20);
        }
        if let Some(n) = next {
            s.next = n;
        }
        cx.notify();
    }

    /// The current tab differs from what is saved.
    fn tab_changed(&self) -> bool {
        scheme(&self.edited, self.tab) != scheme(&self.saved, self.tab) || (self.tab == 0 && (self.edited.drawing_reuse != self.saved.drawing_reuse || self.edited.propagation != self.saved.propagation))
    }

    /// "Save changes only saves the changes for the current tab being
    /// edited"; numbers generated meanwhile keep their counters.
    fn save(&mut self, cx: &mut Context<Self>) {
        let mut stored = numbering_schemes();
        let tab = self.tab;
        let mut edited = scheme(&self.edited, tab).clone();
        // "Never decreasing": numbers generated since keep their counter.
        edited.next = edited.next.max(scheme(&stored, tab).next);
        *scheme_mut(&mut stored, tab) = edited.clone();
        if tab == 0 {
            stored.drawing_reuse = self.edited.drawing_reuse;
            stored.propagation = self.edited.propagation;
        }
        set_numbering_schemes(&stored);
        *scheme_mut(&mut self.saved, tab) = edited;
        if tab == 0 {
            self.saved.drawing_reuse = self.edited.drawing_reuse;
            self.saved.propagation = self.edited.propagation;
        }
        cx.notify();
    }
}

/// A Bootstrap-style select (34 px, caret at the right) with its open list.
fn select(id: &'static str, value: &str, open: bool, width: f32, options: Vec<Stateful<Div>>, toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Div {
    let pal = p();
    v_flex()
        .w(px(width))
        .relative()
        .child(
            h_flex()
                .id(id)
                .h(px(34.))
                .px(px(12.))
                .border_1()
                .rounded(px(4.))
                .border_color(rgb(0xcccccc))
                .cursor_pointer()
                .child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(value.to_string()))
                .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                .on_click(toggle),
        )
        .when(open, |d| d.child(v_flex().border_1().border_color(rgb(0x767676)).bg(rgb(0xffffff)).children(options)))
}

fn option(id: SharedString, text: &'static str) -> Stateful<Div> {
    div().id(id).px(px(10.)).py(px(4.)).text_size(px(14.)).cursor_pointer().hover(|d| d.bg(rgb(0x1e90ff)).text_color(gpui_kit::rgb_static(0xffffff))).child(text)
}

/// A 34 px bordered box of the flow chart (inputs and previews).
fn chart_box(w: f32) -> Div {
    div().w(px(w)).h(px(34.)).flex().items_center().px(px(8.)).border_1().rounded(px(2.)).border_color(rgb(0xcccccc)).bg(rgb(0xffffff)).text_size(px(14.))
}

impl Render for NumberingPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let tab = self.tab;
        let s = scheme(&self.edited, tab).clone();
        let line = rgb(0x999999);
        let bold = |t: &'static str| div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(t);
        // Tabs: Bootstrap nav-tabs (the active one bordered, open at the bottom).
        let tabs = h_flex().items_end().border_b_1().border_color(rgb(0xdee2e6)).children(TABS.iter().enumerate().map(|(i, t)| {
            let active = i == tab;
            div()
                .id(SharedString::from(format!("numbering-tab-{i}")))
                .px(px(10.))
                .py(px(7.))
                .mb(px(-1.))
                .text_size(px(14.))
                .text_color(pal.text)
                .border_1()
                .rounded_t(px(4.))
                .when(active, |d| d.border_color(rgb(0xdee2e6)).bg(rgb(0xffffff)))
                .when(!active, |d| d.border_color(rgba(0)).cursor_pointer().hover(|d| d.border_color(rgb(0xe9ecef))))
                .child(*t)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.tab = i;
                    this.select_open = None;
                    cx.notify();
                }))
        }));
        let gen_options = GENERATION
            .iter()
            .enumerate()
            .map(|(i, g)| {
                option(SharedString::from(format!("numbering-gen-{i}")), g).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    scheme_mut(&mut this.edited, this.tab).sequential = i == 1;
                    this.select_open = None;
                    cx.notify();
                }))
            })
            .collect();
        let gen_select = select(
            "numbering-gen",
            GENERATION[s.sequential as usize],
            self.select_open == Some(0),
            600.,
            gen_options,
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.select_open = if this.select_open == Some(0) { None } else { Some(0) };
                cx.notify();
            }),
        );
        // The flow chart (help image): Prefix boxes per type joined by a
        // bracket to Min length, a line to Next, a bracket to the previews.
        let types: Vec<&'static str> = if tab == 0 { RELEASE_TYPES.to_vec() } else { vec![""] };
        let rows = types.len();
        let pitch = 49.0_f32;
        let top = 28.0_f32;
        let center = |r: usize| top + 17.0 + pitch * r as f32;
        let mid = (center(0) + center(rows - 1)) / 2.0;
        let (x_prefix, x_lbracket, x_min, x_next, x_rbracket, x_preview) = (116.0_f32, 220.0_f32, 252.0_f32, 357.0_f32, 432.0_f32, 461.0_f32);
        let height = center(rows - 1) + 17.0 + 4.0;
        let hline = |x0: f32, x1: f32, y: f32| div().absolute().left(px(x0)).top(px(y)).w(px(x1 - x0)).h(px(1.)).bg(line);
        let vline = |x: f32, y0: f32, y1: f32| div().absolute().left(px(x)).top(px(y0)).w(px(1.)).h(px((y1 - y0).max(1.))).bg(line);
        let inp = &self.inputs[tab];
        let mut chart = div()
            .relative()
            .w(px(600.))
            .h(px(height))
            .child(div().absolute().left(px(x_prefix)).w(px(75.)).top_0().flex().justify_center().child(bold("Prefix")))
            .child(div().absolute().left(px(x_preview)).w(px(130.)).top_0().flex().justify_center().child(bold("Preview")))
            .child(div().absolute().left(px(x_min - 10.)).w(px(66.)).top(px(mid - 17.0 - 26.0)).flex().justify_center().child(bold("Min length")))
            .child(div().absolute().left(px(x_next)).w(px(46.)).top(px(mid - 17.0 - 26.0)).flex().justify_center().child(bold("Next")))
            .child(div().absolute().left(px(x_min)).top(px(mid - 17.0)).child(gpui_kit::base::input::input_frame(chart_box(46.).justify_center().child(Input::new(&inp.min_length).appearance(false).small()))))
            .child(div().absolute().left(px(x_next)).top(px(mid - 17.0)).child(gpui_kit::base::input::input_frame(chart_box(46.).justify_center().child(Input::new(&inp.next).appearance(false).small()))))
            .child(hline(x_lbracket, x_min, mid))
            .child(hline(x_min + 46.0, x_next, mid))
            .child(hline(x_next + 46.0, x_rbracket, mid))
            .child(vline(x_lbracket, center(0), center(rows - 1)))
            .child(vline(x_rbracket, center(0), center(rows - 1)));
        for (r, label) in types.iter().enumerate() {
            let y = center(r);
            chart = chart
                .child(div().absolute().left(px(10.)).top(px(y - 10.)).child(bold(label)))
                .when_some(inp.prefixes.get(r), |d, st| d.child(div().absolute().left(px(x_prefix)).top(px(y - 17.)).child(gpui_kit::base::input::input_frame(chart_box(75.).child(Input::new(st).appearance(false).small())))))
                .child(hline(x_prefix + 75.0, x_lbracket, y))
                .child(hline(x_rbracket, x_preview, y))
                .child(div().absolute().left(px(x_preview)).top(px(y - 17.)).child(chart_box(130.).justify_end().text_color(pal.text_muted).child(s.preview(r))));
        }
        let radio = |id: &'static str, text: &'static str, on: bool| {
            h_flex()
                .id(id)
                .gap(px(6.))
                .h(px(26.))
                .cursor_pointer()
                .child(div().size(px(13.)).rounded_full().border_1().border_color(if on { pal.accent } else { rgb(0x767676) }).flex().items_center().justify_center().when(on, |d| d.child(div().size(px(7.)).rounded_full().bg(pal.accent))))
                .child(div().text_size(px(14.)).text_color(pal.text).child(text))
        };
        let prop = self.edited.propagation;
        let prop_options = Propagation::ALL
            .iter()
            .map(|p| {
                let p = *p;
                option(SharedString::from(format!("numbering-prop-{}", p.label())), p.label()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.edited.propagation = p;
                    this.select_open = None;
                    cx.notify();
                }))
            })
            .collect();
        // Help Plans/numbering_schemes (Part number propagation).
        let prop_note = match prop {
            Propagation::Never => "Part numbers will only be applied in the version being released.",
            Propagation::OneWorkspace => "Part numbers will be applied in the version being released and in its workspace, when the version has only one.",
            Propagation::AllWorkspaces => "Part numbers will be applied in the version being released and in all of its workspaces.",
        };
        let changed = self.tab_changed();
        let reuse = self.edited.drawing_reuse;
        // The Preferences page's section look: 18 px / 500 heading over a 1 px
        // `#333` rule, the content 15 px in.
        v_flex()
            .w_full()
            .pl(px(15.))
            .child(h_flex().ml(px(-15.)).h(px(32.)).pl(px(15.)).gap(px(6.)).border_b_1().border_color(pal.text).child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Numbering scheme settings")).child(crate::assets::icon("help-gray", 20., cx)))
            .child(div().pt(px(10.)).child(tabs))
            .child(
                v_flex()
                    .px(px(34.))
                    .pt(px(20.))
                    .pb(px(14.))
                    .gap(px(6.))
                    .child(bold("Number generation"))
                    .child(gen_select)
                    .when(s.sequential, |d| d.child(div().pt(px(14.)).child(chart)))
                    .when(tab == 0, |d| {
                        d.child(div().pt(px(14.)).child(bold("Part number uniqueness")))
                            .child(radio("numbering-unique", "All part numbers in a release must be unique", !reuse).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.edited.drawing_reuse = false;
                                cx.notify();
                            })))
                            .child(radio("numbering-reuse", "Drawing can reuse part number from an assembly or part in the release", reuse).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.edited.drawing_reuse = true;
                                cx.notify();
                            })))
                            .child(h_flex().pt(px(8.)).gap(px(6.)).child(bold("Part number propagation")).child(crate::assets::icon("help", 18., cx)))
                            .child(select(
                                "numbering-prop",
                                prop.label(),
                                self.select_open == Some(1),
                                600.,
                                prop_options,
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.select_open = if this.select_open == Some(1) { None } else { Some(1) };
                                    cx.notify();
                                }),
                            ))
                            .child(div().text_size(px(14.)).text_color(pal.text).child(prop_note))
                    })
                    .child(
                        h_flex().pt(px(12.)).child(
                            div()
                                .id("numbering-save")
                                .px(px(12.))
                                .py(px(7.))
                                .rounded(px(4.))
                                .text_size(px(14.))
                                .text_color(gpui_kit::rgb_static(0xffffff))
                                .bg(gpui_kit::rgb_static(0x1651b0))
                                .when(!changed, |d| d.opacity(0.4))
                                .when(changed, |d| d.cursor_pointer())
                                .child("Save changes")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if this.tab_changed() {
                                        this.save(cx);
                                    }
                                })),
                        ),
                    ),
            )
    }
}
