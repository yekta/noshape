//! Edit in context (assembly instance menu, live assembly/115–117): the
//! Part Studio shows the assembly around the edited part as translucent
//! context graphics, under a gray-blue banner "Assembly 1 displayed"
//! [Go to assembly] [Hide/show instances] ✕ with Show internal geometry,
//! Select transparent geometry and a Context transparency slider. The
//! context is transient: going back to the assembly drops it.

use super::*;
use noshape_model::assembly::InstanceId;
use noshape_model::display::MeshBuffers;
use noshape_render::{Drawable, LineData, MeshData};
use gpui_kit::component::IconName;
use std::collections::HashSet;
use std::rc::Rc;

/// One instance of the assembly, in the Part Studio's coordinates.
#[derive(Clone)]
pub struct ContextOccurrence {
    pub top: InstanceId,
    pub name: String,
    pub mesh: Arc<MeshBuffers>,
    pub transform: glam::DMat4,
    pub color: [f32; 4],
}

/// The assembly shown around the edited part.
#[derive(Clone)]
pub struct AssemblyContext {
    pub assembly: noshape_model::document::TabId,
    pub assembly_name: String,
    /// The part being edited (selected on entry).
    pub part: Option<BodyId>,
    pub occurrences: Vec<ContextOccurrence>,
    /// Create Part Studio in context: where the new Part Studio's parts go
    /// in the assembly ("Insert and go to assembly").
    pub insert_at: Option<noshape_model::assembly::Placement>,
    /// The assembly's camera in the Part Studio's coordinates (Onshape
    /// keeps the view orientation and zoom).
    pub camera: Option<noshape_render::Camera>,
    /// The Part Studio's Assembly context (snapshot) shown: its bodies come
    /// from the regeneration (`StudioDisplay::context_bodies`), pickable
    /// for references; `None`: display-only `occurrences`.
    pub context: Option<u32>,
}

/// Uploaded context graphics and the banner's settings.
pub(super) struct InContextUi {
    pub ctx: AssemblyContext,
    gpu: Vec<(usize, noshape_render::MeshHandle, noshape_render::LinesHandle)>,
    pub hidden: HashSet<InstanceId>,
    pub show_internal: bool,
    pub select_transparent: bool,
    /// Context transparency slider position, 0..1.
    pub slider: f32,
    pub hide_show_open: bool,
    slider_bounds: Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// Top-level instance of each context body (in snapshot order).
    body_tops: Vec<InstanceId>,
    /// The amber banner's split button shows Go to assembly (last used).
    pub split_go_to_assembly: bool,
    /// Insert and go to assembly's "Select items to insert" dialog is open.
    pub insert_go: bool,
}

/// A Part Studio menu action.
type StudioAct = Rc<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

/// A picked context face's fill and a context reference's outline (live
/// assembly/504, 506).
const CONTEXT_REF_FILL: u32 = 0xc573e4;
const CONTEXT_REF_OUTLINE: [u8; 4] = crate::theme::rgb8(0xdc62f7);
/// A selected context face outside a dialog: amber at 80 % over the
/// see-through face (blended in linear light; fitted to live assembly/706–707).
const CONTEXT_SELECTED_FILL: u32 = 0xe9b94d;
const CONTEXT_SELECTED_ALPHA: f32 = 0.8;

/// The boundary polylines of face `fi`: the mesh edges whose points all
/// lie on the face's triangles.
fn face_boundary(m: &MeshBuffers, fi: usize) -> Vec<&[[f32; 3]]> {
    let Some(&(first, count)) = m.face_ranges.get(fi) else { return Vec::new() };
    let q = |p: &[f32; 3]| [(p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64, (p[2] * 1e5).round() as i64];
    let verts: HashSet<[i64; 3]> = m.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]).iter().filter_map(|&i| m.positions.get(i as usize)).map(q).collect();
    m.edges.iter().filter(|pl| !pl.is_empty() && pl.iter().all(|p| verts.contains(&q(p)))).map(|pl| pl.as_slice()).collect()
}

/// Default slider position (live: the context draws at about 20 % opacity).
const DEFAULT_SLIDER: f32 = 0.29;

/// Events the Part Studio sends to the workspace.
pub enum PartStudioEvent {
    /// "Go to assembly" in the in-context banner.
    ActivateTab(noshape_model::document::TabId),
    /// "Insert and go to assembly" (Part Studio created in context).
    InsertInAssembly { assembly: noshape_model::document::TabId, studio: noshape_model::document::TabId, parts: Vec<BodyId>, sketches: Vec<FeatureId>, at: noshape_model::assembly::Placement },
}

impl EventEmitter<PartStudioEvent> for PartStudioView {}

impl PartStudioView {
    /// Enter the assembly context (from the assembly's Edit in context).
    pub fn set_assembly_context(&mut self, ctx: Option<AssemblyContext>, cx: &mut Context<Self>) {
        self.ui.in_context = ctx.map(|ctx| {
            let up = self.gpu.uploader();
            let gpu = ctx
                .occurrences
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let m = &o.mesh;
                    let mesh = up.upload_mesh(&MeshData { positions: m.positions.clone(), normals: m.normals.clone(), pick_ids: vec![0; m.positions.len()], indices: m.indices.clone() });
                    let mut lines = LineData::default();
                    for pl in &m.edges {
                        lines.push_polyline(pl, [0, 0, 0, 255], 0, 1.0, 0);
                    }
                    (i, mesh, up.upload_lines(&lines))
                })
                .collect();
            if let Some(b) = &ctx.part {
                self.selection = vec![Reference::Body(b.clone())];
            }
            if let Some(cam) = ctx.camera {
                self.fitted_once = true;
                self.viewport.update(cx, |vp, cx| {
                    vp.camera = cam;
                    vp.request_redraw(cx);
                });
            }
            let body_tops = ctx
                .context
                .and_then(|id| self.studio(cx).and_then(|s| s.contexts.iter().find(|c| c.id == id)).map(|c| c.bodies.iter().map(|b| InstanceId(b.path.first().copied().unwrap_or(0))).collect()))
                .unwrap_or_default();
            InContextUi { ctx, gpu, hidden: HashSet::new(), show_internal: false, select_transparent: true, slider: DEFAULT_SLIDER, hide_show_open: false, slider_bounds: Default::default(), body_tops, split_go_to_assembly: false, insert_go: false }
        });
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Context references in magenta (live assembly/504–506): a context
    /// face picked in a feature dialog is filled magenta with the amber
    /// selection outline; hovering a feature's context arrow outlines the
    /// context entities it references.
    pub(super) fn context_ref_overlay(&self, selection: &[Reference], cx: &App) -> Vec<noshape_render::Instance> {
        let bodies = &self.display.context_bodies;
        if bodies.is_empty() {
            return Vec::new();
        }
        let face = |r: &Reference| bodies.iter().filter(|b| !b.hidden).find_map(|b| b.face_refs.iter().position(|f| f == r).map(|i| (b, i)));
        let mut fills: Vec<MeshData> = Vec::new();
        let mut lines = LineData::default();
        let outline_face = |b: &noshape_model::display::BodyDisplay, fi: usize, color: [u8; 4], lines: &mut LineData| {
            for pl in face_boundary(&b.mesh, fi) {
                lines.push_polyline(pl, color, 0, 2.0, noshape_render::LineSegment::ON_TOP);
            }
        };
        if self.edit.is_some() {
            for r in selection {
                let Some((b, fi)) = face(r) else { continue };
                let m = &b.mesh;
                let Some(&(first, count)) = m.face_ranges.get(fi) else { continue };
                let indices = m.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]).to_vec();
                fills.push(MeshData { positions: m.positions.clone(), normals: m.normals.clone(), pick_ids: vec![0; m.positions.len()], indices });
                outline_face(b, fi, crate::theme::rgb8(crate::theme::viewport::SELECTION), &mut lines);
            }
        }
        // The see-through context faces don't show the usual hover tint: a
        // hovered one gets the preselection outline (`#ffc685`, live
        // assembly/503, 703); a selected one (outside a dialog) a translucent
        // amber fill and the selection outline (live 706–707).
        let mut selected: Vec<MeshData> = Vec::new();
        if self.edit.is_none() {
            for r in selection {
                let Some((b, fi)) = face(r) else { continue };
                let m = &b.mesh;
                if let Some(&(first, count)) = m.face_ranges.get(fi) {
                    let indices = m.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]).to_vec();
                    selected.push(MeshData { positions: m.positions.clone(), normals: m.normals.clone(), pick_ids: vec![0; m.positions.len()], indices });
                }
                outline_face(b, fi, crate::theme::rgb8(crate::theme::viewport::SELECTION), &mut lines);
            }
        }
        if let Some((b, fi)) = self.hover.as_ref().filter(|r| !selection.contains(r)).and_then(&face) {
            outline_face(b, fi, crate::theme::rgb8(crate::theme::viewport::PREHIGHLIGHT), &mut lines);
        }
        if let Some(f) = self.ui.context_arrow_hover.and_then(|id| self.studio(cx)?.features.iter().find(|f| f.id == id)) {
            for r in noshape_model::in_context::context_refs(&f.params) {
                if let Some((b, fi)) = face(&r) {
                    outline_face(b, fi, CONTEXT_REF_OUTLINE, &mut lines);
                } else if let Some((b, ei)) = bodies.iter().filter(|b| !b.hidden).find_map(|b| b.edge_refs.iter().position(|e| *e == r).map(|i| (b, i)))
                    && let Some(pl) = b.mesh.edges.get(ei)
                {
                    lines.push_polyline(pl, CONTEXT_REF_OUTLINE, 0, 2.0, noshape_render::LineSegment::ON_TOP);
                }
            }
        }
        let up = self.gpu.uploader();
        let mut out: Vec<noshape_render::Instance> = fills.iter().map(|m| noshape_render::Instance::new(Drawable::Mesh(up.upload_mesh(m))).with_color(crate::theme::lin(CONTEXT_REF_FILL)).unlit().unpickable().layer(8)).collect();
        let [r, g, b, _] = crate::theme::lin(CONTEXT_SELECTED_FILL);
        out.extend(selected.iter().map(|m| noshape_render::Instance::new(Drawable::Mesh(up.upload_mesh(m))).with_color([r, g, b, CONTEXT_SELECTED_ALPHA]).unlit().unpickable().layer(8)));
        if !lines.segments.is_empty() {
            out.push(noshape_render::Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(12));
        }
        out
    }

    /// What the measure readout measures: the regenerated model, plus the
    /// shown Assembly context's bodies when a context entity is selected
    /// (live assembly/706–707: "Area", "Perp planes angle" of ghost faces).
    pub(super) fn measure_context(&self, selection: &[Reference], cx: &App) -> Option<Arc<noshape_model::regen::context::Context>> {
        let base = self.context.clone()?;
        let ghost = |r: &Reference| self.display.context_bodies.iter().any(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r));
        if !selection.iter().any(ghost) {
            return Some(base);
        }
        let studio = self.studio(cx)?;
        let c = studio.contexts.iter().find(|c| Some(c.id) == studio.active_context)?;
        let mut ctx = (*base).clone();
        ctx.bodies.extend(c.regen_bodies());
        Some(Arc::new(ctx))
    }

    /// Context graphics: the other instances, translucent.
    pub(super) fn context_overlay(&self) -> Vec<noshape_render::Instance> {
        let Some(c) = self.ui.in_context.as_ref().filter(|c| c.ctx.context.is_none()) else { return Vec::new() };
        let alpha = c.slider * 0.69;
        let mut out = Vec::new();
        for (i, mesh, lines) in &c.gpu {
            let o = &c.ctx.occurrences[*i];
            if c.hidden.contains(&o.top) {
                continue;
            }
            let t = o.transform.as_mat4();
            let color = [o.color[0], o.color[1], o.color[2], alpha];
            out.push(noshape_render::Instance::new(Drawable::Mesh(mesh.clone())).with_color(color).with_transform(t).unpickable());
            out.push(noshape_render::Instance::new(Drawable::Lines(lines.clone())).with_color([0.0, 0.0, 0.0, alpha]).with_transform(t).unpickable().layer(1));
        }
        out
    }

    /// The scene's display: the model plus the shown Assembly context's
    /// bodies at the banner's transparency, without hidden instances.
    pub(super) fn display_for_scene(&self) -> Arc<StudioDisplay> {
        let d = self.display_for_scene_inner();
        if d.context_bodies.is_empty() {
            return d;
        }
        let (alpha, hidden, tops) = match &self.ui.in_context {
            Some(c) => (c.slider * 0.69, c.hidden.clone(), c.body_tops.clone()),
            None => (DEFAULT_SLIDER * 0.69, HashSet::new(), Vec::new()),
        };
        let mut out = (*d).clone();
        for (i, b) in d.context_bodies.iter().enumerate() {
            if b.hidden || tops.get(i).is_some_and(|t| hidden.contains(t)) {
                continue;
            }
            let mut b = b.clone();
            b.color[3] = alpha.max(0.05);
            out.bodies.push(b);
        }
        Arc::new(out)
    }

    /// Leave the context (✕ "Exit context", or going back to the assembly).
    /// The Assembly context stays in the Part Studio only while a feature
    /// references it (Onshape: "An entity must be referenced for the
    /// assembly context to persist").
    pub(super) fn exit_context(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.ui.in_context.as_ref().and_then(|c| c.ctx.context) {
            let tab = self.tab;
            self.doc.update(cx, |d, cx| {
                d.edit_document_not_undoable("Exit context", cx, |doc| {
                    if let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                        noshape_ops::in_context::exit(ps, id);
                    }
                })
            });
            // The Part Studio regenerates on its own tab's change event.
            self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(tab)));
        }
        self.ui.in_context = None;
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The gray-blue banner at the top centre (`#d0dbe5`, 490 px).
    pub(super) fn render_context_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = self.ui.in_context.as_ref()?;
        let small_button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .h(px(22.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(3.5))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(rgb(0xffffff))
                .text_size(px(13.))
                .text_color(rgb(0x333333))
                .cursor_pointer()
                .child(label)
        };
        let check = |id: &'static str, label: &'static str, on: bool| {
            h_flex()
                .id(id)
                .h(px(22.))
                .pl(px(14.))
                .gap(px(4.))
                .cursor_pointer()
                .child(
                    div()
                        .size(px(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .border_1()
                        .border_color(if on { rgb(0x1651b0) } else { rgb(0x888888) })
                        .bg(if on { rgb(0x1651b0) } else { rgb(0xffffff) })
                        .when(on, |d| d.child(Icon::new(IconName::Check).size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)))),
                )
                .child(div().text_size(px(13.33)).text_color(rgb(0x666666)).child(label))
        };
        let assembly = c.ctx.assembly;
        let slider = c.slider;
        let probe = c.slider_bounds.clone();
        let track_bounds = c.slider_bounds.clone();
        let set_slider = move |this: &mut Self, x: Pixels, cx: &mut Context<Self>| {
            let b = track_bounds.get();
            let w = f32::from(b.size.width).max(1.0);
            if let Some(c) = &mut this.ui.in_context {
                c.slider = (f32::from(x - b.origin.x) / w).clamp(0.0, 1.0);
            }
            this.refresh_scene(false, cx);
            cx.notify();
        };
        let set2 = set_slider.clone();
        // Once a feature references the context: amber "Context 1 of
        // <assembly>" with [Hide/show instances | ▾] [Done] (live 504, 509).
        let amber = self.shown_context_referenced(cx);
        let split_label = c.split_go_to_assembly;
        let mut banner = v_flex()
            .w(px(if amber.is_some() { 420. } else { 490. }))
            .bg(if amber.is_some() { rgb(0xfab601) } else { rgb(0xd0dbe5) })
            .child(
                h_flex()
                    .h(px(28.))
                    .pl(px(10.))
                    .pr(px(3.))
                    .pt(px(3.))
                    .child(div().text_size(px(13.)).text_color(rgb(0x333333)).child(match &amber {
                        Some((_, name)) => format!("{name} of {}", c.ctx.assembly_name),
                        None => format!("{} displayed", c.ctx.assembly_name),
                    }))
                    .child(div().flex_1())
                    .when_some(amber.clone(), |d, _| {
                        let main = if split_label { "Go to assembly" } else { "Hide/show instances" };
                        d.child(
                            h_flex()
                                .child(small_button("ctx-split-main", main).rounded_r(px(0.)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if split_label {
                                        let assembly = this.ui.in_context.as_ref().map(|c| c.ctx.assembly);
                                        this.exit_context(cx);
                                        if let Some(a) = assembly {
                                            cx.emit(PartStudioEvent::ActivateTab(a));
                                        }
                                    } else if let Some(c) = &mut this.ui.in_context {
                                        c.hide_show_open = !c.hide_show_open;
                                        cx.notify();
                                    }
                                })))
                                .child(small_button("ctx-split-caret", "").w(px(20.)).px(px(0.)).justify_center().rounded_l(px(0.)).border_l_0().child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x333333))).on_click(cx.listener(|_this, ev: &ClickEvent, window, cx| {
                                    let view = cx.entity();
                                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                        let (v1, v2) = (view.clone(), view.clone());
                                        m.list_no_icons()
                                            .item(crate::os_menu::OsMenuItem::new("Go to assembly").on_click(move |_, _, cx| {
                                                v1.update(cx, |this, cx| {
                                                    let assembly = this.ui.in_context.as_ref().map(|c| c.ctx.assembly);
                                                    if let Some(c) = &mut this.ui.in_context {
                                                        c.split_go_to_assembly = true;
                                                    }
                                                    this.exit_context(cx);
                                                    if let Some(a) = assembly {
                                                        cx.emit(PartStudioEvent::ActivateTab(a));
                                                    }
                                                })
                                            }))
                                            .item(crate::os_menu::OsMenuItem::new("Hide/show instances").on_click(move |_, _, cx| {
                                                v2.update(cx, |this, cx| {
                                                    if let Some(c) = &mut this.ui.in_context {
                                                        c.split_go_to_assembly = false;
                                                        c.hide_show_open = !c.hide_show_open;
                                                    }
                                                    cx.notify();
                                                })
                                            }))
                                    });
                                    crate::os_menu::open_anchored(ev.position(), Anchor::TopLeft, menu, window, cx);
                                }))),
                        )
                        .child(div().w(px(10.)))
                        // Done: leave the context, stay in the Part Studio (it persists).
                        .child(small_button("ctx-done", "Done").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.exit_context(cx))))
                        .child(div().w(px(10.)))
                    })
                    .when(amber.is_none(), |d| d.child(match c.ctx.insert_at {
                        // A Part Studio created in context (live assembly/551):
                        // [Insert and go to assembly][▾] (Insert and go to
                        // assembly, Go to assembly, Hide/show instances) and ✕.
                        Some(_) => h_flex()
                            .child(small_button("ctx-insert-go", "Insert and go to assembly").rounded_r(px(0.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_insert_go(cx))))
                            .child(small_button("ctx-insert-caret", "").w(px(20.)).px(px(0.)).justify_center().rounded_l(px(0.)).border_l_0().child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x333333))).on_click(cx.listener(move |_this, ev: &ClickEvent, window, cx| {
                                let view = cx.entity();
                                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                    let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                                    m.list_no_icons()
                                        .item(crate::os_menu::OsMenuItem::new("Insert and go to assembly").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.open_insert_go(cx))))
                                        .item(crate::os_menu::OsMenuItem::new("Go to assembly").on_click(move |_, _, cx| {
                                            v2.update(cx, |this, cx| {
                                                this.exit_context(cx);
                                                cx.emit(PartStudioEvent::ActivateTab(assembly));
                                            })
                                        }))
                                        .item(crate::os_menu::OsMenuItem::new("Hide/show instances").on_click(move |_, _, cx| {
                                            v3.update(cx, |this, cx| {
                                                if let Some(c) = &mut this.ui.in_context {
                                                    c.hide_show_open = !c.hide_show_open;
                                                }
                                                cx.notify();
                                            })
                                        }))
                                });
                                crate::os_menu::open_anchored(ev.position(), Anchor::TopLeft, menu, window, cx);
                            })))
                            .into_any_element(),
                        None => h_flex()
                            .child(small_button("ctx-go", "Go to assembly").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.exit_context(cx);
                                cx.emit(PartStudioEvent::ActivateTab(assembly));
                            })))
                            .child(div().w(px(10.)))
                            .child(small_button("ctx-hide-show", "Hide/show instances").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(c) = &mut this.ui.in_context {
                                    c.hide_show_open = !c.hide_show_open;
                                }
                                cx.notify();
                            })))
                            .into_any_element(),
                    }))
                    .when(amber.is_none(), |d| d
                    .child(
                        div()
                            .id("ctx-exit")
                            .ml(px(7.5))
                            .mr(px(4.5))
                            .size(px(20.))
                            .opacity(0.5)
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip("Exit context"))
                            // Bootstrap `btn-close` (live 551: an 11 px ✕).
                            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 20., rgb(0x000000)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.exit_context(cx))),
                    )),
            )
            .child(
                h_flex()
                    .h(px(22.))
                    .child(check("ctx-internal", "Show internal geometry", c.show_internal).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(c) = &mut this.ui.in_context {
                            c.show_internal = !c.show_internal;
                        }
                        cx.notify();
                    })))
                    .child(div().w(px(20.)))
                    .child(check("ctx-select", "Select transparent geometry", c.select_transparent).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(c) = &mut this.ui.in_context {
                            c.select_transparent = !c.select_transparent;
                        }
                        cx.notify();
                    }))),
            )
            .child(
                // Context transparency slider (noUi: 10 px track, 11 px round handle).
                div()
                    .id("ctx-slider")
                    .h(px(25.))
                    .px(px(11.))
                    .pt(px(4.))
                    .tooltip(crate::ui::tooltip("Context transparency"))
                    .child(
                        div()
                            .relative()
                            .h(px(10.))
                            .w_full()
                            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                            .child(div().absolute().top(px(4.)).left_0().right_0().h(px(1.)).bg(rgb(0x888888)))
                            .child(
                                div()
                                    .absolute()
                                    .top(px(1.))
                                    .left(relative(slider))
                                    .ml(px(-5.5))
                                    .size(px(11.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(0x888888))
                                    .bg(rgb(0xffffff))
                                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }]),
                            )
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| set_slider(this, ev.position.x, cx)))
                            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                                if ev.pressed_button == Some(MouseButton::Left) && !cx.has_active_drag() {
                                    set2(this, ev.position.x, cx);
                                }
                            })),
                    ),
            );
        // Hide/show instances: the context's instances with their eyes.
        if c.hide_show_open {
            let mut list = v_flex().bg(rgb(0xffffff)).border_t_1().border_color(rgb(0xcccccc)).py(px(2.));
            for o in &c.ctx.occurrences {
                if list_has(&c.ctx.occurrences, o) {
                    continue;
                }
                let top = o.top;
                let hidden = c.hidden.contains(&top);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("ctx-inst-{}", top.0)))
                        .h(px(24.))
                        .px(px(10.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xf0f0f0)))
                        .child(crate::ui::glyph(if hidden { "eye-off" } else { "eye" }, 14., rgb(0x666666)))
                        .child(div().text_size(px(13.)).text_color(if hidden { rgb(0x999999) } else { rgb(0x333333) }).child(crate::ui::user_text(o.name.clone())))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(c) = &mut this.ui.in_context
                                && !c.hidden.remove(&top)
                            {
                                c.hidden.insert(top);
                            }
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })),
                );
            }
            banner = banner.child(list);
        }
        Some(div().absolute().top(px(0.)).left_0().right_0().flex().justify_center().child(banner).into_any_element())
    }
}

impl PartStudioView {
    /// Insert and go to assembly (live assembly/545): the "Select items to
    /// insert" dialog, its field empty (no preselection).
    pub(super) fn open_insert_go(&mut self, cx: &mut Context<Self>) {
        let Some(c) = &mut self.ui.in_context else { return };
        c.insert_go = true;
        self.selection.clear();
        self.selected_features.clear();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn insert_go_open(&self) -> bool {
        self.ui.in_context.as_ref().is_some_and(|c| c.insert_go)
    }

    /// What a pick adds to "Select parts/surfaces/sketches": the part of a
    /// face, edge or vertex, the sketch of a curve or region.
    pub(super) fn insert_go_item(&self, r: &Reference) -> Option<Reference> {
        match r {
            Reference::Body(_) | Reference::Sketch(_) => Some(r.clone()),
            Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_) => self.body_of(r).map(Reference::Body),
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } => Some(Reference::Sketch(*sketch)),
            _ => None,
        }
    }

    /// A graphics or list pick while the dialog is open toggles its item;
    /// `false` when the dialog isn't open.
    pub(super) fn insert_go_pick(&mut self, r: &Reference, cx: &mut Context<Self>) -> bool {
        if !self.insert_go_open() {
            return false;
        }
        match self.insert_go_item(r) {
            Some(item) => match self.selection.iter().position(|x| *x == item) {
                Some(i) => {
                    self.selection.remove(i);
                }
                None => self.selection.push(item),
            },
            None => self.flash_info("Select parts/surfaces/sketches".into(), cx),
        }
        self.refresh_scene(false, cx);
        cx.notify();
        true
    }

    /// The dialog's items: picked parts, surfaces and sketches.
    fn insert_go_items(&self) -> Vec<Reference> {
        let mut out: Vec<Reference> = Vec::new();
        for r in self.selection.iter().filter_map(|r| self.insert_go_item(r)).chain(self.selected_features.iter().filter(|f| self.display.sketches.iter().any(|s| s.feature == **f)).map(|f| Reference::Sketch(*f))) {
            if !out.contains(&r) {
                out.push(r);
            }
        }
        out
    }

    /// Esc / ✕: close the dialog (stays in the Part Studio).
    pub(super) fn close_insert_go(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(c) = self.ui.in_context.as_mut().filter(|c| c.insert_go) else { return false };
        c.insert_go = false;
        self.selection.clear();
        self.refresh_scene(false, cx);
        cx.notify();
        true
    }

    /// ✓: nothing picked closes the dialog and stays (live 546); otherwise
    /// the items go into the assembly at the Part Studio's origin and the
    /// assembly opens (live 547–550).
    fn accept_insert_go(&mut self, cx: &mut Context<Self>) {
        let items = self.insert_go_items();
        let target = self.ui.in_context.as_ref().and_then(|c| c.ctx.insert_at.map(|at| (c.ctx.assembly, at)));
        self.close_insert_go(cx);
        let Some((assembly, at)) = target.filter(|_| !items.is_empty()) else { return };
        let parts = items.iter().filter_map(|r| if let Reference::Body(b) = r { Some(b.clone()) } else { None }).collect();
        let sketches = items.iter().filter_map(|r| if let Reference::Sketch(f) = r { Some(*f) } else { None }).collect();
        let studio = self.tab;
        self.exit_context(cx);
        cx.emit(PartStudioEvent::InsertInAssembly { assembly, studio, parts, sketches, at });
    }

    /// The 220 px feature-style dialog "Select items to insert" (live
    /// dom/assembly-insert-and-go-dialog.txt): ✓ ✕, the blue query field
    /// "Select parts/surfaces/sketches" with one chip per item, `?`.
    pub(super) fn render_insert_go(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.insert_go_open() {
            return None;
        }
        let pal = p();
        let items = self.insert_go_items();
        let name = |r: &Reference| match r {
            Reference::Body(b) => self.display.body(b).map(|b| b.name.clone()).unwrap_or_default(),
            Reference::Sketch(f) => self.studio(cx).and_then(|s| s.features.iter().find(|x| x.id == *f).map(|x| x.name.clone())).unwrap_or_default(),
            _ => String::new(),
        };
        let mut field = v_flex()
            .rounded(px(2.))
            .border_1()
            .border_color(rgb(0x1651b0))
            .bg(rgb(0xdef1fb))
            .text_color(rgb(0x555555));
        if items.is_empty() {
            field = field.child(div().h(px(24.)).px(px(6.)).flex().items_center().text_size(px(12.)).child("Select parts/surfaces/sketches"));
        } else {
            field = field.child(div().px(px(6.)).pt(px(5.)).text_size(px(10.)).child("Select parts/surfaces/sketches"));
            for (i, r) in items.into_iter().enumerate() {
                let label = name(&r);
                field = field.child(
                    h_flex()
                        .h(px(24.))
                        .pl(px(6.))
                        .pr(px(5.))
                        .text_size(px(13.33))
                        .child(div().flex_1().truncate().child(label.clone()))
                        .child(
                            div()
                                .id(SharedString::from(format!("insert-go-del-{i}")))
                                .px(px(5.))
                                .text_size(px(16.67))
                                .text_color(rgb(0x666666))
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip(format!("Delete {label}")))
                                .child("×")
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let keep: Vec<Reference> = this.selection.iter().filter(|x| this.insert_go_item(x).as_ref() != Some(&r)).cloned().collect();
                                    this.selection = keep;
                                    if let Reference::Sketch(f) = &r {
                                        this.selected_features.retain(|x| x != f);
                                    }
                                    this.refresh_scene(false, cx);
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        let help = div().size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0x888888)));
        Some(
            v_flex()
                .id("insert-go")
                .occlude()
                .absolute()
                .top(px(3.))
                .left_0()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child("Select items to insert"))
                        .child(
                            div()
                                .id("insert-go-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_insert_go(cx))),
                        )
                        .child(
                            div()
                                .id("insert-go-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_insert_go(cx);
                                })),
                        ),
                )
                .child(div().px(px(3.)).pt(px(5.)).child(field))
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(help))
                .into_any_element(),
        )
    }
}

/// Whether an earlier occurrence already lists the same instance (one row
/// per instance in Hide/show instances).
fn list_has(all: &[ContextOccurrence], o: &ContextOccurrence) -> bool {
    all.iter().position(|x| x.top == o.top).is_some_and(|i| !std::ptr::eq(&all[i], o))
}

/// Onshape's in-context arrow ⇦ (20 px): `solid` = yellow fill (the active
/// context's features), else an outline; `dashed` = an instance that is not
/// the context's primary instance (live 505, 517, 520).
pub fn context_arrow_icon(solid: bool, dashed: bool) -> AnyElement {
    let ink = rgb(0x333333);
    div()
        .relative()
        .size(px(20.))
        .flex_none()
        .when(solid, |d| d.child(div().absolute().top_0().left_0().child(crate::ui::glyph("incontext-arrow-fill", 20., rgb(0xfab601)))))
        .child(div().absolute().top_0().left_0().child(crate::ui::glyph(if dashed { "incontext-arrow-dashed" } else { "incontext-arrow-outline" }, 20., ink)))
        .into_any_element()
}

impl PartStudioView {
    /// The arrow after a feature that references an Assembly context,
    /// tooltip the context's name; yellow while that context is shown.
    pub(super) fn context_arrow(&self, f: &noshape_model::partstudio::Feature, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = *noshape_model::in_context::referenced_contexts(std::slice::from_ref(f)).first()?;
        let studio = self.studio(cx)?;
        let name = studio.contexts.iter().find(|c| c.id == id)?.name.clone();
        let active = studio.active_context == Some(id);
        let fid = f.id;
        let arrow = div().id(SharedString::from(format!("feat-ctx-{}", f.id))).flex_none().mr(px(6.)).tooltip(crate::ui::tooltip(name)).child(context_arrow_icon(active, false));
        Some(arrow.on_hover(cx.listener(move |this, on: &bool, _, cx| {
            let next = on.then_some(fid);
            if this.ui.context_arrow_hover != next {
                this.ui.context_arrow_hover = next;
                this.refresh_scene(false, cx);
                cx.notify();
            }
        })).into_any_element())
    }

    fn assembly_name(&self, assembly: noshape_model::document::TabId, cx: &App) -> String {
        self.doc.read(cx).session.doc.tab(assembly).map(|t| t.name.clone()).unwrap_or_default()
    }

    /// Show an Assembly context (dropdown pick) or none.
    pub(super) fn activate_context(&mut self, id: Option<u32>, cx: &mut Context<Self>) {
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_document_not_undoable("Assembly context", cx, |doc| {
                if let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                    ps.active_context = id;
                }
            })
        });
        // The Part Studio regenerates on its own tab's change event.
        self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(tab)));
        let ctx = id.and_then(|id| {
            let assembly = self.studio(cx)?.contexts.iter().find(|c| c.id == id)?.assembly;
            let assembly_name = self.assembly_name(assembly, cx);
            Some(AssemblyContext { assembly, assembly_name, part: None, occurrences: Vec::new(), insert_at: None, camera: None, context: Some(id) })
        });
        self.ui.in_context = None;
        if ctx.is_some() {
            self.set_assembly_context(ctx, cx);
        } else {
            self.refresh_scene(false, cx);
            cx.notify();
        }
    }

    /// ⋯ › Update context: a new snapshot of the assembly as it is now,
    /// around the context's primary instance (features regenerate).
    pub(super) fn update_context_here(&mut self, id: u32, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx).session.doc.clone();
        let Some(old) = self.studio(cx).and_then(|s| s.contexts.iter().find(|c| c.id == id).cloned()) else { return };
        let Some(noshape_model::TabContent::Assembly(asm)) = doc.tab(old.assembly).map(|t| &t.content) else { return };
        let mut ev = crate::derived::assembly_evaluator(self.doc.read(cx).studio_cache.clone());
        let result = ev.evaluate(&doc, asm, &noshape_model::assembly::eval::SolveOptions::default(), None);
        let Some(fresh) = noshape_ops::in_context::refreshed(&old, &result, asm, None) else { return };
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_document(&noshape_ops::in_context::update_description(&old.name), cx, |doc| {
                if let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab_mut(tab).map(|t| &mut t.content)
                    && let Some(c) = ps.contexts.iter_mut().find(|c| c.id == id)
                {
                    *c = fresh;
                }
            })
        });
        cx.default_global::<crate::assembly_view::StaleContexts>().0.remove(&(tab, id));
        self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(tab)));
    }

    fn rename_context(&mut self, id: u32, name: String, cx: &mut Context<Self>) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_document("Rename context", cx, |doc| {
                if let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab_mut(tab).map(|t| &mut t.content)
                    && let Some(c) = ps.contexts.iter_mut().find(|c| c.id == id)
                {
                    c.name = name;
                }
            })
        });
        // The Part Studio regenerates on its own tab's change event.
        self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(tab)));
    }

    fn delete_context(&mut self, id: u32, cx: &mut Context<Self>) {
        let tab = self.tab;
        self.ui.in_context = None;
        self.doc.update(cx, |d, cx| {
            d.edit_document("Delete context", cx, |doc| {
                if let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                    ps.contexts.retain(|c| c.id != id);
                    if ps.active_context == Some(id) {
                        ps.active_context = None;
                    }
                }
            })
        });
        // The Part Studio regenerates on its own tab's change event.
        self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(tab)));
    }

    /// "Assembly contexts" above the feature filter (live 504–508,
    /// dom/ctx-assembly-contexts-section.txt): the bold header, a dropdown
    /// "Context 1  ctx asm" (None / contexts) and, with a context shown,
    /// ⋯ (Rename…, Update context, Hide/show context instances…, Restore
    /// context view state, Follow position ▸ | Delete).
    pub(super) fn render_contexts_section(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let studio = self.studio(cx)?;
        // Only contexts a feature references exist for the user (live 502:
        // no section until a feature picks context geometry).
        // While a feature dialog picks context geometry the section shows
        // that context greyed, without ⋯ (live 504).
        let editing = self.edit.is_some();
        let pending = self.ui.in_context.as_ref().and_then(|c| c.ctx.context).filter(|id| self.edit.as_ref().is_some_and(|e| noshape_model::in_context::params_use_context(&e.dialog.read(cx).feature.params, Some(*id))));
        let mut referenced = (*self.referenced_contexts(cx)).clone();
        referenced.extend(pending);
        let contexts: Vec<(u32, String, String)> = studio.contexts.iter().filter(|c| referenced.contains(&c.id)).map(|c| (c.id, c.name.clone(), self.assembly_name(c.assembly, cx))).collect();
        if contexts.is_empty() {
            return None;
        }
        let active = studio.active_context.filter(|id| referenced.contains(id)).or(pending);
        // Out-of-date contexts (as the assembly last saw them) get a blue dot
        // after the header and before their name in the list (live 520–521).
        let tab = self.tab;
        let stale: HashSet<u32> = cx.try_global::<crate::assembly_view::StaleContexts>().map(|g| g.0.iter().filter(|(t, _)| *t == tab).map(|(_, id)| *id).collect()).unwrap_or_default();
        let any_stale = contexts.iter().any(|c| stale.contains(&c.0));
        let pal = p();
        let view = cx.entity();
        let renaming = self.ui.context_rename.clone();
        let current = active.and_then(|id| contexts.iter().find(|c| c.0 == id).cloned());
        let label: AnyElement = match (&renaming, &current) {
            (Some((rid, st, _)), Some(c)) if *rid == c.0 => div().flex_1().child(gpui_kit::component::input::Input::new(st).xsmall()).into_any_element(),
            (_, Some((_, name, asm))) => h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(10.))
                .child(div().truncate().when(editing, |d| d.text_color(rgb(0xaaaaaa))).child(crate::ui::user_text(name.clone())))
                .child(div().italic().text_color(rgb(if editing { 0xcccccc } else { 0x999999 })).truncate().child(asm.clone()))
                .into_any_element(),
            (_, None) => div().flex_1().italic().child("None").into_any_element(),
        };
        let v = view.clone();
        let list = contexts.clone();
        let dropdown = h_flex()
            .id("ctx-select")
            .w(px(153.))
            .h(px(30.))
            .pl(px(3.))
            .pr(px(8.))
            .rounded(px(5.25))
            .border_1()
            .border_color(rgb(if editing { 0xdddddd } else { 0x9aa5af }))
            .bg(rgb(0xffffff))
            .text_size(px(13.))
            .text_color(rgb(0x333333))
            .when(!editing, |d| d.cursor_pointer())
            .child(label)
            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(if editing { 0xcccccc } else { 0x333333 })))
            .on_click(move |e: &ClickEvent, window, cx| {
                if editing {
                    return;
                }
                let (v, list, stale) = (v.clone(), list.clone(), stale.clone());
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    m = m.select_list(153.);
                    let v0 = v.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new("None").checked(active.is_none()).on_click(move |_, _, cx| v0.update(cx, |this, cx| this.activate_context(None, cx))));
                    for (id, name, asm) in &list {
                        let (v, id) = (v.clone(), *id);
                        let dot = if stale.contains(&id) { "• " } else { "" };
                        m = m.item(crate::os_menu::OsMenuItem::new(format!("{dot}{name}  {asm}")).checked(active == Some(id)).on_click(move |_, _, cx| v.update(cx, |this, cx| this.activate_context(Some(id), cx))));
                    }
                    m
                });
                crate::os_menu::open(e.position(), menu, window, cx);
            });
        let more = current.as_ref().filter(|_| !editing).map(|(id, _, _)| {
            let (id, v) = (*id, view.clone());
            div()
                .id("ctx-more")
                .w(px(34.))
                .h(px(31.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(crate::assets::icon("more", 20., cx))
                .on_click(move |e: &ClickEvent, window, cx| {
                    let v = v.clone();
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| {
                        let it = |label: &'static str, f: StudioAct| {
                            let v = v.clone();
                            crate::os_menu::OsMenuItem::new(label).on_click(move |_, window, cx| v.update(cx, |this, cx| f(this, window, cx)))
                        };
                        m.list_no_icons()
                            .item(it("Rename…", Rc::new(move |this, window, cx| this.start_context_rename(id, window, cx))))
                            .item(it("Update context", Rc::new(move |this, _, cx| this.update_context_here(id, cx))))
                            .item(it("Hide/show context instances…", Rc::new(|this, _, cx| {
                                if let Some(c) = &mut this.ui.in_context {
                                    c.hide_show_open = true;
                                }
                                cx.notify();
                            })))
                            .item(it("Restore context view state", Rc::new(|this, _, cx| {
                                if let Some(c) = &mut this.ui.in_context {
                                    c.hidden.clear();
                                }
                                this.refresh_scene(false, cx);
                                cx.notify();
                            })))
                            // Follow position: the only position is the assembly's current one (live 510).
                            .submenu("Follow position", window, cx, |sm, _, _| sm.item(crate::os_menu::OsMenuItem::new("Current assembly position").checked(true)))
                            .separator()
                            .item(it("Delete", Rc::new(move |this, _, cx| this.delete_context(id, cx))))
                    });
                    crate::os_menu::open(e.position(), menu, window, cx);
                })
        });
        Some(
            v_flex()
                .flex_none()
                .pt(px(4.))
                .child(
                    h_flex()
                        .h(px(24.))
                        .pl(px(4.))
                        .gap(px(5.))
                        .text_size(px(13.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.text)
                        .child("Assembly contexts")
                        .when(any_stale, |d| d.child(div().id("ctx-stale-dot").size(px(6.)).rounded_full().bg(rgb(0x769fcc)).tooltip(crate::ui::tooltip("Update available for a context.")))),
                )
                .child(h_flex().h(px(32.)).pl(px(3.)).pr(px(9.)).child(dropdown).children(more))
                .into_any_element(),
        )
    }

    fn start_context_rename(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.studio(cx).and_then(|s| s.contexts.iter().find(|c| c.id == id)).map(|c| c.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(name));
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &gpui_kit::component::input::InputEvent, cx| {
            if matches!(ev, gpui_kit::component::input::InputEvent::PressEnter { .. } | gpui_kit::component::input::InputEvent::Blur) {
                let name = st.read(cx).value().to_string();
                this.ui.context_rename = None;
                this.rename_context(id, name, cx);
                cx.notify();
            }
        });
        self.ui.context_rename = Some((id, st, Rc::new(sub)));
        cx.notify();
    }

    /// Whether the shown context is referenced (the banner turns amber
    /// "Context 1 of <assembly>" once a feature picks its geometry).
    pub(super) fn shown_context_referenced(&self, cx: &App) -> Option<(u32, String)> {
        let id = self.ui.in_context.as_ref()?.ctx.context?;
        let studio = self.studio(cx)?;
        let c = studio.contexts.iter().find(|c| c.id == id)?;
        let mut referenced = self.referenced_contexts(cx).contains(&id);
        if let Some(e) = &self.edit {
            referenced |= noshape_model::in_context::params_use_context(&e.dialog.read(cx).feature.params, Some(id));
        }
        referenced.then(|| (id, c.name.clone()))
    }
}
