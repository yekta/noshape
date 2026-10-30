//! Onshape's "Sheet metal table and flat view" panel (live "qa5 sm" (8),
//! qa5/sm-091…105, dark/dk-973; bundle template
//! /project/web/ts/sheetmetal/sheetmetal.html and woolsthorpe.css): the
//! "Sheet metal context:" select over the collapsible Bends and Other
//! joints tables (top half), a splitter, and the flat view (bottom half).

use super::PartStudioView;
use super::flat_view::{FlatPick, FlatView, FlatViewEvent};
use crate::os_menu::{OsMenu, OsMenuItem};
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::reference::{BodyId, FeatureId, Reference};
use noshape_model::sheetmetal::SheetMetalData;
use noshape_ops::sheetmetal::{bend_faces, bend_row_order, insert_joint_edit, joint_edit_index, joint_rows, plain};
use std::sync::Arc;

/// os-splitter-container: the tables' default share and the two minimums
/// (os-default-length 50 %, os-min-length 140 / 210).
const SPLIT_MIN_TOP: f32 = 140.0;
const SPLIT_MIN_BOTTOM: f32 = 210.0;

thread_local! {
    /// The tables' share of the height the user dragged (live: kept across
    /// a reload as a proportion, "ui (10-24 a)" (3)).
    static SPLIT: std::cell::Cell<Option<f32>> = std::cell::Cell::new(
        std::fs::read_to_string(crate::paths::config_dir().join("sheet_metal_split.txt")).ok().and_then(|t| t.trim().parse().ok()),
    );
    /// The tables' share when the splitter was pressed, and the pointer y.
    static SPLIT_PRESS: std::cell::Cell<(f32, Pixels)> = const { std::cell::Cell::new((0.5, Pixels::ZERO)) };
}

/// The splitter being dragged.
#[derive(Clone)]
struct SmSplitDrag;

impl Render for SmSplitDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl PartStudioView {
    /// The sheet metal models (feature, name), by name (live: orderBy name).
    fn sheet_metal_models(&self, cx: &App) -> Vec<(FeatureId, String)> {
        let Some(ctx) = self.context.as_ref() else { return Vec::new() };
        let studio = self.studio(cx);
        let mut out: Vec<(FeatureId, String)> = Vec::new();
        for d in ctx.sheet_metal.values() {
            if out.iter().any(|(f, _)| *f == d.model) {
                continue;
            }
            let name = studio.and_then(|s| s.feature(d.model)).map(|f| f.name.clone()).unwrap_or_else(|| "Sheet metal model".into());
            out.push((d.model, name));
        }
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    /// The active context: the chosen model, else the first by name.
    fn sheet_metal_context(&self, models: &[(FeatureId, String)]) -> Option<FeatureId> {
        self.ui.sm_context.filter(|m| models.iter().any(|(f, _)| f == m)).or_else(|| models.first().map(|(f, _)| *f))
    }

    /// Right-click in the flat view (live "qa5 sm" (9), "ui (10-24 a)" (4e)).
    fn open_flat_menu(&mut self, position: Point<Pixels>, on_part: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !on_part {
            // Live (4e): empty flat space offers only these.
            let fv = self.flat_view.clone();
            let (f1, f2) = (fv.clone(), fv);
            let menu = OsMenu::build(window, cx, move |m, _, _| {
                let (f1, f2) = (f1.clone(), f2.clone());
                m.item(OsMenuItem::new("Zoom to fit").on_click(move |_, _, cx| {
                    if let Some(f) = &f1 {
                        f.update(cx, |f, cx| f.zoom_to_fit(cx));
                    }
                }))
                .item(OsMenuItem::new("Isometric").on_click(move |_, _, cx| {
                    if let Some(f) = &f2 {
                        f.update(cx, |f, cx| f.isometric(cx));
                    }
                }))
            });
            let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
            menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
            self.context_menu = Some((position, menu, sub));
            cx.notify();
            return;
        }
        let models = self.sheet_metal_models(cx);
        let Some(model) = self.sheet_metal_context(&models) else { return };
        let model_name = models.iter().find(|(f, _)| *f == model).map(|(_, n)| n.clone()).unwrap_or_default();
        let Some(body) = self.context.as_ref().and_then(|c| c.sheet_metal.iter().find(|(_, d)| d.model == model).map(|(b, _)| b.clone())) else { return };
        // New sketch…: on the base wall's face on the flat's side (live
        // request 1605: "Face of Sheet metal 1", while the model is active).
        let sketch_face = self.context.as_ref().and_then(|c| c.sheet_metal.get(&body)).filter(|d| d.active).and_then(|d| {
            let w = d.walls.iter().find(|w| !w.is_virtual())?;
            let side = if d.view_back { &w.back } else { &w.faces };
            side.first().map(|n| noshape_model::reference::Reference::Face((**n).clone()))
        });
        let part = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_default();
        let sketch = self.studio(cx).and_then(|s| s.feature(model).and_then(|f| noshape_model::interaction::source_sketch(&s.features, f)).and_then(|id| s.feature(id).map(|f| (id, f.name.clone()))));
        let (view, fv) = (cx.entity(), self.flat_view.clone());
        // Live (sm-111), in groups: Edit <model>…, Edit <sketch>…, Show
        // dimensions, Show dependencies of <model>… | New sketch… | Copy Flat
        // pattern of <part> | Create Drawing of Flat pattern of <part>…, Export
        // DXF/DWG of Flat pattern of <part>… | Select other… | Add comment |
        // Zoom to fit, Zoom to selection, View normal to | Delete <model>.
        // "Add comment" is cloud (not built, as in the other menus).
        // Not built: Copy Flat pattern of <part> (live pastes a flat-pattern
        // part instance into an assembly: our flats aren't parts).
        let menu = OsMenu::build(window, cx, move |m, _, _| {
            type Action = Box<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;
            let item = |label: String, f: Action| {
                let v = view.clone();
                OsMenuItem::new(label).on_click(move |_, window, cx| {
                    v.update(cx, |this, cx| {
                        this.close_context_menu(cx);
                        f(this, window, cx)
                    })
                })
            };
            let mut m = m.truncate_labels().item(item(format!("Edit {model_name}…"), Box::new(move |this, w, cx| this.edit_feature(model, w, cx))));
            if let Some((sid, sname)) = sketch.clone() {
                m = m.item(item(format!("Edit {sname}…"), Box::new(move |this, w, cx| this.edit_feature(sid, w, cx)))).item(item("Show dimensions".into(), Box::new(move |this, w, cx| this.show_dimensions(sid, w, cx))));
            }
            let b = body.clone();
            let (f1, f2, f3) = (fv.clone(), fv.clone(), fv.clone());
            let mut m = m.item(item(format!("Show dependencies of {model_name}…"), Box::new(move |this, _, cx| {
                this.ui.dependencies = Some(model);
                cx.notify();
            })));
            if let Some(face) = sketch_face.clone() {
                m = m.separator().item(item("New sketch…".into(), Box::new(move |this, w, cx| {
                    this.selection = vec![face.clone()];
                    this.start_feature("sketch", w, cx);
                })));
            }
            m.separator()
            .item(item(format!("Create Drawing of Flat pattern of {part}…"), {
                let (b, part) = (b.clone(), part.clone());
                Box::new(move |this, w, cx| {
                    // Live (dx 10-27): a drawing "Flat pattern of Part 1", Suggested views from its flat view.
                    let source = Some(noshape_model::drawing::create::DrawingSource { tab: this.tab, part: Some(b.clone()), config: Default::default(), sketch: None, flat: true });
                    crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(format!("Flat pattern of {part}")) }, w, cx);
                })
            }))
            .item(item(format!("Export DXF/DWG of Flat pattern of {part}…"), Box::new(move |this, w, cx| this.open_flat_export(b.clone(), w, cx))))
            .separator()
            .item(item("Select other…".into(), {
                let label = format!("Face of {model_name}");
                Box::new(move |this, w, cx| this.open_flat_select_other(position, label.clone(), w, cx))
            }))
            .separator()
            .item(item("Zoom to fit".into(), Box::new(move |_, _, cx| {
                if let Some(fv) = &f1 {
                    fv.update(cx, |f, cx| f.zoom_to_fit(cx));
                }
            })))
            .item(item("Zoom to selection".into(), Box::new(move |_, _, cx| {
                if let Some(fv) = &f3 {
                    fv.update(cx, |f, cx| f.zoom_to_selection(cx));
                }
            })))
            // The flat lies in XY: normal to any of its faces is Top.
            .item(item("View normal to".into(), Box::new(move |_, _, cx| {
                if let Some(fv) = &f2 {
                    fv.update(cx, |f, cx| f.top(cx));
                }
            })))
            .separator()
            .item(item(format!("Delete {model_name}"), Box::new(move |this, _, cx| this.delete_features(vec![model], cx))).icon_name("menu-delete"))
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        cx.notify();
    }

    /// The model face of the flat's reference side (`true`) or back: the
    /// base wall's (live request 1605: picking the flat's face selects it).
    fn flat_model_face(&self, side: bool) -> Option<noshape_model::reference::Reference> {
        let models = self.sheet_metal_models_ctx();
        let model = self.ui.sm_context.or(models)?;
        let d = self.context.as_ref()?.sheet_metal.values().find(|d| d.model == model)?;
        let w = d.walls.iter().find(|w| !w.is_virtual())?;
        let faces = if side != d.view_back { &w.faces } else { &w.back };
        faces.first().map(|n| noshape_model::reference::Reference::Face((**n).clone()))
    }

    /// A bend line's right-click menu in the flat view (live request 1605:
    /// Curve/surface analysis…, Select other…, Zoom to fit, Zoom to selection,
    /// Align view with vertical axis; the first two need a model entity the
    /// flat's line isn't).
    fn open_flat_bend_menu(&mut self, position: Point<Pixels>, bend: usize, window: &mut Window, cx: &mut Context<Self>) {
        let (view, fv) = (cx.entity(), self.flat_view.clone());
        let menu = OsMenu::build(window, cx, move |m, _, _| {
            let item = |label: &'static str, f: fn(&mut FlatView, usize, &mut Context<FlatView>)| {
                let (v, fv) = (view.clone(), fv.clone());
                OsMenuItem::new(label).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| this.close_context_menu(cx));
                    if let Some(fv) = &fv {
                        fv.update(cx, |flat, cx| f(flat, bend, cx));
                    }
                })
            };
            m.item(item("Zoom to fit", |f, _, cx| f.zoom_to_fit(cx))).item(item("Zoom to selection", |f, _, cx| f.zoom_to_selection(cx))).item(item("Align view with vertical axis", |f, b, cx| f.align_bend_vertical(b, cx)))
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        cx.notify();
    }

    /// Select other… on the flat's face (live request 1605): its two faces,
    /// the reference side and the back, both "Face of <model>".
    fn open_flat_select_other(&mut self, position: Point<Pixels>, label: String, window: &mut Window, cx: &mut Context<Self>) {
        let (view, fv) = (cx.entity(), self.flat_view.clone());
        let menu = OsMenu::build(window, cx, move |mut m, _, _| {
            for side in [true, false] {
                let (v, fv) = (view.clone(), fv.clone());
                m = m.item(OsMenuItem::new(label.clone()).on_click(move |_, _, cx| {
                    if let Some(fv) = &fv {
                        fv.update(cx, |f, cx| f.select(Some(side), cx));
                    }
                    v.update(cx, |this, cx| {
                        this.close_context_menu(cx);
                        this.select_flat_face(Some(side), cx);
                    });
                }));
            }
            m
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        cx.notify();
    }

    /// The first sheet metal model of the context by name (no `App` needed).
    fn sheet_metal_models_ctx(&self) -> Option<FeatureId> {
        self.context.as_ref()?.sheet_metal.values().map(|d| d.model).next()
    }

    /// The flat's face selected in the flat view: the model face selected.
    fn select_flat_face(&mut self, side: Option<bool>, cx: &mut Context<Self>) {
        self.selection = side.and_then(|s| self.flat_model_face(s)).into_iter().collect();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The active model's stored Bends row order.
    fn sm_stored_order(&self, model: FeatureId, cx: &App) -> Vec<Reference> {
        self.studio(cx).and_then(|s| s.bend_order.iter().find(|(m, _)| *m == model).map(|(_, v)| v.clone())).unwrap_or_default()
    }

    /// Move up (`delta` -1) / Move down (+1) bend `bend` in the table.
    fn sm_move_bend(&mut self, model: FeatureId, bend: usize, delta: isize, cx: &mut Context<Self>) {
        let Some(data) = self.context.as_ref().and_then(|c| c.sheet_metal.values().find(|d| d.model == model).cloned()) else { return };
        let mut order = bend_row_order(&data, &self.sm_stored_order(model, cx));
        let Some(pos) = order.iter().position(|&i| i == bend) else { return };
        let to = pos as isize + delta;
        if to < 0 || to as usize >= order.len() {
            return;
        }
        order.swap(pos, to as usize);
        let list: Vec<Reference> = order.iter().filter_map(|&i| bend_faces(&data.bends[i]).into_iter().next()).collect();
        self.edit_config(if delta < 0 { "Move up" } else { "Move down" }, cx, |ps| {
            ps.bend_order.retain(|(m, _)| *m != model);
            ps.bend_order.push((model, list));
        });
        cx.notify();
    }

    /// The faces of Bends row `row` of the active model.
    fn sm_bend_refs(&self, row: usize) -> Vec<noshape_model::reference::Reference> {
        let Some(ctx) = self.context.as_ref() else { return Vec::new() };
        let model = self.ui.sm_context.filter(|m| ctx.sheet_metal.values().any(|d| d.model == *m)).or_else(|| ctx.sheet_metal.values().next().map(|d| d.model));
        let Some(data) = ctx.sheet_metal.values().find(|d| Some(d.model) == model) else { return Vec::new() };
        data.bends.get(row).map(|b| b.inner_faces.iter().chain(&b.outer_faces).map(|n| noshape_model::reference::Reference::Face((**n).clone())).collect()).unwrap_or_default()
    }

    /// The bend under the pointer in the Bends table (pre-highlighted).
    pub(super) fn sm_hover_refs(&self) -> Vec<noshape_model::reference::Reference> {
        match self.ui.sm_hover {
            Some((0, row, _)) if self.right_panel == Some(super::config_panel::RightPanel::SheetMetal) => self.sm_bend_refs(row),
            _ => Vec::new(),
        }
    }

    /// Right-click on a Bends row. Live: Move up / Move down (not the first
    /// / last row), a separator, "Convert <name> to rip".
    fn open_bend_row_menu(&mut self, row: usize, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let models = self.sheet_metal_models(cx);
        let Some(model) = self.sheet_metal_context(&models) else { return };
        let Some(data) = self.context.as_ref().and_then(|c| c.sheet_metal.values().find(|d| d.model == model).cloned()) else { return };
        let studio = self.studio(cx);
        let order = |f: FeatureId| studio.and_then(|s| s.features.iter().position(|x| x.id == f)).unwrap_or(usize::MAX);
        let up = |_: usize| None;
        let (bends, _) = joint_rows(&data, &up, &order);
        let Some(name) = bends.get(row).map(|r| r.name.clone()) else { return };
        let Some(face) = data.bends.get(row).and_then(|b| b.inner_faces.first().or(b.outer_faces.first())).map(|n| noshape_model::reference::Reference::Face((**n).clone())) else { return };
        let order = bend_row_order(&data, &self.sm_stored_order(model, cx));
        let pos = order.iter().position(|&i| i == row).unwrap_or(0);
        let (first, last) = (pos == 0, pos + 1 >= order.len());
        let view = cx.entity();
        let menu = OsMenu::build(window, cx, move |mut m, _, _| {
            for (show, label, delta) in [(!first, "Move up", -1isize), (!last, "Move down", 1)] {
                if show {
                    let v = view.clone();
                    m = m.item(OsMenuItem::new(label).on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.close_context_menu(cx);
                            this.sm_move_bend(model, row, delta, cx);
                        })
                    }));
                }
            }
            if !(first && last) {
                m = m.separator();
            }
            let v = view.clone();
            let face = face.clone();
            m.item(OsMenuItem::new(format!("Convert {name} to rip")).on_click(move |_, _, cx| {
                let face = face.clone();
                v.update(cx, |this, cx| {
                    this.close_context_menu(cx);
                    this.sm_convert_to_rip(model, face, cx);
                })
            }))
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        cx.notify();
    }

    /// "Convert … to rip": a Modify joint of the bend, Rip / Edge joint
    /// (live "ui (10-24 b)").
    fn sm_convert_to_rip(&mut self, model: FeatureId, face: noshape_model::reference::Reference, cx: &mut Context<Self>) {
        use noshape_model::params::ParamValue;
        let units = self.doc.read(cx).session.doc.settings.units;
        self.edit_config("Convert joint to rip.", cx, |ps| {
            let Some(mut f) = ps.new_feature("sheetMetalJoint", &units) else { return };
            f.params.set("entity", ParamValue::Query(vec![face]));
            f.params.set("jointType", ParamValue::Enum("RIP".into()));
            f.params.set("jointStyle", ParamValue::Enum("EDGE".into()));
            let at = joint_edit_index(ps, model);
            insert_joint_edit(ps, f, at);
        });
        self.selection.clear();
        cx.notify();
    }

    /// Apply an edited Bends cell: live adds a "Modify joint N" for that bend
    /// with the value overridden ("3" + Tab on Bend B's radius → Modify
    /// joint 1, radius 3 mm) at the rollback bar or before the model's
    /// Finish; a second edit of the bend changes that Modify joint (live "ui
    /// (10-24 b)"). A bare number is in the document's unit.
    fn sm_apply_edit(&mut self, row: usize, col: usize, text: String, cx: &mut Context<Self>) {
        use noshape_model::params::ParamValue;
        use noshape_units::ExprValue;
        // Tab commits, then the field's blur would again: apply once.
        if self.ui.sm_edit != Some((row, col)) {
            return;
        }
        self.ui.sm_edit = None;
        let text = text.trim().to_string();
        let models = self.sheet_metal_models(cx);
        let Some(model) = self.sheet_metal_context(&models) else { return };
        let Some(data) = self.context.as_ref().and_then(|c| c.sheet_metal.values().find(|d| d.model == model).cloned()) else { return };
        let Some(face) = data.bends.get(row).and_then(|b| b.inner_faces.first().or(b.outer_faces.first())).map(|n| noshape_model::reference::Reference::Face((**n).clone())) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let current = if col == 2 { plain(data.bends[row].radius / units.si_factor(noshape_units::Dim::LENGTH)) } else { plain(data.bends[row].k_factor) };
        if text.is_empty() || text == current {
            cx.notify();
            return;
        }
        let bare = text.parse::<f64>().is_ok();
        let value = if col == 2 && bare { format!("{text} {}", units.length.symbol()) } else { text };
        let faces = bend_faces(&data.bends[row]);
        let set = move |p: &mut noshape_model::params::ParamValues| {
            if col == 2 {
                p.set("useDefaultRadius", ParamValue::Bool(false));
                p.set("radius", ParamValue::Expr(ExprValue::new(&value)));
            } else {
                p.set("useDefaultKFactor", ParamValue::Bool(false));
                p.set("bendCalculationType", ParamValue::Enum("K_FACTOR".into()));
                p.set("kFactor", ParamValue::Expr(ExprValue::new(&value)));
            }
        };
        self.edit_config("Modify joint", cx, |ps| {
            let at = joint_edit_index(ps, model);
            let from = ps.features.iter().position(|f| f.id == model).map_or(0, |i| i + 1);
            let is_this_bend = |f: &noshape_model::partstudio::Feature| {
                f.type_id == "sheetMetalJoint"
                    && !f.suppressed
                    && matches!(f.params.0.get("jointType"), Some(ParamValue::Enum(t)) if t == "BEND")
                    && matches!(f.params.0.get("entity"), Some(ParamValue::Query(q)) if q.iter().any(|r| faces.contains(r)))
            };
            if let Some(i) = (from..at.min(ps.features.len())).rev().find(|&i| is_this_bend(&ps.features[i])) {
                set(&mut ps.features[i].params);
                return;
            }
            let Some(mut f) = ps.new_feature("sheetMetalJoint", &units) else { return };
            f.params.set("entity", ParamValue::Query(vec![face]));
            f.params.set("jointType", ParamValue::Enum("BEND".into()));
            set(&mut f.params);
            insert_joint_edit(ps, f, at);
        });
        cx.notify();
    }

    pub(super) fn render_sheet_metal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let models = self.sheet_metal_models(cx);
        let active = self.sheet_metal_context(&models);
        // The model's parts and data.
        let bodies: Vec<(BodyId, Arc<SheetMetalData>)> = self.context.as_ref().map(|c| c.sheet_metal.iter().filter(|(_, d)| Some(d.model) == active).map(|(b, d)| (b.clone(), d.clone())).collect()).unwrap_or_default();
        self.ensure_flats(cx);
        let flat = bodies.first().and_then(|(b, _)| self.flat_cache.as_ref()?.parts.iter().find(|(id, _)| id == b).and_then(|(_, f)| f.as_ref().ok().cloned()));
        let color = bodies.first().and_then(|(b, _)| self.display.body(b)).map(|b| b.color).unwrap_or([0.6, 0.75, 0.85, 1.0]);

        // Sheet metal context: [model ▾] (os-table-selector-container: 4 px
        // padding; label 10 px + 4 px in; a 28 px select at most 220 px wide,
        // 10 px after the label).
        let view = cx.entity();
        let context_row = h_flex()
            .h(px(37.))
            .flex_none()
            .pt(px(4.))
            .items_start()
            .text_size(px(14.))
            .text_color(pal.text)
            .child(div().w(px(141.)).flex_none().pl(px(14.)).pt(px(2.)).whitespace_nowrap().child("Sheet metal context:"))
            .child(if models.is_empty() {
                div().pl(px(10.)).pt(px(5.)).child(if self.context.is_some() { "No sheet metal models." } else { "Fetching sheet metal models." }).into_any_element()
            } else {
                let ids: Vec<FeatureId> = models.iter().map(|(f, _)| *f).collect();
                let names: Vec<String> = models.iter().map(|(_, n)| n.clone()).collect();
                let current = ids.iter().position(|f| Some(*f) == active).unwrap_or(0);
                crate::os_ui::user_select_field("sm-context", names, current, move |i, _, cx| {
                    let f = ids[i];
                    view.update(cx, |this, cx| {
                        this.ui.sm_context = Some(f);
                        cx.notify();
                    })
                })
                .h(px(28.))
                .w(px(220.))
                .ml(px(10.))
                .into_any_element()
            });

        // The tables (data from the active model's first part).
        let tables = bodies.first().map(|(b, d)| {
            let flat_up = |i: usize| flat.as_ref().and_then(|f| f.bend_lines.iter().find(|l| l.bend == i)).map(|l| l.up);
            let _ = b;
            let studio = self.studio(cx);
            let order = |f: FeatureId| studio.and_then(|s| s.features.iter().position(|x| x.id == f)).unwrap_or(usize::MAX);
            joint_rows(d, &flat_up, &order)
        });
        let (bends, joints) = tables.unwrap_or_default();
        // Move up / Move down order (indices into `bends`).
        let row_order = match (bodies.first(), active) {
            (Some((_, d)), Some(m)) => bend_row_order(d, &self.sm_stored_order(m, cx)),
            _ => (0..bends.len()).collect(),
        };
        let units = self.doc.read(cx).session.doc.settings.units;
        let unit = units.length.symbol();
        let len = |x: f64| plain(x / units.si_factor(noshape_units::Dim::LENGTH));
        let grid = gpui_kit::rgb(0xaaaaaa);
        // An os-table cell: 34 px with the 1 px right / bottom grid lines;
        // `fill` its background, centred 14 px text. `at`: (table, row,
        // column) of a body cell, which reports the pointer over it.
        let hover = self.ui.sm_hover;
        let selected = self.ui.sm_cell;
        let hover_view = cx.entity();
        let cell = |w: f32, fill: Rgba, bold: bool, text: SharedString, at: Option<(u8, usize, usize)>| {
            let fill = match (at, hover) {
                // Live (5): the cell under the pointer, and its row header.
                (Some(a), Some(h)) if a == h && a.2 > 0 => gpui_kit::rgb(0xeeeeee),
                (Some(a), Some(h)) if a.0 == h.0 && a.1 == h.1 && a.2 == 0 && h.2 > 0 => gpui_kit::rgb(0xd7d7d7),
                _ => fill,
            };
            let v = hover_view.clone();
            div()
                .id(SharedString::from(at.map_or_else(|| format!("sm-cell-{text}"), |(t, r, c)| format!("sm-cell-{t}-{r}-{c}"))))
                .when_some(at, |d, a| {
                    d.on_hover(move |on: &bool, _, cx| {
                        let on = *on;
                        v.update(cx, |this, cx| {
                            let next = if on { Some(a) } else if this.ui.sm_hover == Some(a) { None } else { this.ui.sm_hover };
                            if next != this.ui.sm_hover {
                                this.ui.sm_hover = next;
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        })
                    })
                })
                .when_some(at.filter(|a| a.0 == 0), |d, a| {
                    let v = hover_view.clone();
                    d.on_mouse_down(MouseButton::Right, move |e, window, cx| {
                        cx.stop_propagation();
                        let at = e.position;
                        v.update(cx, |this, cx| this.open_bend_row_menu(a.1, at, window, cx))
                    })
                })
                .when_some(at, |d, a| {
                    let v = hover_view.clone();
                    d.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.ui.sm_cell = Some(a);
                            if a.0 == 0 && a.2 == 0 {
                                this.selection = this.sm_bend_refs(a.1);
                                this.refresh_scene(false, cx);
                            }
                            cx.notify();
                        })
                    })
                })
                // Live (sm-092, sm-094): a 2 px #1651b0 outline on the
                // selected cell, or round the row from its "#".
                .when(at.is_some_and(|a| selected == Some(a) && a.2 > 0), |d| d.border_2().border_color(gpui_kit::rgb(0x1651b0)))
                .w(px(w))
                .h(px(34.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .px(px(3.))
                .pb(px(1.))
                .border_r_1()
                .border_b_1()
                .border_color(grid)
                .bg(fill)
                .when(bold, |d| d.font_weight(FontWeight::BOLD))
                .child(div().truncate().child(text))
        };
        let head = pal.table_header;
        let readonly = gpui_kit::rgb(0xf5f5f5);
        let editable = pal.bg_senary;
        // A collapsible table: the os-table-description bar (#eaeaea, 16 px
        // / 500 title, the expanded / collapsed chevron) over the grid.
        let section = |id: &'static str, title: &'static str, open: bool, body: AnyElement, cx: &mut Context<Self>| {
            v_flex()
                .child(
                    h_flex()
                        .id(id)
                        .h(px(37.))
                        .flex_none()
                        .px(px(6.))
                        .items_center()
                        .border_t_1()
                        .border_l_1()
                        .border_b_1()
                        .border_color(grid)
                        .bg(head)
                        .cursor_pointer()
                        // os-icon-xsmall expanded / collapsed: a 10 x 5 px chevron.
                        .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 10., pal.text))
                        .child(div().pl(px(4.)).pt(px(2.)).text_size(px(16.)).font_weight(FontWeight::MEDIUM).child(title))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if id == "sm-bends-title" {
                                this.ui.sm_bends_closed = !this.ui.sm_bends_closed;
                            } else {
                                this.ui.sm_joints_closed = !this.ui.sm_joints_closed;
                            }
                            cx.notify();
                        })),
                )
                .when(open, |d| d.child(body))
        };
        // Bends: # (60, frozen look) | Name | Radius(unit) | Angle(deg) |
        // Bend direction | K Factor (140 each); Radius and K Factor are
        // editable (white).
        let bend_cols: [(&str, f32); 6] = [("#", 60.), ("Name", 140.), ("Radius", 140.), ("Angle(deg)", 140.), ("Bend direction", 140.), ("K Factor", 140.)];
        let col_head = |t: u8, c: usize| if hover.is_some_and(|h| h.0 == t && h.2 == c && c > 0) { gpui_kit::rgb(0xd7d7d7) } else { head };
        let bend_head = h_flex().border_l_1().border_color(grid).children(bend_cols.iter().enumerate().map(|(c, (n, w))| {
            let label: SharedString = if *n == "Radius" { format!("Radius({unit})").into() } else { (*n).into() };
            cell(*w, col_head(0, c), true, label, None)
        }));
        // Radius and K Factor: a double-click edits the value in place (live
        // (6): a 2 px #1651b0 outline, the number selected, no unit); Enter
        // or Tab applies it as a Modify joint feature, Esc cancels.
        let editing = self.ui.sm_edit;
        let mut edit_input: Option<(usize, usize, Entity<gpui_kit::component::input::InputState>)> = None;
        if let Some((row, col)) = editing
            && let Some(r) = bends.get(row)
        {
            let value = if col == 2 { len(r.radius) } else { plain(r.k_factor) };
            let st = self.cell_input(format!("sm-edit-{row}-{col}"), value, window, cx, move |this, v, cx| this.sm_apply_edit(row, col, v, cx));
            if self.ui.sm_edit_focus {
                self.ui.sm_edit_focus = false;
                st.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
            }
            edit_input = Some((row, col, st));
        }
        let edit_view = cx.entity();
        let edit_cell = |row: usize, col: usize, text: String| -> AnyElement {
            if let Some((_, _, st)) = edit_input.as_ref().filter(|(r, c, _)| (*r, *c) == (row, col)) {
                let v = edit_view.clone();
                return div()
                    .id(SharedString::from(format!("sm-edit-cell-{row}-{col}")))
                    .w(px(140.))
                    .h(px(34.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(2.))
                    .bg(editable)
                    .border_2()
                    .border_color(gpui_kit::rgb(0x1651b0))
                    .text_size(px(13.33))
                    .on_key_down(move |e: &KeyDownEvent, _, cx| {
                        if e.keystroke.key == "escape" {
                            cx.stop_propagation();
                            v.update(cx, |this, cx| {
                                this.ui.sm_edit = None;
                                cx.notify();
                            });
                        }
                    })
                    .child(gpui_kit::component::input::Input::new(st).appearance(false).text_size(px(13.33)).pl(px(1.)))
                    .into_any_element();
            }
            let v = edit_view.clone();
            cell(140., editable, false, text.into(), Some((0, row, col)))
                .on_click(move |e, _, cx| {
                    if e.click_count() >= 2 {
                        v.update(cx, |this, cx| {
                            this.ui.sm_edit = Some((row, col));
                            this.ui.sm_edit_focus = true;
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        };
        let bend_rows = row_order.iter().enumerate().filter_map(|(pos, &i)| bends.get(i).map(|r| (pos, i, r))).map(|(pos, i, r)| {
            let row_on = selected == Some((0, i, 0));
            h_flex()
                .relative()
                .border_l_1()
                .border_color(grid)
                .child(cell(60., head, true, (pos + 1).to_string().into(), Some((0, i, 0))))
                .child(cell(140., readonly, false, r.name.clone().into(), Some((0, i, 1))))
                .child(edit_cell(i, 2, len(r.radius)))
                .child(cell(140., readonly, false, plain(r.angle).into(), Some((0, i, 3))))
                .child(cell(140., readonly, false, r.up.map_or("", |u| if u { "Up" } else { "Down" }).into(), Some((0, i, 4))))
                .child(edit_cell(i, 5, plain(r.k_factor)))
                .when(row_on, |d| d.child(div().absolute().top(px(-1.)).left(px(-1.)).right_0().bottom_0().border_2().border_color(gpui_kit::rgb(0x1651b0))))
        });
        let scroll = self.ui.sm_bends_scroll.clone();
        // The table scrolls inside the panel's width (its columns are wider).
        let bends_table = v_flex()
            .w_full()
            .child(v_flex().id("sm-bends").w_full().overflow_x_scroll().track_scroll(&scroll).child(v_flex().flex_none().min_w(px(1. + bend_cols.iter().map(|c| c.1).sum::<f32>())).items_start().child(bend_head).children(bend_rows)))
            .children(h_scrollbar(&scroll, cx))
            .into_any_element();
        // Other joints: Name (frozen) | Type ▾ | Style ▾.
        let select_cell = |text: &'static str| {
            div()
                .w(px(140.))
                .h(px(34.))
                .flex_none()
                .flex()
                .items_center()
                .pl(px(8.))
                .pr(px(10.))
                .border_r_1()
                .border_b_1()
                .border_color(grid)
                .bg(editable)
                .child(div().flex_1().truncate().child(text))
                .child(crate::ui::glyph("ui-select-caret", 7., gpui_kit::rgb(0x333333)))
        };
        let joint_head = h_flex().border_l_1().border_color(grid).child(cell(140., head, true, "Name".into(), None)).child(cell(140., col_head(1, 1), true, "Type".into(), None)).child(cell(140., col_head(1, 2), true, "Style".into(), None));
        let joint_rows_el = joints.iter().enumerate().map(|(i, r)| h_flex().border_l_1().border_color(grid).child(cell(140., head, true, r.name.clone().into(), Some((1, i, 0)))).child(select_cell("Rip")).child(select_cell(r.style)));
        let joints_table = v_flex().items_start().child(joint_head).children(joint_rows_el).into_any_element();
        let tables_el = v_flex()
            .id("sm-tables")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .mt(px(2.))
            .text_size(px(14.))
            .text_color(pal.text)
            .when(!models.is_empty(), |d| {
                let (b_open, j_open) = (!self.ui.sm_bends_closed, !self.ui.sm_joints_closed);
                d.child(section("sm-bends-title", "Bends", b_open, bends_table, cx)).child(section("sm-joints-title", "Other joints", j_open, joints_table, cx))
            });

        // The flat view (its own graphics area).
        let fv = match self.flat_view.clone() {
            Some(fv) => fv,
            None => {
                let gpu = self.gpu.clone();
                let fv = cx.new(|cx| FlatView::new(gpu, window, cx));
                self._subs.push(cx.subscribe_in(&fv, window, |this, _, ev: &FlatViewEvent, window, cx| match ev {
                    FlatViewEvent::ContextMenu(at, FlatPick::Bend(b)) => this.open_flat_bend_menu(*at, *b, window, cx),
                    FlatViewEvent::ContextMenu(at, on) => this.open_flat_menu(*at, *on == FlatPick::Face, window, cx),
                    FlatViewEvent::Selected(side) => this.select_flat_face(*side, cx),
                }));
                self.flat_view = Some(fv.clone());
                fv
            }
        };
        let highlight = match self.ui.sm_cell {
            Some((0, row, 0)) => Some(row),
            _ => None,
        };
        // Live "ui (10-24 b)": the hovered row's bend zone outlined too.
        let hovered = match self.ui.sm_hover {
            Some((0, row, _)) => Some(row),
            _ => None,
        };
        // An open sketch on a face of the flat's part: the face's box on the
        // flat, with a margin, and the sketch's name at its top-left (live
        // request 1605, g-4).
        let sketch_frame = self.sketch.as_ref().zip(flat.as_ref()).zip(self.context.as_ref()).and_then(|((_, f), ctx)| {
            let feature = &self.edit.as_ref()?.dialog.read(cx).feature;
            let Some(noshape_model::params::ParamValue::Query(plane)) = feature.params.get("sketchPlane") else { return None };
            let faces: Vec<_> = plane.iter().flat_map(|r| noshape_model::regen::measure::resolve_shapes(ctx, r)).collect();
            let b = faces.first()?.bounding_box(false).ok()?;
            let on: Vec<glam::DVec3> = (0..8).map(|k| f.to_flat_view(glam::DVec3::new([b.min.x, b.max.x][k & 1], [b.min.y, b.max.y][(k >> 1) & 1], [b.min.z, b.max.z][(k >> 2) & 1]))).collect();
            // Only a face lying on the flat's sheet (its top or bottom).
            let on_sheet = |z: f64| z.abs() < 1e-6 || (z.abs() - f.thickness).abs() < 1e-6;
            if !on.iter().all(|c| on_sheet(c.z)) {
                return None;
            }
            let (lo, hi) = on.iter().fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), c| (lo.min(c.truncate()), hi.max(c.truncate())));
            let m = 0.06 * (hi - lo).max_element();
            let (lo, hi) = (lo - glam::DVec2::splat(m), hi + glam::DVec2::splat(m));
            let at = |x: f64, y: f64| glam::DVec3::new(x, y, 0.0);
            let asc = 0.045 * (hi.y - lo.y);
            Some(super::flat_view::SketchFrame { corners: [at(lo.x, lo.y), at(hi.x, lo.y), at(hi.x, hi.y), at(lo.x, hi.y)], name_at: at(lo.x + 0.3 * asc, hi.y - 0.3 * asc), x: glam::DVec3::X, y: glam::DVec3::Y, name: feature.name.clone() })
        });
        fv.update(cx, |v, cx| {
            v.show(flat.as_ref(), color, highlight, hovered, cx);
            v.show_sketch_frame(sketch_frame, cx);
        });

        let split = SPLIT.with(|s| s.get());
        v_flex()
            .id("sm-panel")
            .size_full()
            .on_drag_move(move |e: &DragMoveEvent<SmSplitDrag>, window, _| {
                let (f0, y0) = SPLIT_PRESS.with(|p| p.get());
                let total = f32::from(e.bounds.size.height).max(1.0);
                let h = (f0 * total + f32::from(e.event.position.y - y0)).round().clamp(SPLIT_MIN_TOP, (total - SPLIT_MIN_BOTTOM).max(SPLIT_MIN_TOP));
                let f = h / total;
                if SPLIT.with(|s| s.get()) != Some(f) {
                    SPLIT.with(|s| s.set(Some(f)));
                    let _ = std::fs::write(crate::paths::config_dir().join("sheet_metal_split.txt"), f.to_string());
                    window.refresh();
                }
            })
            .child(
                v_flex()
                    .h(relative(split.unwrap_or(0.5)))
                    .flex_none()
                    .min_h(px(SPLIT_MIN_TOP))
                    .overflow_hidden()
                    .child(context_row)
                    .child(tables_el),
            )
            // The splitter: a 1 px `#d7d7d7` line, row-resize.
            .child(
                div()
                    .id("sm-splitter")
                    .h(px(9.))
                    .mt(px(-5.))
                    .mb(px(-3.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .occlude()
                    .cursor(CursorStyle::ResizeRow)
                    .child(div().w_full().h(px(1.)).bg(pal.border))
                    .on_mouse_down(MouseButton::Left, |e, _, _| {
                        SPLIT_PRESS.with(|p| p.set((SPLIT.with(|s| s.get()).unwrap_or(0.5), e.position.y)));
                    })
                    .on_drag(SmSplitDrag, |d, _, _, cx| cx.new(|_| d.clone())),
            )
            // Live (4): the main window's Measure and Analysis tools sit at
            // the flat view's bottom right.
            .child(div().flex_1().min_h(px(SPLIT_MIN_BOTTOM)).relative().child(fv).child(h_flex().occlude().absolute().bottom_0().right_0().h(px(30.)).px(px(5.)).gap(px(5.)).children(self.eval_buttons("flat", false, cx))))
            .into_any_element()
    }
}

/// Chrome's classic horizontal scrollbar under a table wider than the
/// panel (live sm-091: a 15 px `#fcfcfc` track, ◂ / ▸ buttons, a 9 px
/// round `#8b8b8b` thumb); none when everything fits.
fn h_scrollbar(handle: &ScrollHandle, cx: &mut Context<PartStudioView>) -> Option<AnyElement> {
    let (view_w, max) = (f32::from(handle.bounds().size.width), f32::from(handle.max_offset().x));
    if max <= 0.5 || view_w <= 0.0 {
        // Laid out once the table has its size.
        if view_w <= 0.0 {
            cx.notify();
        }
        return None;
    }
    let off = -f32::from(handle.offset().x);
    let track = (view_w - 30.0 - 6.0).max(10.0);
    let thumb = (track * view_w / (view_w + max)).max(20.0);
    let x = 3.0 + (track - thumb) * (off / max).clamp(0.0, 1.0);
    // Live dk-973: a #9f9f9f thumb in dark mode.
    let ink = if gpui_kit::hex_theme_active() { gpui_kit::rgb_static(0x9f9f9f) } else { gpui_kit::rgb(0x8b8b8b) };
    let step = |id: &'static str, glyph: &'static str, d: f32| {
        let h = handle.clone();
        div().id(id).w(px(15.)).h_full().flex().items_center().justify_center().child(crate::ui::glyph(glyph, 9., ink)).on_click(move |_, window, _| {
            let o = h.offset();
            h.set_offset(point(o.x - px(d), o.y));
            window.refresh();
        })
    };
    let h = handle.clone();
    Some(
        h_flex()
            .h(px(15.))
            .bg(gpui_kit::rgb(0xfafafa))
            .border_l_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .child(step("sm-scroll-left", "ui-caret-left", -40.))
            .child(
                div()
                    .id("sm-scroll-track")
                    .flex_1()
                    .h_full()
                    .relative()
                    .child(div().absolute().left(px(x)).top(px(3.)).h(px(9.)).w(px(thumb)).rounded_full().bg(ink))
                    .on_mouse_down(MouseButton::Left, move |e, window, _| {
                        // A press on the track jumps the thumb's middle there.
                        let start = f32::from(e.position.x - h.bounds().origin.x) - 15.0;
                        let t = ((start - thumb / 2.0 - 3.0) / (track - thumb).max(1.0)).clamp(0.0, 1.0);
                        let o = h.offset();
                        h.set_offset(point(px(-t * max), o.y));
                        window.refresh();
                    }),
            )
            .child(step("sm-scroll-right", "ui-caret-right", 40.))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn table_numbers_drop_trailing_zeros() {
        assert_eq!(plain(2.0), "2");
        assert_eq!(plain(0.01), "0.01");
        assert_eq!(plain(0.45), "0.45");
        assert_eq!(plain(180.0), "180");
        assert_eq!(plain(-0.0), "0");
    }
}
