//! Parts list › Assign material…: Onshape's floating "Material" panel (live
//! dialogs/assign-material-01…08, dom/dialog-assign-material*.txt): 280 px at
//! the bottom left of the graphics area, ✓ (enabled once a material is
//! picked) and ✕; tabs Library | Custom. Library: the library select
//! ("Onshape Material Library" and the document's Material Library tabs)
//! with "+" (Add material library…), a searchable picker ("- None -",
//! two-line rows "Name" / "Density: 2.836e-1 lb/in³") and the picked
//! material's read-only properties. Custom: the same fields, editable.

use super::PartStudioView;
use super::material_data::ONSHAPE_MATERIALS;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::reference::BodyId;

/// lb/in³ → kg/m³.
const LB_IN3: f64 = 27_679.904_710_191;

/// Property rows under the picker, in Onshape's order.
const FIELDS: [&str; 8] = ["Name", "Density (lb/in³)", "Poisson's ratio", "Young's modulus (Pa)", "Tensile yield strength (Pa)", "Ultimate tensile strength (Pa)", "Compressive yield strength (Pa)", "Ultimate compressive strength (Pa)"];

/// The captured properties of library materials (live: Aluminum - 6061).
fn known_properties(name: &str) -> Option<[&'static str; 6]> {
    match name {
        "Aluminum - 6061" => Some(["0.35", "68900000000", "241000000", "290000000", "0", "0"]),
        _ => None,
    }
}

/// A material of a library: name, category, density (kg/m³), the other
/// property values as shown.
#[derive(Clone, Debug)]
struct Entry {
    name: String,
    category: String,
    density: f64,
    density_text: String,
    extra: [String; 6],
}

pub struct MaterialPanel {
    pub bodies: Vec<BodyId>,
    custom: bool,
    /// 0 = Onshape Material Library, then the document's library tabs.
    library: usize,
    picked: Option<Entry>,
    list_open: bool,
    /// The library list is open.
    lib_open: bool,
    search: Entity<InputState>,
    /// Custom tab: one input per field.
    custom_fields: Vec<Entity<InputState>>,
    _subs: Vec<Subscription>,
}

/// A library of the select: the Onshape one, one added to the account, or
/// a Material Library tab of this document.
#[derive(Clone, Debug)]
enum Source {
    Onshape,
    Added(crate::material_libraries::AddedLibrary),
    Doc(usize),
}

impl PartStudioView {
    /// The libraries of the select, in order, with their names.
    fn material_sources(&self, cx: &App) -> Vec<(Source, String)> {
        let mut out = vec![(Source::Onshape, "Onshape Material Library".to_string())];
        out.extend(crate::material_libraries::added().into_iter().map(|l| {
            let n = l.name.clone();
            (Source::Added(l), n)
        }));
        out.extend(noshape_model::material_library::libraries(&self.doc.read(cx).session.doc).into_iter().enumerate().map(|(i, (_, n, _))| (Source::Doc(i), n)));
        out
    }

    /// The library entries of the selected library.
    fn material_entries(&self, library: usize, cx: &App) -> Vec<Entry> {
        let to_entries = |ms: &[noshape_model::material_library::LibraryMaterial]| -> Vec<Entry> {
            ms.iter().map(|m| Entry { name: m.name.clone(), category: m.category.clone(), density: m.density, density_text: format!("{:.3e} lb/in³", m.density / LB_IN3), extra: Default::default() }).collect()
        };
        let source = self.material_sources(cx).get(library).map(|(s, _)| s.clone()).unwrap_or(Source::Onshape);
        if let Source::Added(l) = &source {
            return to_entries(&crate::material_libraries::materials_of(l));
        }
        if let Source::Doc(i) = source {
            let doc = self.doc.read(cx);
            return noshape_model::material_library::libraries(&doc.session.doc).get(i).map(|(_, _, lib)| to_entries(&lib.materials)).unwrap_or_default();
        }
        {
            return ONSHAPE_MATERIALS
                .iter()
                .map(|(n, d)| Entry {
                    name: n.to_string(),
                    category: String::new(),
                    density: d.parse::<f64>().unwrap_or(0.0) * LB_IN3,
                    density_text: format!("{d} lb/in³"),
                    extra: known_properties(n).map(|v| v.map(str::to_string)).unwrap_or_default(),
                })
                .collect();
        }
    }

    pub(super) fn open_material_panel(&mut self, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search by material or category"));
        let mut subs = vec![cx.subscribe(&search, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })];
        // Live (dialog-assign-material-custom): Name empty, Density "1".
        let custom_fields: Vec<Entity<InputState>> = FIELDS
            .iter()
            .enumerate()
            .map(|(i, _)| cx.new(|cx| InputState::new(window, cx).default_value(if i == 1 { "1" } else { "" })))
            .collect();
        for f in &custom_fields {
            subs.push(cx.subscribe(f, |_, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }
        self.ui.material_panel = Some(MaterialPanel { bodies, custom: false, library: 0, picked: None, list_open: false, lib_open: false, search, custom_fields, _subs: subs });
        cx.notify();
    }

    /// ✓: assign the picked (or custom) material to the parts.
    fn apply_material(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.ui.material_panel.take() else { return };
        let (name, density) = if m.custom {
            let name = m.custom_fields[0].read(cx).value().trim().to_string();
            let d = m.custom_fields[1].read(cx).value().trim().parse::<f64>().unwrap_or(0.0) * LB_IN3;
            (name, d)
        } else {
            match &m.picked {
                Some(e) => (e.name.clone(), e.density),
                None => return,
            }
        };
        if name.is_empty() {
            return;
        }
        let bodies = m.bodies.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Assign material", cx, |ps| {
                for b in &bodies {
                    let p = ps.part_properties.entry(b.clone()).or_default();
                    p.material = Some(name.clone());
                    p.density = Some(density);
                }
            });
        });
        cx.notify();
    }

    pub(super) fn render_material_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let libraries: Vec<String> = self.material_sources(cx).into_iter().map(|(_, n)| n).collect();
        let m = self.ui.material_panel.as_ref()?;
        let (custom, library, list_open, lib_open) = (m.custom, m.library, m.list_open, m.lib_open);
        let picked = m.picked.clone();
        let query = m.search.read(cx).value().trim().to_lowercase();
        let search = m.search.clone();
        let custom_fields = m.custom_fields.clone();
        let entries = self.material_entries(library, cx);
        let can_apply = if custom { !custom_fields[0].read(cx).value().trim().is_empty() } else { picked.is_some() };
        let label = |t: &str| div().flex_none().whitespace_nowrap().pl(px(5.)).pt(px(7.)).text_size(px(12.)).line_height(px(16.)).text_color(gpui_kit::rgb(0x666666)).child(t.to_string());
        let ro_value = |v: String| div().flex_1().h(px(26.)).pt(px(5.)).px(px(3.)).text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(v);
        let tab = |id: &'static str, on: bool, to_custom: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                .when(!on, |d| d.text_color(pal.text))
                .child(if to_custom { "Custom" } else { "Library" })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(m) = &mut this.ui.material_panel {
                        m.custom = to_custom;
                    }
                    cx.notify();
                }))
        };
        let mut body = v_flex();
        if custom {
            for (i, f) in FIELDS.iter().enumerate() {
                body = body.child(h_flex().pr(px(5.)).child(label(f)).child(div().flex_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Input::new(&custom_fields[i]).appearance(false))));
            }
        } else {
            // Library select and "+" (Add material library…).
            let lib_items = libraries.iter().enumerate().map(|(i, n)| {
                div()
                    .id(SharedString::from(format!("matlib-{i}")))
                    .px(px(6.))
                    .py(px(2.))
                    .text_size(px(13.33))
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0x1e90ff)).text_color(gpui_kit::rgb(0xffffff)))
                    .child(n.clone())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(m) = &mut this.ui.material_panel {
                            m.library = i;
                            m.picked = None;
                            m.lib_open = false;
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            });
            let lib_items: Vec<AnyElement> = lib_items.collect();
            body = body.child(
                h_flex()
                    .pl(px(3.))
                    .pr(px(4.))
                    .py(px(3.))
                    .gap(px(1.))
                    .child(
                        div()
                            .id("matlib-select")
                            .flex_1()
                            .h(px(32.))
                            .px(px(5.))
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(gpui_kit::rgb(0xd7d7d7))
                            .text_size(px(13.33))
                            .child(div().flex_1().truncate().child(libraries.get(library).cloned().unwrap_or_default()))
                            .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(m) = &mut this.ui.material_panel {
                                    m.lib_open = !m.lib_open;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("matlib-add")
                            .w(px(36.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.5))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xaaaaaa))
                            .cursor_pointer()
                            .child(crate::assets::icon("plus", 20., cx))
                            .tooltip(crate::ui::tooltip("Add material library…"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_add_material_library(window, cx))),
                    ),
            );
            if lib_open {
                body = body.child(v_flex().mx(px(3.)).border_1().border_color(gpui_kit::rgb(0x767676)).bg(gpui_kit::rgb(0xffffff)).children(lib_items));
            }
            // The picker: "- None -" or the material, a caret. Open, it becomes
            // the search field (live assign-material-03: 34 px, blue border).
            body = body.child(div().pl(px(2.)).pr(px(4.)).pt(px(2.)).pb(px(1.)).child(if list_open {
                div().h(px(34.)).pl(px(8.)).pr(px(3.)).flex().items_center().border_1().border_color(pal.accent).text_size(px(14.)).child(Input::new(&search).appearance(false)).into_any_element()
            } else {
                    h_flex()
                        .id("mat-picker")
                        .h(px(34.))
                        .pl(px(11.))
                        .pr(px(10.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .cursor_pointer()
                        .child(div().flex_1().truncate().text_size(px(13.33)).text_color(gpui_kit::rgb(0x333333)).child(picked.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| "- None -".into())))
                        .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            if let Some(m) = &mut this.ui.material_panel {
                                m.list_open = !m.list_open;
                                if m.list_open {
                                    m.search.update(cx, |s, cx| s.focus(window, cx));
                                }
                            }
                            cx.notify();
                        }))
                        .into_any_element()
            }));
            if list_open {
                let rows: Vec<AnyElement> = std::iter::once(None)
                    .chain(entries.iter().filter(|e| query.is_empty() || e.name.to_lowercase().contains(&query) || e.category.to_lowercase().contains(&query)).map(Some))
                    .enumerate()
                    .map(|(i, e)| {
                        let e = e.cloned();
                        let name = e.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| "- None -".into());
                        let dens = e.as_ref().map(|e| e.density_text.clone()).unwrap_or_else(|| "0.000e+0 lb/in³".into());
                        // Live: 44 px rows, 20 px in, #555; the current one #b2ddf6.
                        let current = e.as_ref().map(|e| &e.name) == picked.as_ref().map(|p| &p.name);
                        v_flex()
                            .id(SharedString::from(format!("mat-row-{i}")))
                            .h(px(44.))
                            .flex_none()
                            .justify_center()
                            .px(px(20.))
                            .cursor_pointer()
                            .when(current, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                            .hover(|d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                            .child(div().text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).child(name))
                            .child(div().text_size(px(12.)).text_color(gpui_kit::rgb(0x777777)).child(format!("Density: {dens}")))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(m) = &mut this.ui.material_panel {
                                    m.picked = e.clone();
                                    m.list_open = false;
                                }
                                cx.notify();
                            }))
                            .into_any_element()
                    })
                    .collect();
                body = body.child(div().pl(px(3.)).pr(px(3.)).child(div().id("mat-list").h(px(200.)).py(px(4.)).rounded(px(5.)).border_1().border_color(gpui_kit::rgb(0xd7d7d7)).bg(gpui_kit::rgb(0xffffff)).overflow_y_scroll().children(rows)));
            }
            match &picked {
                // Live (assign-material-03): hidden while the list is open.
                None if list_open => {}
                None => body = body.child(div().h(px(22.)).pl(px(2.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("No material assigned")),
                Some(e) => {
                    let vals: Vec<String> = std::iter::once(e.name.clone()).chain(std::iter::once(format!("{:.3}", e.density / LB_IN3).trim_end_matches('0').trim_end_matches('.').to_string())).chain(e.extra.iter().cloned()).collect();
                    for (i, f) in FIELDS.iter().enumerate() {
                        body = body.child(h_flex().pr(px(5.)).child(label(f)).child(ro_value(vals.get(i).cloned().unwrap_or_default())));
                    }
                }
            }
        }
        Some(
            v_flex()
                .id("material-panel")
                .occlude()
                .absolute()
                // Live (dom/dialog-assign-material*.txt): 280x368 in every
                // state, flush with the bottom of the graphics area.
                .left(px(40.))
                .bottom_0()
                .w(px(280.))
                .h(px(368.))
                .pb(px(12.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(pal.panel)
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Material"))
                        .child(
                            div()
                                .id("mat-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if can_apply { pal.accept } else { gpui_kit::rgb(0xcce9cc) })
                                .when(can_apply, |d| d.cursor_pointer())
                                .child(crate::assets::icon("dialog-ok", 20., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_material(cx))),
                        )
                        .child(
                            div()
                                .id("mat-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .child(crate::assets::icon("dialog-cancel", 18., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.material_panel = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(h_flex().border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(tab("mat-tab-lib", !custom, false, cx)).child(tab("mat-tab-custom", custom, true, cx)))
                .child(div().id("mat-body").flex_1().min_h_0().overflow_y_scroll().child(body))
                // The dialog's help button at the bottom right (live 14 px, 4 px in).
                .child(div().absolute().right(px(4.)).bottom(px(4.)).size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?"))
                .into_any_element(),
        )
    }

    /// "+" › "Add material library": the picked library is added to the
    /// account and chosen in the select.
    fn open_add_material_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::material_libraries::open(
            move |lib, _, cx| {
                view.update(cx, |this, cx| {
                    let i = match &lib {
                        None => 0,
                        Some(l) => this.material_sources(cx).iter().position(|(s, _)| matches!(s, Source::Added(a) if a == l)).unwrap_or(0),
                    };
                    if let Some(m) = &mut this.ui.material_panel {
                        m.library = i;
                        m.picked = None;
                    }
                    cx.notify();
                })
            },
            window,
            cx,
        );
    }
}
