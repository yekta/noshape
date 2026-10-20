//! The assembly's bottom-right Analysis tools menu and Mass properties
//! dialog (help View__analysis_tools, View__mass_properties_tool,
//! View__interference_detection; live part-studio/80–82, analysis/03,
//! dom/dialog-mass-properties.txt, menu-analysis-tools.txt).
//!
//! Analysis tools in an assembly has three items (live assembly/734):
//! Interference detection…, Zebra stripes…, Reflection analysis…. Mass and
//! section properties is the 440 px dialog for the parts of the instances
//! to measure where they sit in the assembly (live 737–739).

use super::*;
use gpui_kit::component::input::InputEvent;
use noshape_model::assembly::analysis::{AssemblyMass, assembly_mass_properties};

/// Mass properties dialog state.
#[derive(Default)]
pub(crate) struct AsmMassUi {
    /// Face tab (else Part).
    pub faces: bool,
    /// The "material not defined" warning was dismissed.
    pub warning_closed: bool,
}

/// The analysis tools of an assembly, in menu order (live assembly/734).
const ANALYSIS_TOOLS: [&str; 3] = ["Interference detection…", "Zebra stripes…", "Reflection analysis…"];

impl AssemblyView {
    /// "Show analysis tools": the context-menu-style list (live analysis/03,
    /// 157 px, 22 px rows) anchored at the button.
    pub(super) fn open_analysis_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.list_no_icons();
            for (k, label) in ANALYSIS_TOOLS.iter().enumerate() {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(*label).on_click(move |_, window, cx| v.update(cx, |this, cx| this.run_analysis_tool(k, window, cx))));
            }
            m
        });
        crate::os_menu::open_anchored(at, Anchor::BottomRight, menu, window, cx);
    }

    fn run_analysis_tool(&mut self, k: usize, window: &mut Window, cx: &mut Context<Self>) {
        match k {
            0 => self.open_interference(cx),
            1 => {
                self.close_reflection(false, cx);
                self.open_zebra(window, cx);
            }
            _ => {
                self.close_zebra(false, cx);
                self.open_reflection(window, cx);
            }
        }
    }

    /// "Display mass and section properties" toggles the dialog.
    pub(super) fn toggle_mass(&mut self, cx: &mut Context<Self>) {
        self.ui.mass = if self.ui.mass.is_some() { None } else { Some(AsmMassUi::default()) };
        cx.notify();
    }

    /// Mass properties of the selected instances' parts (nothing when none is
    /// selected: live 737 leaves every value blank), with each part's
    /// material density from its Part Studio.
    fn assembly_mass(&self, cx: &App) -> Option<AssemblyMass> {
        if self.selected.is_empty() {
            return None;
        }
        let doc = &self.doc.read(cx).session.doc;
        let density = |o: &noshape_model::assembly::eval::PartOccurrence| match doc.tab(o.studio).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => ps.part_properties.get(&o.body).and_then(|p| p.density),
            _ => None,
        };
        assembly_mass_properties(&self.result, &self.selected, &density)
    }

    /// The 440 px "Mass and section properties" dialog in the feature-dialog
    /// place, plus Onshape's warning bubble when a part has no material.
    pub(super) fn render_mass_dialog(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(ui) = &self.ui.mass else { return vec![] };
        let (faces, warning_closed) = (ui.faces, ui.warning_closed);
        let pal = p();
        let units = self.units(cx);
        let mass = if faces { None } else { self.assembly_mass(cx) };
        // The top-level row selects every instance: the field then holds the
        // assembly itself (live 739), whose Mass can be overridden.
        let top_level = self.assembly(cx).is_some_and(|a| !self.selected.is_empty() && a.instances.iter().filter(|i| a.generated_by(i.id).is_none()).all(|i| self.selected.contains(&i.id)));
        let names: Vec<String> = if faces {
            Vec::new()
        } else if top_level {
            self.doc.read(cx).session.doc.tab(self.tab).map(|t| vec![t.name.clone()]).unwrap_or_default()
        } else {
            self.selected.iter().filter_map(|id| self.assembly(cx).and_then(|a| a.instance(*id)).map(|i| i.name.clone())).collect()
        };
        let (mass_sym, mass_factor) = mass_unit(&units);
        let len_sym = units.length.symbol();
        let in_units = |x: f64, dim: noshape_units::Dim| number(x / units.si_factor(dim));
        let missing = mass.as_ref().is_some_and(|m| !m.without_material.is_empty());
        let tab = |id: &'static str, label: &'static str, on_faces: bool, cx: &mut Context<Self>| {
            let on = faces == on_faces;
            div()
                .id(id)
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(pal.border))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(m) = &mut this.ui.mass {
                        m.faces = on_faces;
                    }
                    cx.notify();
                }))
        };
        // Onshape's selection box; parts without a material listed in red.
        let field = |label: &str, items: Vec<String>, active: bool, invalid: bool| {
            let empty = items.is_empty();
            v_flex()
                .mx(px(3.))
                .my(px(2.))
                .rounded(px(2.))
                .border_1()
                .border_color(if active { pal.accent } else { pal.border })
                .when(active, |d| d.bg(rgb(0xdef1fb)))
                .child(div().px(px(6.)).when(empty, |d| d.py(px(5.)).text_size(px(12.))).when(!empty, |d| d.pt(px(2.)).text_size(px(10.))).text_color(rgb(0x555555)).child(label.to_string()))
                .children(items.into_iter().map(|t| h_flex().h(px(24.)).px(px(6.)).text_size(px(13.33)).text_color(if invalid { pal.text_invalid } else { rgb(0x555555) }).child(div().flex_1().truncate().child(t)).child(div().px(px(5.)).text_size(px(16.67)).text_color(rgb(0x666666)).child("×"))))
        };
        let value = |v: String| div().flex_1().mr(px(5.)).h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(v);
        let value_row = |label: &'static str, v: String| h_flex().h(px(28.)).child(div().px(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child(label)).child(value(v));
        // Override: a greyed box (live 737–738), white for the assembly's Mass (739).
        let override_check = |id: &'static str, enabled: bool| {
            h_flex()
                .id(id)
                .h(px(27.))
                .pl(px(4.))
                .gap(px(4.))
                .text_size(px(13.33))
                .text_color(pal.text_muted)
                .child(div().size(px(12.)).rounded(px(3.)).when(enabled, |d| d.bg(rgb(0xffffff)).border_1().border_color(rgb(0x888888))).when(!enabled, |d| d.bg(rgb(0x999999))))
                .child("Override")
        };
        let axis = |l: &'static str, c: u32, glyph: &'static str| h_flex().w(px(30.)).pl(px(5.)).gap(px(1.)).text_size(px(12.)).text_color(rgb(c)).child(l).child(glyph);
        let mut body = v_flex().w_full().child(h_flex().w_full().mb(px(2.)).child(tab("amass-tab-part", "Part", false, cx)).child(tab("amass-tab-face", "Face", true, cx)));
        body = body
            .child(field(if faces { "Faces to measure" } else { "Instances to measure" }, names, true, missing))
            .child(field("Mate connector for reference frame", Vec::new(), false, false))
            .child(crate::ui::os_checkbox("amass-variance", "Show calculation variance", false));
        // Every row is shown; values only once something is measured.
        let props = mass.as_ref().map(|m| &m.props);
        // Without a material Onshape leaves mass, center of mass and inertia empty.
        let have = props.is_some_and(|p| p.mass.is_some());
        let length = noshape_units::Dim::LENGTH;
        let with_unit = |v: Option<String>, unit: String| v.map(|v| format!("{v} {unit}")).unwrap_or_default();
        let c = props.map(|p| p.center_of_mass).unwrap_or_default();
        let com = |v: f64| with_unit(have.then(|| in_units(v, length)), len_sym.to_string());
        body = body
            .child(h_flex().h(px(28.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Mass")).child(override_check("amass-override", top_level)).child(value(with_unit(props.and_then(|p| p.mass).map(|m| number(m * mass_factor)), mass_sym.to_string()))))
            .child(value_row("Volume", with_unit(props.map(|p| in_units(p.volume, noshape_units::Dim { length: 3, angle: 0 })), format!("{len_sym}³"))))
            .child(value_row("Surface area", with_unit(props.map(|p| in_units(p.surface_area, noshape_units::Dim { length: 2, angle: 0 })), format!("{len_sym}²"))))
            .child(h_flex().h(px(28.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Center of mass")).child(override_check("amass-com-override", false)))
            .children([("X", 0xcd0000u32, "↘", c.x), ("Y", 0x009400, "↗", c.y), ("Z", 0x0000d0, "↑", c.z)].into_iter().map(|(l, col, g, v)| h_flex().h(px(26.)).child(axis(l, col, g)).child(value(com(v)))))
            .child(h_flex().h(px(28.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child(format!("Mass moments of inertia ({mass_sym} {len_sym}²)"))).child(override_check("amass-moi-override", false)));
        let f = units.si_factor(length).powi(2);
        let i = props.map(|p| p.inertia).unwrap_or(glam::DMat3::ZERO);
        let iv = |x: f64| if have { number(x * mass_factor / f) } else { String::new() };
        let grid = [[i.x_axis.x, i.y_axis.x, i.z_axis.x], [i.x_axis.y, i.y_axis.y, i.z_axis.y], [i.x_axis.z, i.y_axis.z, i.z_axis.z]];
        let labels = [["Lxx", "Lxy", "Lxz"], ["Lyx", "Lyy", "Lyz"], ["Lzx", "Lzy", "Lzz"]];
        for (r, row_names) in labels.iter().enumerate() {
            let mut row = h_flex().h(px(26.)).px(px(5.)).gap(px(8.));
            for (k, n) in row_names.iter().enumerate() {
                row = row.child(h_flex().flex_1().gap(px(4.)).child(div().w(px(24.)).text_size(px(12.)).text_color(pal.text_muted).child(*n)).child(div().flex_1().h(px(22.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(iv(grid[r][k]))));
            }
            body = body.child(row);
        }
        let help = super::props::help_dot();
        let mut out = vec![
            v_flex()
                .id("amass-dialog")
                .occlude()
                .w(px(440.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .pl(px(6.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Mass and section properties"))
                        // Live: the ✓ is shown disabled (op .2).
                        .child(div().size(px(28.)).flex().items_center().justify_center().bg(pal.accept).opacity(0.2).child(Icon::new(IconName::Check).with_size(px(18.)).text_color(rgb(0xffffff))))
                        .child(
                            div()
                                .id("amass-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .tooltip(crate::ui::tooltip("Cancel (Esc)"))
                                .child(crate::assets::icon("dialog-cancel", 18., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.mass = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(body.pb(px(4.)))
                .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(help))
                .into_any_element(),
        ];
        if missing && !warning_closed {
            // Live (part-studio/82): a yellow warning bubble under the toolbar.
            out.push(
                div()
                    .absolute()
                    .top(px(9.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(crate::os_ui::message_bubble(
                        "amass-warning",
                        crate::ui::AlertKind::Warning,
                        div().child("One or more parts do not have a material defined."),
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(m) = &mut this.ui.mass {
                                m.warning_closed = true;
                            }
                            cx.notify();
                        }),
                        cx,
                    ))
                    .into_any_element(),
            );
        }
        out
    }
}

/// Zebra stripes settings: stripe count, flipped, edges shown (live
/// analysis/50: 35, off, on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ZebraSettings {
    pub stripes: u32,
    pub flip: bool,
    pub show_edges: bool,
}

impl Default for ZebraSettings {
    fn default() -> Self {
        Self { stripes: 35, flip: false, show_edges: true }
    }
}

/// The open Zebra stripes dialog.
pub(crate) struct ZebraDialog {
    pub settings: ZebraSettings,
    stripes: Entity<InputState>,
    _sub: Subscription,
}

impl AssemblyView {
    /// Analysis tools › Zebra stripes… (as in the Part Studio: ✓ keeps the
    /// stripes on, ✕ / Esc turns them off).
    fn open_zebra(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.ui.zebra.unwrap_or_default();
        let stripes = cx.new(|cx| InputState::new(window, cx).default_value(settings.stripes.to_string()));
        let sub = cx.subscribe_in(&stripes, window, |this: &mut Self, st, ev: &InputEvent, _, cx| match ev {
            InputEvent::Change => {
                if let Ok(n) = st.read(cx).value().trim().parse::<u32>()
                    && let Some(d) = &mut this.ui.zebra_dialog
                    && (1..=500).contains(&n)
                {
                    d.settings.stripes = n;
                    this.rebuild_scene(false, cx);
                }
            }
            InputEvent::PressEnter { .. } => this.close_zebra(true, cx),
            _ => {}
        });
        self.ui.zebra_dialog = Some(ZebraDialog { settings, stripes, _sub: sub });
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    pub(super) fn close_zebra(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.ui.zebra_dialog.take() else { return };
        self.ui.zebra = accept.then_some(d.settings);
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// Zebra stripes being shown (the open dialog's, else the accepted ones).
    pub(super) fn active_zebra(&self) -> Option<ZebraSettings> {
        self.ui.zebra_dialog.as_ref().map(|d| d.settings).or(self.ui.zebra)
    }

    /// The 220 px Zebra stripes dialog (live dom/dialog-zebra-stripes.txt).
    pub(super) fn render_zebra_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.zebra_dialog.as_ref()?;
        let pal = p();
        let (flip, edges) = (d.settings.flip, d.settings.show_edges);
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut ZebraSettings), cx: &mut Context<Self>| {
            crate::os_ui::checkbox(id, label, on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.ui.zebra_dialog {
                    f(&mut d.settings);
                }
                this.rebuild_scene(false, cx);
                cx.notify();
            }))
        };
        Some(
            v_flex()
                .id("azebra-dialog")
                .occlude()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .pl(px(6.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Zebra stripes"))
                        .child(
                            div()
                                .id("azebra-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip("Accept (Enter)"))
                                .child(Icon::new(IconName::Check).with_size(px(18.)).text_color(rgb(0xffffff)))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_zebra(true, cx))),
                        )
                        .child(
                            div()
                                .id("azebra-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .tooltip(crate::ui::tooltip("Cancel (Esc)"))
                                .child(crate::assets::icon("dialog-cancel", 18., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_zebra(false, cx))),
                        ),
                )
                .child(crate::os_ui::param_row("Stripe count", Input::new(&d.stripes).appearance(false).text_size(px(13.33)).text_right().h(px(25.))))
                .child(check("azebra-flip", "Flip stripes", flip, |s| s.flip = !s.flip, cx))
                .child(check("azebra-edges", "Show edges", edges, |s| s.show_edges = !s.show_edges, cx))
                .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(super::props::help_dot()))
                .into_any_element(),
        )
    }
}

/// The user's mass unit (Workspace units › Mass: Kilogram, Gram, Pound,
/// Ounce): (symbol, per kilogram).
fn mass_unit(units: &noshape_units::UnitPrefs) -> (&'static str, f64) {
    match units.other[2].0 {
        1 => ("g", 1000.0),
        2 => ("lb", 2.204_622_621_848_776),
        3 => ("oz", 35.273_961_949_580_41),
        _ => ("kg", 1.0),
    }
}

/// A Mass and section properties value as Onshape writes it (live 738–739):
/// three decimals without trailing zeros ("5.77", "11000"), and values that
/// would read 0.000 in exponent form ("-3.307e-4").
fn number(x: f64) -> String {
    let fixed = format!("{x:.3}");
    if x != 0.0 && fixed.trim_start_matches('-') == "0.000" {
        return format!("{x:.3e}");
    }
    let t = fixed.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_string() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mass_values_are_written_like_onshape() {
        use super::number;
        assert_eq!(number(5.7700), "5.77");
        assert_eq!(number(11000.0), "11000");
        assert_eq!(number(0.138), "0.138");
        assert_eq!(number(-0.00033071), "-3.307e-4");
        assert_eq!(number(-0.00095), "-0.001");
        assert_eq!(number(0.0), "0");
    }
}
