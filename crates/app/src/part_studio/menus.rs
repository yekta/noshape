//! Viewport context menu, measurement panel, mass properties and part
//! properties for the Part Studio view.

use super::PartStudioView;
use crate::scene_builder::{PickKind, PickTarget};
use crate::theme::{p, with_alpha};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::partstudio::{Appearance, PartProperties};
use noshape_model::reference::{BodyId, FeatureId, Reference};
use noshape_model::regen::measure::{self, Measurement};
use noshape_render::StandardView;
use noshape_units::{Dim, Quantity};

/// Common materials with densities (kg/m³).
pub const MATERIALS: &[(&str, f64)] = &[
    ("Steel", 7850.0),
    ("Stainless steel", 8000.0),
    ("Aluminum 6061", 2700.0),
    ("Titanium Ti-6Al-4V", 4430.0),
    ("Brass", 8500.0),
    ("Copper", 8960.0),
    ("ABS", 1040.0),
    ("PLA", 1240.0),
    ("PETG", 1270.0),
    ("Nylon", 1150.0),
    ("Acrylic", 1180.0),
    ("Oak", 750.0),
    ("Plywood", 600.0),
];

const SWATCHES: &[u32] = &[0xb8bec9, 0x9fb7d6, 0xd6b89f, 0xa7cfae, 0xcfa7c7, 0xd6d29f, 0x9fd3d6, 0xc9a9a9, 0x5d6470, 0x2f3540, 0xe0e3e8, 0xd9534f];

impl PartStudioView {
    /// Body owning a referenced entity.
    fn owning_body(&self, r: &Reference) -> Option<BodyId> {
        match r {
            Reference::Body(b) => Some(b.clone()),
            _ => self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| b.id.clone()),
        }
    }

    /// Feature that created a referenced entity.
    fn creating_feature(&self, r: &Reference) -> Option<FeatureId> {
        match r {
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } => Some(*sketch),
            Reference::Face(n) => n.creator(),
            Reference::Body(b) => Some(b.feature),
            Reference::FeaturePlane(f) => Some(*f),
            Reference::MateConnector { feature, .. } => Some(*feature),
            _ => None,
        }
    }

    pub(super) fn open_context_menu(&mut self, target: Option<PickTarget>, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let in_sketch = self.sketch.is_some();
        let target_ref = target.as_ref().map(|t| t.reference.clone());
        let kind = target.as_ref().map(|t| t.kind);
        let body = target_ref.as_ref().and_then(|r| self.owning_body(r));
        let creator = target_ref.as_ref().and_then(|r| self.creating_feature(r));
        let creator_name = creator.and_then(|id| self.studio(cx).and_then(|s| s.feature(id)).map(|f| f.name.clone()));
        let sheet_metal = body.as_ref().is_some_and(|b| self.context.as_ref().is_some_and(|c| c.sheet_metal.contains_key(b)));
        let menu = PopupMenu::build(window, cx, move |mut m, _window, _cx| {
            let v = view.clone();
            let item = |label: &str, f: Box<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>| {
                let v = v.clone();
                PopupMenuItem::new(label.to_string()).on_click(move |_, window, cx| v.update(cx, |this, cx| {
                    this.close_context_menu(cx);
                    f(this, window, cx)
                }))
            };
            if let (Some(r), Some(k)) = (target_ref.clone(), kind) {
                let r2 = r.clone();
                match k {
                    PickKind::Face | PickKind::Plane | PickKind::SketchRegion if !in_sketch => {
                        if matches!(k, PickKind::Plane) || matches!(k, PickKind::Face) {
                            let r3 = r.clone();
                            m = m.item(item("New sketch", Box::new(move |this, w, cx| {
                                this.selection = vec![r3.clone()];
                                this.start_feature("sketch", w, cx);
                            })));
                        }
                        if matches!(k, PickKind::SketchRegion) {
                            let r3 = r.clone();
                            m = m.item(item("Extrude", Box::new(move |this, w, cx| {
                                this.selection = vec![r3.clone()];
                                this.start_feature("extrude", w, cx);
                            })));
                            let r4 = r.clone();
                            m = m.item(item("Revolve", Box::new(move |this, w, cx| {
                                this.selection = vec![r4.clone()];
                                this.start_feature("revolve", w, cx);
                            })));
                        }
                        let r5 = r.clone();
                        m = m.item(item("View normal to", Box::new(move |this, _, cx| {
                            this.hover = Some(r5.clone());
                            this.view_normal(cx);
                        })));
                    }
                    PickKind::Edge if !in_sketch => {
                        let r3 = r.clone();
                        m = m.item(item("Fillet", Box::new(move |this, w, cx| {
                            this.selection = vec![r3.clone()];
                            this.start_feature("fillet", w, cx);
                        })));
                        let r4 = r.clone();
                        m = m.item(item("Chamfer", Box::new(move |this, w, cx| {
                            this.selection = vec![r4.clone()];
                            this.start_feature("chamfer", w, cx);
                        })));
                    }
                    _ => {}
                }
                if let (Some(id), Some(name)) = (creator, creator_name.clone())
                    && !in_sketch
                {
                    m = m.item(item(&format!("Edit {name}"), Box::new(move |this, w, cx| this.edit_feature(id, w, cx))));
                }
                m = m.separator();
                let r6 = r2.clone();
                m = m.item(item("Measure", Box::new(move |this, _, cx| {
                    if !this.selection.contains(&r6) {
                        this.selection.push(r6.clone());
                    }
                    this.measure_cache = None;
                    this.refresh_scene(false, cx);
                    cx.notify();
                })));
            }
            if let Some(b) = body.clone() {
                m = m.separator().label("Part");
                let b1 = b.clone();
                m = m.item(item("Part properties…", Box::new(move |this, w, cx| this.open_part_properties(b1.clone(), w, cx))));
                let b2 = b.clone();
                m = m.item(item("Hide part", Box::new(move |this, _, cx| this.set_part_hidden(&b2, true, cx))));
                let b3 = b.clone();
                m = m.item(item("Isolate", Box::new(move |this, _, cx| this.isolate_part(&b3, cx))));
                if sheet_metal {
                    let b5 = b.clone();
                    m = m.item(item("Export flat pattern (DXF)…", Box::new(move |this, w, cx| this.export_flat(b5.clone(), "dxf", w, cx))));
                    let b6 = b.clone();
                    m = m.item(item("Export flat pattern (SVG)…", Box::new(move |this, w, cx| this.export_flat(b6.clone(), "svg", w, cx))));
                }
                let b4 = b.clone();
                m = m.item(item("Mass properties", Box::new(move |this, _, cx| {
                    this.selection = vec![Reference::Body(b4.clone())];
                    this.show_mass = true;
                    cx.notify();
                })));
            }
            m = m.separator();
            m = m.item(item("Show all parts", Box::new(|this, _, cx| this.show_all_parts(cx))));
            m = m.item(item("Zoom to fit", Box::new(|this, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx)))));
            m = m.item(item("Isometric view", Box::new(|this, _, cx| this.view(StandardView::Isometric, cx))));
            m.item(item("Mass properties of all parts", Box::new(|this, _, cx| {
                this.selection.clear();
                this.show_mass = true;
                cx.notify();
            })))
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        cx.notify();
    }

    pub(super) fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn render_context_menu(&self) -> Vec<AnyElement> {
        match &self.context_menu {
            Some((pos, menu, _)) => vec![deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(menu.clone())).with_priority(2).into_any_element()],
            None => Vec::new(),
        }
    }

    // ------------------------------------------------------------ parts

    fn edit_part_props(&mut self, body: &BodyId, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut PartProperties)) {
        let body = body.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, desc, cx, |ps| f(ps.part_properties.entry(body).or_default()));
        });
    }

    pub(super) fn set_part_hidden(&mut self, body: &BodyId, hidden: bool, cx: &mut Context<Self>) {
        self.edit_part_props(body, if hidden { "Hide part" } else { "Show part" }, cx, |p| p.hidden = hidden);
    }

    pub(super) fn show_all_parts(&mut self, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Show all parts", cx, |ps| {
                for p in ps.part_properties.values_mut() {
                    p.hidden = false;
                }
            });
        });
    }

    pub(super) fn isolate_part(&mut self, body: &BodyId, cx: &mut Context<Self>) {
        let others: Vec<BodyId> = self.display.bodies.iter().map(|b| b.id.clone()).filter(|b| b != body).collect();
        let body = body.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Isolate part", cx, |ps| {
                for o in &others {
                    ps.part_properties.entry(o.clone()).or_default().hidden = true;
                }
                ps.part_properties.entry(body).or_default().hidden = false;
            });
        });
    }

    pub(super) fn open_part_properties(&mut self, body: BodyId, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let view = cx.entity();
        let props = self.studio(cx).and_then(|s| s.part_properties.get(&body).cloned()).unwrap_or_default();
        let pal = p();
        window.open_dialog(cx, move |dialog, _, _| {
            let v = view.clone();
            let b = body.clone();
            let name2 = name.clone();
            let swatches = SWATCHES.iter().map(|hex| {
                let v = view.clone();
                let b = body.clone();
                let hex = *hex;
                div()
                    .id(SharedString::from(format!("sw-{hex:x}")))
                    .size(px(22.))
                    .rounded_md()
                    .bg(rgb(hex))
                    .border_1()
                    .border_color(pal.border_strong)
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| this.edit_part_props(&b, "Change appearance", cx, |p| p.appearance = Some(Appearance { color: crate::theme::lin(hex) })));
                    })
            });
            let materials = MATERIALS.iter().map(|(m, rho)| {
                let v = view.clone();
                let b = body.clone();
                let selected = props.material.as_deref() == Some(*m);
                let m2 = m.to_string();
                let rho = *rho;
                Button::new(SharedString::from(format!("mat-{m}")))
                    .label(*m)
                    .xsmall()
                    .when_selected(selected)
                    .on_click(move |_, _, cx| {
                        let m3 = m2.clone();
                        v.update(cx, |this, cx| this.edit_part_props(&b, "Assign material", cx, |p| {
                            p.material = Some(m3);
                            p.density = Some(rho);
                        }));
                    })
            });
            dialog
                .title("Part properties")
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Name")).child(Input::new(&name)))
                        .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Appearance")).child(h_flex().gap_1p5().flex_wrap().children(swatches)))
                        .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Material")).child(h_flex().gap_1().flex_wrap().children(materials))),
                )
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(DialogClose::new().child(Button::new("cancel").label("Close").outline()))
                        .child(DialogAction::new().child(Button::new("ok").label("Apply name").primary())),
                )
                .on_ok(move |_, _, cx| {
                    let n = name2.read(cx).value().to_string();
                    let b2 = b.clone();
                    v.update(cx, |this, cx| this.edit_part_props(&b2, "Rename part", cx, |p| p.name = Some(n.clone()).filter(|s| !s.trim().is_empty())));
                    true
                })
        });
    }

    pub(super) fn export_flat(&mut self, body: BodyId, ext: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let geometry = match noshape_model::sheetmetal::flat_pattern_for(&ctx, &body) {
            Ok(f) => f.to_sheet_geometry(),
            Err(e) => {
                self.message = Some((noshape_model::display::FeatureStatus::Error, e));
                cx.notify();
                return;
            }
        };
        let name = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_else(|| "flat".into());
        let view = cx.entity();
        crate::files::request_save_path("Export flat pattern", format!("{name} flat.{ext}"), ext, window, cx, move |path, cx| {
            let bytes = if ext == "dxf" { noshape_model::drawing::export::to_dxf(&geometry).into_bytes() } else { noshape_model::drawing::export::to_svg(&geometry).into_bytes() };
            let msg = match std::fs::write(&path, bytes) {
                Ok(()) => (noshape_model::display::FeatureStatus::Info, format!("Exported {}", path.display())),
                Err(e) => (noshape_model::display::FeatureStatus::Error, format!("Export failed: {e}")),
            };
            view.update(cx, |this, cx| {
                this.message = Some(msg);
                cx.notify();
            });
        });
    }

    // ------------------------------------------------------------ measurement

    fn fmt(&self, q: Quantity, cx: &App) -> String {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        noshape_units::format_quantity(q, &units)
    }

    pub(super) fn render_measure_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        if self.edit.is_some() || self.context.is_none() {
            return Vec::new();
        }
        let ctx = self.context.clone().unwrap();
        let mut rows: Vec<(String, String)> = Vec::new();
        let mut title = "Measure";
        if self.show_mass {
            title = "Mass properties";
            let bodies: Vec<BodyId> = self.selection.iter().filter_map(|r| self.owning_body(r)).collect();
            let props = self.studio(cx).map(|s| s.part_properties.clone()).unwrap_or_default();
            if let Some(mp) = measure::mass_properties(&ctx, &bodies, &|b| props.get(b).and_then(|p| p.density)) {
                let len3 = Dim { length: 3, angle: 0 };
                let len2 = Dim { length: 2, angle: 0 };
                rows.push(("Volume".into(), self.fmt(Quantity::new(mp.volume, len3), cx)));
                rows.push(("Surface area".into(), self.fmt(Quantity::new(mp.surface_area, len2), cx)));
                rows.push(("Mass".into(), mp.mass.map(|m| format!("{m:.4} kg")).unwrap_or_else(|| "assign a material".into())));
                let c = mp.center_of_mass;
                let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                rows.push(("Center of mass".into(), format!("{}, {}, {}", l(c.x), l(c.y), l(c.z))));
            }
        } else {
            if self.selection.is_empty() {
                return Vec::new();
            }
            let sel = self.selection.clone();
            let results = match &self.measure_cache {
                Some((s, r)) if *s == sel => r.clone(),
                _ => {
                    let r = measure::measure(&ctx, &sel);
                    self.measure_cache = Some((sel, r.clone()));
                    r
                }
            };
            for (label, m) in results {
                let v = match m {
                    Measurement::Length(x) | Measurement::Radius(x) | Measurement::Diameter(x) => self.fmt(Quantity::new(x, Dim::LENGTH), cx),
                    Measurement::Area(x) => self.fmt(Quantity::new(x, Dim { length: 2, angle: 0 }), cx),
                    Measurement::Volume(x) => self.fmt(Quantity::new(x, Dim { length: 3, angle: 0 }), cx),
                    Measurement::Angle(x) => self.fmt(Quantity::new(x, Dim::ANGLE), cx),
                    Measurement::Point(p) => {
                        let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                        format!("{}, {}, {}", l(p.x), l(p.y), l(p.z))
                    }
                    Measurement::Distance { distance, dx, dy, dz, .. } => {
                        let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                        rows.push(("ΔX / ΔY / ΔZ".into(), format!("{} / {} / {}", l(dx), l(dy), l(dz))));
                        l(distance)
                    }
                };
                rows.push((label, v));
            }
            if rows.is_empty() {
                return Vec::new();
            }
        }
        let close = Button::new("measure-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.show_mass = false;
            this.selection.clear();
            this.refresh_scene(false, cx);
            cx.notify();
        }));
        vec![
            v_flex()
                .absolute()
                .bottom_3()
                .right_3()
                .w(px(280.))
                .p_2()
                .gap_1()
                .rounded_lg()
                .bg(with_alpha(pal.panel_raised, 0.95))
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(h_flex().justify_between().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(title.to_uppercase())).child(close))
                .children(rows.into_iter().map(|(k, v)| {
                    h_flex().justify_between().gap_2().child(div().text_xs().text_color(pal.text_muted).child(k)).child(div().text_xs().text_color(pal.text).child(v))
                }))
                .into_any_element(),
        ]
    }
}

trait WhenSelected {
    fn when_selected(self, on: bool) -> Self;
}

impl WhenSelected for Button {
    fn when_selected(self, on: bool) -> Self {
        use gpui_kit::component::Selectable as _;
        self.selected(on)
    }
}
