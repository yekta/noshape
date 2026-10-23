//! Onshape's Create selection dialog (live: part-studio/380–398,
//! dom/dialog-create-selection.txt; Help: Create selection). It floats at
//! the right-click point, or beside the feature dialog when opened from a
//! field's "Create selection" button; picks fill its Selection field, the
//! result previews in the query highlight and [Add selection] adds it to the
//! selection (or to the feature dialog's field) and resets the dialog.

use super::PartStudioView;
use crate::scene_builder::PickKind;
use crate::theme::{p, with_alpha};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::reference::Reference;
use noshape_model::select::Criterion;
use std::sync::Arc;

/// Faces tab select, in Onshape's order and labels.
const FACE_KINDS: [(Criterion, &str); 6] = [
    (Criterion::Protrusion, "Protrusion"),
    (Criterion::Pocket, "Pocket"),
    (Criterion::Hole, "Hole"),
    (Criterion::Fillets, "Fillets"),
    (Criterion::TangentFaces, "Tangent connected"),
    (Criterion::BoundedFaces, "Bounded faces"),
];
/// Edges tab select.
const EDGE_KINDS: [(Criterion, &str); 4] = [
    (Criterion::TangentEdges, "Tangent connected"),
    (Criterion::EdgeLoop, "Loop/chain connected"),
    (Criterion::EqualRadius, "Equal length/radius"),
    (Criterion::ParallelEdges, "Parallel"),
];

pub struct CreateSelection {
    /// Edges tab (it always opens on Faces).
    pub edges: bool,
    pub face_kind: Criterion,
    pub edge_kind: Criterion,
    /// The Selection field (seed and, for Bounded faces, the boundary).
    pub seeds: Vec<Reference>,
    pub patterns: bool,
    /// What the criterion finds from the seeds (previewed).
    pub found: Vec<Reference>,
    /// Top-left in viewport coordinates.
    pub pos: Point<Pixels>,
    /// Where [Add selection] puts the result.
    pub target: CsTarget,
    /// The criterion list is open (a native select: no search field).
    pub list_open: bool,
    grab: Arc<parking_lot::Mutex<Point<Pixels>>>,
}

/// What [Add selection] fills.
#[derive(Clone, Debug, PartialEq)]
pub enum CsTarget {
    /// The Part Studio selection.
    Selection,
    /// A feature dialog field (its path).
    Field(String),
    /// A Flatten surfaces field (0 = Faces to flatten, 1 = Rip edges).
    Flatten(u8),
}

/// Dragging the dialog by its header.
#[derive(Clone)]
pub struct CsMove(pub Arc<parking_lot::Mutex<Point<Pixels>>>);

struct CsMoveGhost;

impl Render for CsMoveGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn kind_label(c: Criterion) -> &'static str {
    FACE_KINDS.iter().chain(EDGE_KINDS.iter()).find(|(k, _)| *k == c).map(|(_, l)| *l).unwrap_or("")
}


impl PartStudioView {
    /// Open the dialog with its top-left at `pos` (viewport coordinates).
    pub(super) fn open_create_selection(&mut self, pos: Point<Pixels>, target: CsTarget, cx: &mut Context<Self>) {
        self.close_create_selection(cx);
        if let (CsTarget::Field(f), Some(e)) = (&target, &self.edit) {
            let f = f.clone();
            e.dialog.update(cx, |d, cx| {
                d.cs_target = Some(f);
                cx.notify();
            });
        }
        self.ui.create_selection = Some(CreateSelection {
            edges: false,
            face_kind: Criterion::Protrusion,
            edge_kind: Criterion::TangentEdges,
            seeds: Vec::new(),
            patterns: false,
            found: Vec::new(),
            pos,
            target,
            list_open: false,
            grab: Default::default(),
        });
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// From a context menu at `window_pos`: at the click point; with a
    /// feature dialog open the result goes to its active field (Help).
    pub(super) fn open_create_selection_at(&mut self, window_pos: Point<Pixels>, cx: &mut Context<Self>) {
        let origin = self.viewport.read(cx).origin();
        let target = match self.edit.as_ref().and_then(|e| e.dialog.read(cx).active_query.clone()) {
            Some(f) => CsTarget::Field(f),
            None => CsTarget::Selection,
        };
        self.open_create_selection(window_pos - origin, target, cx);
    }

    /// From a feature dialog field's button: beside the dialog (live
    /// part-studio/394: its left edge at the dialog's right edge, level
    /// with the field).
    pub(super) fn open_create_selection_for_field(&mut self, field: String, cx: &mut Context<Self>) {
        let origin = self.ui.dialog_pos.unwrap_or(point(px(0.), px(3.)));
        let width = self.ui.dialog_width.unwrap_or(px(220.));
        self.open_create_selection(point(origin.x + width + px(4.), origin.y + px(97.)), CsTarget::Field(field), cx);
    }

    /// From a Flatten surfaces field's button: beside that dialog.
    pub(super) fn open_create_selection_for_flatten(&mut self, field: u8, cx: &mut Context<Self>) {
        self.open_create_selection(point(px(224.), px(100.)), CsTarget::Flatten(field), cx);
    }

    /// A feature dialog closing takes a Create selection dialog that fills
    /// one of its fields with it.
    pub(super) fn close_create_selection_of_dialog(&mut self, cx: &mut Context<Self>) {
        if self.ui.create_selection.as_ref().is_some_and(|c| matches!(c.target, CsTarget::Field(_))) {
            self.close_create_selection(cx);
        }
    }

    pub(super) fn close_create_selection(&mut self, cx: &mut Context<Self>) {
        if self.ui.create_selection.take().is_none() {
            return;
        }
        if let Some(e) = &self.edit {
            e.dialog.update(cx, |d, cx| {
                d.cs_target = None;
                cx.notify();
            });
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// A pick while the dialog is open: toggles it in the Selection field.
    pub(super) fn cs_pick(&mut self, r: &Reference, kind: PickKind, cx: &mut Context<Self>) {
        let Some(cs) = &mut self.ui.create_selection else { return };
        // Faces and edges of parts, sketch regions and curves (Help: loops
        // continue across sketch edges).
        if !matches!(kind, PickKind::Face | PickKind::Edge | PickKind::SketchRegion | PickKind::SketchCurve) {
            return;
        }
        if let Some(i) = cs.seeds.iter().position(|x| x == r) {
            cs.seeds.remove(i);
        } else {
            cs.seeds.push(r.clone());
        }
        self.cs_update(cx);
    }

    /// Re-evaluate the criterion for the current seeds.
    fn cs_update(&mut self, cx: &mut Context<Self>) {
        let Some(cs) = &self.ui.create_selection else { return };
        let (seeds, c, patterns) = (cs.seeds.clone(), if cs.edges { cs.edge_kind } else { cs.face_kind }, cs.patterns);
        let found = match (&self.context, c) {
            (_, _) if seeds.is_empty() => Vec::new(),
            (Some(ctx), Criterion::BoundedFaces) => match seeds.iter().find(|r| matches!(r, Reference::Face(_))) {
                Some(seed) => {
                    let boundary: Vec<Reference> = seeds.iter().filter(|r| *r != seed).cloned().collect();
                    noshape_model::select::select_bounded(ctx, seed, &boundary)
                }
                None => seeds.iter().filter(|r| matches!(r, Reference::SketchRegion { .. })).cloned().collect(),
            },
            (Some(ctx), Criterion::EdgeLoop) => {
                let mut out = noshape_model::select::select_loops(ctx, &seeds);
                for s in seeds.iter().filter(|r| matches!(r, Reference::SketchEntity { .. })) {
                    out.extend(self.related(s, Criterion::EdgeLoop));
                }
                out
            }
            (Some(_), c) => {
                // Faces criteria grow faces, edge criteria grow edges.
                let mut out: Vec<Reference> = Vec::new();
                for s in &seeds {
                    let fits = match s {
                        Reference::Face(_) | Reference::SketchRegion { .. } => !cs.edges,
                        _ => cs.edges,
                    };
                    if fits {
                        out.extend(self.related(s, c));
                    }
                }
                out
            }
            (None, _) => Vec::new(),
        };
        let mut uniq: Vec<Reference> = Vec::new();
        for r in found {
            if !uniq.contains(&r) {
                uniq.push(r);
            }
        }
        if patterns && let Some(ctx) = &self.context {
            uniq = noshape_model::select::with_pattern_instances(ctx, &uniq);
        }
        if let Some(cs) = &mut self.ui.create_selection {
            cs.found = uniq;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// [Add selection]: the result joins the selection (or the feature
    /// dialog's field) and the dialog resets, staying open.
    fn cs_add(&mut self, cx: &mut Context<Self>) {
        let Some(cs) = &mut self.ui.create_selection else { return };
        let found = std::mem::take(&mut cs.found);
        cs.seeds.clear();
        let target = cs.target.clone();
        match (&self.edit, target) {
            (_, CsTarget::Flatten(i)) => {
                if let Some(fl) = &mut self.ui.flatten {
                    for r in found {
                        let list = match (i, &r) {
                            (0, Reference::Face(_)) => &mut fl.faces,
                            (1, Reference::Edge(_)) => &mut fl.rips,
                            _ => continue,
                        };
                        if !list.contains(&r) {
                            list.push(r);
                        }
                    }
                }
                self.redraw_flatten(cx);
            }
            (Some(e), CsTarget::Field(field)) => {
                let dialog = e.dialog.clone();
                dialog.update(cx, |d, cx| {
                    let prev = d.active_query.replace(field.clone());
                    if let Some((_, filter, _)) = d.active_filter() {
                        let current = d.selected_refs();
                        for r in found {
                            let kind = match r {
                                Reference::Edge(_) => PickKind::Edge,
                                Reference::SketchEntity { .. } => PickKind::SketchCurve,
                                Reference::SketchRegion { .. } => PickKind::SketchRegion,
                                _ => PickKind::Face,
                            };
                            if super::kind_matches(kind, &r, filter) && !current.contains(&r) {
                                d.add_pick(r, cx);
                            }
                        }
                    }
                    d.active_query = prev.or(Some(field));
                });
            }
            _ => {
                for r in found {
                    if !self.selection.contains(&r) {
                        self.selection.push(r);
                    }
                }
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Preview: the seeds are selected (orange), the found entities use the
    /// query highlight (live part-studio/387: translucent yellow-green).
    pub(super) fn cs_preview(&self) -> Option<(Vec<Reference>, Vec<Reference>)> {
        let cs = self.ui.create_selection.as_ref()?;
        Some((cs.seeds.clone(), cs.found.iter().filter(|r| !cs.seeds.contains(r)).cloned().collect()))
    }

    pub(super) fn render_create_selection(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let labeler = self.labeler(cx);
        let cs = self.ui.create_selection.as_ref()?;
        let edges = cs.edges;
        let current = if edges { cs.edge_kind } else { cs.face_kind };
        let list: &'static [(Criterion, &'static str)] = if edges { &EDGE_KINDS } else { &FACE_KINDS };
        let n = cs.found.len();
        let all_faces = cs.found.iter().all(|r| matches!(r, Reference::Face(_) | Reference::SketchRegion { .. }));
        let all_edges = cs.found.iter().all(|r| matches!(r, Reference::Edge(_) | Reference::SketchEntity { .. }));
        let noun = match (n, all_faces, all_edges) {
            (0, _, _) => "entities",
            (1, true, _) => "face",
            (_, true, _) => "faces",
            (1, _, true) => "edge",
            (_, _, true) => "edges",
            (1, _, _) => "entity",
            _ => "entities",
        };
        let result = format!("{n} {noun} selected");
        let tab = |label: &'static str, on: bool, to_edges: bool, cx: &mut Context<Self>| {
            // Live: 110 px halves, 12 px text, the selected one 500 blue with
            // a 2 px blue underline over the row's 1 px #eee rule.
            div()
                .id(label)
                .flex_1()
                .h(px(26.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                .when(!on, |d| d.text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(cs) = &mut this.ui.create_selection {
                        cs.edges = to_edges;
                    }
                    this.cs_update(cx);
                }))
        };
        let chips: Vec<AnyElement> = cs
            .seeds
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let r2 = r.clone();
                let r3 = r.clone();
                h_flex()
                    .id(SharedString::from(format!("cs-seed-{i}")))
                    .h(px(23.))
                    .pl(px(6.))
                    .pr(px(4.))
                    .text_size(px(13.33))
                    .text_color(gpui_kit::rgb(0x555555))
                    .hover(|d| d.bg(with_alpha(gpui_kit::rgb(0xffffff), 0.35)))
                    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                        let want = on.then(|| r3.clone());
                        if this.ui.query_hover != want {
                            this.ui.query_hover = want;
                            this.refresh_scene(false, cx);
                        }
                    }))
                    .child(div().flex_1().truncate().child(labeler(r)))
                    .child(
                        div()
                            .id(SharedString::from(format!("cs-seed-x-{i}")))
                            .group("cs-seed-x")
                            .cursor_pointer()
                            // Live: Onshape's selection-list × (7 px, #666), red on hover.
                            .child(crate::ui::glyph("ui-clear", 9., gpui_kit::rgb(0x666666)).group_hover("cs-seed-x", |s| s.text_color(pal.error)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if let Some(cs) = &mut this.ui.create_selection {
                                    cs.seeds.retain(|x| *x != r2);
                                }
                                this.ui.query_hover = None;
                                this.cs_update(cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        let empty = chips.is_empty();
        let field = v_flex()
            .id("cs-selection")
            .mx(px(3.))
            .mt(px(5.))
            .min_h(px(26.))
            .max_h(px(160.))
            .overflow_y_scroll()
            .rounded(px(2.))
            .border_1()
            .border_color(pal.accent)
            .bg(pal.row_hover)
            .child(
                div()
                    .px(px(6.))
                    .when(empty, |d| d.pt(px(6.)).pb(px(5.)).text_size(px(12.)).line_height(px(13.33)))
                    .when(!empty, |d| d.pt(px(7.)).text_size(px(10.)).line_height(px(9.)))
                    .text_color(pal.text_muted)
                    .child("Selection"),
            )
            .children(chips);
        let patterns = cs.patterns;
        let checkbox = h_flex()
            .id("cs-patterns")
            .h(px(27.))
            .px(px(4.))
            .gap(px(4.))
            .cursor_pointer()
            .child(
                div()
                    .size(px(12.))
                    .rounded(px(3.))
                    .border_1()
                    .border_color(if patterns { pal.accent } else { gpui_kit::rgb(0xcccccc) })
                    .bg(if patterns { pal.accent } else { gpui_kit::rgb(0xffffff) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(patterns, |d| d.child(Icon::new(IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)))),
            )
            .child(div().text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child("Select patterns"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(cs) = &mut this.ui.create_selection {
                    cs.patterns = !cs.patterns;
                }
                this.cs_update(cx);
            }));
        let enabled = n > 0;
        let add = div()
            .id("cs-add")
            .w(px(135.))
            .h(px(27.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.5))
            .border_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .bg(gpui_kit::rgb(0xffffff))
            .text_size(px(12.25))
            .text_color(gpui_kit::rgb(0x333333))
            .when(!enabled, |d| d.opacity(0.4).text_color(gpui_kit::rgb(0x999999)))
            .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xf2f2f2))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cs_add(cx))))
            .child("Add selection");
        let grab = cs.grab.clone();
        Some(
            v_flex()
                .id("create-selection")
                .occlude()
                .absolute()
                .left(cs.pos.x)
                .top(cs.pos.y)
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    // Header: title and a red ✕, no ✓ (live).
                    h_flex()
                        .h(px(29.))
                        .flex_none()
                        .bg(pal.panel)
                        .border_b_1()
                        .border_color(pal.border)
                        .child(
                            div()
                                .id("cs-title")
                                .flex_1()
                                .min_w_0()
                                .pl(px(6.))
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Create selection")
                                // Onshape's panel-dialog header: grab.
                                .cursor(CursorStyle::OpenHand)
                                .on_drag(CsMove(grab), |d, offset, _, cx| {
                                    *d.0.lock() = offset;
                                    cx.new(|_| CsMoveGhost)
                                }),
                        )
                        .child(
                            div()
                                .id("cs-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_create_selection(cx))),
                        ),
                )
                .child(
                    h_flex()
                        .w_full()
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xeeeeee))
                        .child(tab("Faces", !edges, false, cx))
                        .child(tab("Edges", edges, true, cx)),
                )
                .child(self.render_cs_select(list, current, cs.list_open, cx))
                .child(field)
                .child(checkbox)
                .child(div().h(px(22.)).px(px(3.)).text_size(px(12.)).line_height(px(21.6)).child(result))
                .child(h_flex().h(px(27.)).px(px(3.)).justify_center().child(add))
                .child(
                    // ? help row (22 px).
                    h_flex()
                        .h(px(22.))
                        .pr(px(4.))
                        .justify_end()
                        .child(div().size(px(14.)).rounded_full().bg(with_alpha(pal.text, 0.55)).flex().items_center().justify_center().text_size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)).child("?")),
                )
                .into_any_element(),
        )
    }

    /// The criterion select: a native `<select>` (live
    /// dom/dialog-create-selection.txt: 194 px, white, 1 px #d7d7d7 underline,
    /// 13.33 px black text, padding 1 6 1 / 20 px right for the arrow).
    fn render_cs_select(&self, list: &'static [(Criterion, &'static str)], current: Criterion, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let highlight = gpui_kit::rgb(0x1e90ff);
        let rows = list.iter().map(|(k, label)| {
            let (k, on) = (*k, *k == current);
            div()
                .id(SharedString::from(format!("cs-opt-{label}")))
                .h(px(20.))
                .px(px(6.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .text_color(gpui_kit::rgb(0x000000))
                .when(on, |d| d.bg(highlight).text_color(gpui_kit::rgb_static(0xffffff)))
                .hover(|d| d.bg(highlight).text_color(gpui_kit::rgb_static(0xffffff)))
                .child(*label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(cs) = &mut this.ui.create_selection {
                        if cs.edges {
                            cs.edge_kind = k;
                        } else {
                            cs.face_kind = k;
                        }
                        cs.list_open = false;
                    }
                    this.cs_update(cx);
                }))
        });
        v_flex()
            .w(px(194.))
            .child(
                h_flex()
                    .id("cs-kind")
                    .h(px(26.))
                    .pl(px(6.))
                    .pr(px(6.))
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .cursor_pointer()
                    .child(div().flex_1().text_size(px(13.33)).text_color(gpui_kit::rgb(0x000000)).child(kind_label(current)))
                    .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(cs) = &mut this.ui.create_selection {
                            cs.list_open = !cs.list_open;
                        }
                        cx.notify();
                    })),
            )
            // The list hangs from the field, over the rows below.
            .when(open, |d| {
                d.child(div().w_full().h_0().child(
                    deferred(
                        anchored().snap_to_window_with_margin(px(4.)).child(
                            v_flex()
                                .id("cs-kind-list")
                                .occlude()
                                .w(px(194.))
                                .py(px(1.))
                                .bg(gpui_kit::rgb(0xffffff))
                                .border_1()
                                .border_color(gpui_kit::rgb(0x767676))
                                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(2.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }])
                                .children(rows)
                                .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                    if let Some(cs) = &mut this.ui.create_selection {
                                        cs.list_open = false;
                                    }
                                    cx.notify();
                                })),
                        ),
                    )
                    .with_priority(3),
                ))
            })
            .into_any_element()
    }

    /// Drag of the dialog header (bound on the viewport area).
    pub(super) fn cs_drag(&mut self, e: &DragMoveEvent<CsMove>, cx: &mut Context<Self>) {
        let grab = *e.drag(cx).0.lock();
        let p = e.event.position - e.bounds.origin - grab;
        let max_x = (e.bounds.size.width - px(80.)).max(px(0.));
        let max_y = (e.bounds.size.height - px(40.)).max(px(0.));
        if let Some(cs) = &mut self.ui.create_selection {
            cs.pos = point(p.x.clamp(px(0.), max_x), p.y.clamp(px(0.), max_y));
        }
        cx.notify();
    }
}
