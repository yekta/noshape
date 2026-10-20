//! Drawing editor: view menu › Select for replication ▸ View | Region… and
//! Replicate annotations (live drawing/447–452, dom/drawing-replicate-*.txt;
//! both items carry a "beta" badge). View remembers the view's annotations;
//! Region… opens "Replicate region" (a center-point rectangle: the dashed
//! orange box with blue corner grips, field "Create rectangle" → "Edit closed
//! profile: Profile ×") and remembers the annotations inside it. Replicate
//! annotations on another view copies them there (Region: "Place rectangle",
//! the box follows the pointer). A copy is made where the target view shows
//! the same geometry: each reference is the same model point, or a vertex /
//! circle center at the mapped spot; the others are skipped.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::build::ViewPlacement;
use noshape_model::drawing::views::{self, SnapKind};
use noshape_model::drawing::{Annotation, AnnotationRef, PointRef, ViewId};
use noshape_render::{Drawable, Instance, LineData, LineSegment, PointData, PointSprite, Scene};

/// What Select for replication picked: the annotations (with their
/// reference points, view coordinates), the source view's placement and
/// the region (sheet mm), if any.
pub struct Replication {
    pub items: Vec<(Annotation, Vec<[f64; 2]>)>,
    pub src: ViewPlacement,
    pub region: Option<[[f64; 2]; 2]>,
}

/// The Replicate region dialog.
pub enum ReplicateDlg {
    /// Region…: draw the rectangle on `view` (center, then a corner).
    Create { view: ViewId, center: Option<[f64; 2]>, rect: Option<[[f64; 2]; 2]> },
    /// Replicate annotations with a region: place the box on `view`.
    Place { view: ViewId, at: Option<[f64; 2]> },
}

/// The rectangle's dashed outline (live #CC925C) and grips (#90CEF1).
const RECT_ORANGE: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
const GRIP_BLUE: [u8; 4] = [0x90, 0xce, 0xf1, 255];
/// References match within this (view coordinates, m).
const MATCH_TOL: f64 = 1e-5;

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl DrawingEditor {
    /// The view menu's replication items (live: after Paste as table).
    pub fn replication_entries(&self, id: ViewId) -> Vec<MenuEntry> {
        vec![
            MenuEntry::Submenu(
                "Select for replication\tbeta".into(),
                vec![
                    MenuEntry::item("View", move |t, _, cx| t.select_for_replication(id, None, cx)),
                    MenuEntry::item("Region...", move |t, _, cx| {
                        t.replicate_dlg = Some(ReplicateDlg::Create { view: id, center: None, rect: None });
                        t.tool = Tool::Select;
                        cx.notify();
                    }),
                ],
            ),
            MenuEntry::item("Replicate annotations\tbeta", move |t, _, cx| t.replicate_annotations(id, cx)).disabled(self.replication.is_none()),
        ]
    }

    /// Remember the view's annotations (inside `region`, sheet mm, if any).
    fn select_for_replication(&mut self, view: ViewId, region: Option<[[f64; 2]; 2]>, cx: &mut Context<Self>) {
        let Some(src) = self.placement(view) else { return };
        let Some(geo) = self.geoms.get(&view).cloned() else { return };
        let sheet = self.current_sheet();
        let Some(ctx) = views::view_source(sheet, view).and_then(|(k, _)| self.sources.get(&k)).map(|s| s.context.clone()) else { return };
        let inside = |a: &Annotation| -> bool {
            let Some(r) = region else { return true };
            let Some(b) = &self.built else { return false };
            let picks = self.picks_of(Sel::Annotation(a.id));
            let pts: Vec<[f64; 2]> = b.geometry.lines.iter().filter(|l| picks.contains(&l.pick)).flat_map(|l| l.points.iter().copied()).collect();
            if pts.is_empty() {
                return false;
            }
            let c = pts.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
            let c = [c[0] / pts.len() as f64, c[1] / pts.len() as f64];
            (r[0][0]..=r[1][0]).contains(&c[0]) && (r[0][1]..=r[1][1]).contains(&c[1])
        };
        let mut items = Vec::new();
        for a in sheet.annotations.iter().filter(|a| a.kind.view() == Some(view) && inside(a)) {
            let pts: Option<Vec<[f64; 2]>> = a.refs.iter().map(|r| views::resolve_in_view(&ctx, &geo, &r.point).map(|(p, _)| p)).collect();
            if let Some(pts) = pts {
                items.push((a.clone(), pts));
            }
        }
        self.replication = Some(Replication { items, src, region });
        cx.notify();
    }

    /// Replicate annotations on `view`: at once (View), or place the
    /// region's box first.
    fn replicate_annotations(&mut self, view: ViewId, cx: &mut Context<Self>) {
        let Some(r) = &self.replication else { return };
        if r.region.is_some() {
            self.replicate_dlg = Some(ReplicateDlg::Place { view, at: None });
            self.tool = Tool::Select;
            cx.notify();
        } else {
            self.replicate_into(view, None, cx);
        }
    }

    /// Copy the remembered annotations onto `target`; `delta` (sheet mm)
    /// moves a region's contents, else view coordinates carry over.
    fn replicate_into(&mut self, target: ViewId, delta: Option<[f64; 2]>, cx: &mut Context<Self>) {
        let Some(r) = &self.replication else { return };
        let (Some(dst), Some(geo)) = (self.placement(target), self.geoms.get(&target).cloned()) else { return };
        let sheet = self.current_sheet();
        let Some(ctx) = views::view_source(sheet, target).and_then(|(k, _)| self.sources.get(&k)).map(|s| s.context.clone()) else { return };
        let src = r.src;
        let map = |p: [f64; 2]| match delta {
            None => p,
            Some(d) => {
                let s = src.to_sheet(p);
                dst.to_view([s[0] + d[0], s[1] + d[1]])
            }
        };
        let shift = delta.unwrap_or_else(|| {
            let (a, b) = (src.to_sheet([0.0, 0.0]), dst.to_sheet([0.0, 0.0]));
            [b[0] - a[0], b[1] - a[1]]
        });
        // A placed box is positioned by hand: snap it so the first vertex /
        // circle center lands on the nearest one of its kind (3 mm on the
        // sheet), and move every point by the same amount.
        let kind_of = |p: &PointRef| match p {
            PointRef::Vertex { .. } => Some(SnapKind::Vertex),
            PointRef::Center { .. } => Some(SnapKind::Center),
            _ => None,
        };
        let mut corr = [0.0, 0.0];
        if delta.is_some() {
            let tol = 3.0 / (dst.scale * 1000.0).max(1e-9);
            let first = r.items.iter().flat_map(|(a, pts)| a.refs.iter().zip(pts)).find_map(|(rf, p)| kind_of(&rf.point).map(|k| (k, map(*p))));
            if let Some((k, q)) = first
                && let Some(s) = geo.snaps.iter().filter(|s| s.kind == k).min_by(|a, b| dist(a.p, q).total_cmp(&dist(b.p, q)))
                && dist(s.p, q) < tol
            {
                corr = [s.p[0] - q[0], s.p[1] - q[1]];
            }
        }
        let map = |p: [f64; 2]| {
            let q = map(p);
            [q[0] + corr[0], q[1] + corr[1]]
        };
        let shift = match delta {
            Some(_) => {
                let (a, b) = (dst.to_sheet([0.0, 0.0]), dst.to_sheet(corr));
                [shift[0] + b[0] - a[0], shift[1] + b[1] - a[1]]
            }
            None => shift,
        };
        let mut next = sheet.next_annotation_id();
        let existing: Vec<&Annotation> = sheet.annotations.iter().filter(|a| a.kind.view() == Some(target)).collect();
        let mut out: Vec<Annotation> = Vec::new();
        for (a, pts) in &r.items {
            // Region copies need references to land on.
            if a.refs.is_empty() && delta.is_some() {
                continue;
            }
            let mut refs = Vec::new();
            for (rf, p) in a.refs.iter().zip(pts) {
                let q = map(*p);
                if views::resolve_in_view(&ctx, &geo, &rf.point).is_some_and(|(x, _)| dist(x, q) < MATCH_TOL) {
                    refs.push(rf.clone());
                    continue;
                }
                let Some(point) = kind_of(&rf.point).and_then(|k| geo.snaps.iter().find(|s| s.kind == k && dist(s.p, q) < MATCH_TOL)).and_then(|s| s.point.clone()) else { break };
                refs.push(AnnotationRef { slot: rf.slot, point });
            }
            if refs.len() != a.refs.len() {
                continue;
            }
            let same = |e: &&Annotation| std::mem::discriminant(&e.kind) == std::mem::discriminant(&a.kind) && e.refs == refs;
            if !refs.is_empty() && (existing.iter().any(same) || out.iter().any(|e| same(&e))) {
                continue;
            }
            let mut b = a.clone();
            b.id = next;
            next += 1;
            b.refs = refs;
            if let Some(v) = b.kind.view_mut() {
                *v = target;
            }
            b.position = [b.position[0] + shift[0], b.position[1] + shift[1]];
            b.group = None;
            b.view_group = None;
            b.ann_group = None;
            b.inspection = None;
            out.push(b);
        }
        let n = out.len();
        if n > 0 {
            self.edit_sheet("Replicate annotations", cx, |s| s.annotations.extend(out));
        }
        self.status = Some(match n {
            1 => "1 annotation replicated".into(),
            n => format!("{n} annotations replicated").into(),
        });
        cx.notify();
    }

    /// A sheet click with the dialog open. True when used.
    pub fn replicate_click(&mut self, cx: &mut Context<Self>) -> bool {
        let c = self.cursor;
        match &mut self.replicate_dlg {
            None => return false,
            Some(ReplicateDlg::Create { center, rect, .. }) => {
                if rect.is_some() {
                    return true;
                }
                match *center {
                    None => *center = Some(c),
                    Some(m) => {
                        let h = [(c[0] - m[0]).abs(), (c[1] - m[1]).abs()];
                        if h[0] > 1e-6 && h[1] > 1e-6 {
                            *rect = Some([[m[0] - h[0], m[1] - h[1]], [m[0] + h[0], m[1] + h[1]]]);
                        }
                    }
                }
            }
            Some(ReplicateDlg::Place { at, .. }) => *at = Some(c),
        }
        cx.notify();
        true
    }

    /// The source region's half size (sheet mm).
    fn replicate_half(&self) -> Option<[f64; 2]> {
        let r = self.replication.as_ref()?.region?;
        Some([(r[1][0] - r[0][0]) * 0.5, (r[1][1] - r[0][1]) * 0.5])
    }

    pub fn close_replicate(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.replicate_dlg.take() else { return };
        if accept {
            match d {
                ReplicateDlg::Create { view, rect: Some(rect), .. } => self.select_for_replication(view, Some(rect), cx),
                ReplicateDlg::Place { view, at: Some(at) } => {
                    if let Some(r) = self.replication.as_ref().and_then(|r| r.region) {
                        let c = [(r[0][0] + r[1][0]) * 0.5, (r[0][1] + r[1][1]) * 0.5];
                        self.replicate_into(view, Some([at[0] - c[0], at[1] - c[1]]), cx);
                    }
                }
                _ => {}
            }
        }
        cx.notify();
    }

    /// The rectangle while the dialog is open (from the scene build).
    pub fn push_replicate_overlay(&self, scene: &mut Scene) {
        let Some(d) = &self.replicate_dlg else { return };
        let c = self.cursor;
        let mut lines = LineData::default();
        let mut grips = PointData::default();
        let seg = |l: &mut LineData, a: [f64; 2], b: [f64; 2], color: [u8; 4], width: f32, style: u32| {
            l.segments.push(LineSegment { a: [a[0] as f32, a[1] as f32, 0.6], b: [b[0] as f32, b[1] as f32, 0.6], color, pick_id: 0, width, style: LineSegment::ON_TOP | style });
        };
        let rect = |l: &mut LineData, r: [[f64; 2]; 2], color: [u8; 4], width: f32, style: u32| {
            let pts = [r[0], [r[1][0], r[0][1]], r[1], [r[0][0], r[1][1]], r[0]];
            for w in pts.windows(2) {
                seg(l, w[0], w[1], color, width, style);
            }
        };
        let black = [0, 0, 0, 255];
        let finished = |l: &mut LineData, g: &mut PointData, r: [[f64; 2]; 2]| {
            rect(l, r, RECT_ORANGE, 2.0, LineSegment::DASHED);
            for p in [r[0], [r[1][0], r[0][1]], r[1], [r[0][0], r[1][1]]] {
                g.points.push(PointSprite { position: [p[0] as f32, p[1] as f32, 0.65], color: GRIP_BLUE, pick_id: 0, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
            }
        };
        match d {
            ReplicateDlg::Create { rect: Some(r), .. } => finished(&mut lines, &mut grips, *r),
            ReplicateDlg::Create { center: Some(m), .. } => {
                // Live drawing/449: the diagonal dashed, the box thin black.
                let h = [(c[0] - m[0]).abs(), (c[1] - m[1]).abs()];
                seg(&mut lines, *m, c, black, 1.0, LineSegment::DASHED);
                rect(&mut lines, [[m[0] - h[0], m[1] - h[1]], [m[0] + h[0], m[1] + h[1]]], black, 1.0, 0);
            }
            ReplicateDlg::Create { .. } => {}
            ReplicateDlg::Place { at, .. } => {
                if let Some(h) = self.replicate_half() {
                    let m = at.unwrap_or(c);
                    let r = [[m[0] - h[0], m[1] - h[1]], [m[0] + h[0], m[1] + h[1]]];
                    if at.is_some() {
                        finished(&mut lines, &mut grips, r);
                    } else {
                        rect(&mut lines, r, black, 1.0, 0);
                    }
                }
            }
        }
        let up = self.gpu.uploader();
        if !lines.segments.is_empty() {
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        if !grips.points.is_empty() {
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&grips))).per_primitive_color().unpickable().layer(19));
        }
    }

    pub fn render_replicate_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.replicate_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let (ready, placing, profile) = match d {
            ReplicateDlg::Create { rect, .. } => (rect.is_some(), false, rect.is_some()),
            ReplicateDlg::Place { at, .. } => (at.is_some(), true, false),
        };
        let prompt = if placing { "Place rectangle" } else { "Create rectangle" };
        let field = div()
            .mx(px(3.))
            .mb(px(3.))
            .px(px(6.))
            .rounded(px(2.))
            .border_1()
            .border_color(gpui_kit::rgb(0x5da7dd))
            .bg(gpui_kit::rgb(0xd9eef9))
            .map(|e| {
                if profile {
                    e.py(px(3.)).child(div().text_size(px(10.)).text_color(gpui_kit::rgb(0x5c6670)).child("Edit closed profile")).child(
                        h_flex().h(px(22.)).text_size(px(13.)).child(div().flex_1().child("Profile")).child(div().id("rr-profile-x").px(px(4.)).text_color(gpui_kit::rgb(0x9a9a9a)).cursor_pointer().child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(ReplicateDlg::Create { center, rect, .. }) = &mut this.replicate_dlg {
                                *center = None;
                                *rect = None;
                            }
                            cx.notify();
                        }))),
                    )
                } else {
                    e.h(px(25.)).flex().items_center().text_size(px(11.)).text_color(gpui_kit::rgb(0x5c6670)).child(prompt)
                }
            });
        Some(
            v_flex()
                .id("dr-replicate")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
                .pb(px(3.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(30.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Replicate region"))
                        .child(
                            div()
                                .id("rr-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(if ready { 0x009500 } else { crate::drawing_chrome::OK_DISABLED }))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_replicate(true, cx))),
                        )
                        .child(div().id("rr-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_replicate(false, cx)))),
                )
                // The rectangle tool (live: center-point rectangle; pressed
                // while placing).
                .child(h_flex().px(px(3.)).py(px(4.)).child(div().size(px(28.)).flex().items_center().justify_center().rounded(px(2.)).when(placing, |e| e.bg(gpui_kit::rgb(0xdfe7ee)).border_1().border_color(gpui_kit::rgb(0xb8c4cf))).child(crate::ui::glyph("center-rectangle", 18., ink))))
                .child(field)
                .into_any_element(),
        )
    }
}
