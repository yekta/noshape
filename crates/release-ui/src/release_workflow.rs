//! ⋯ › View workflow (bundle: `toggleWorkflowDiagram` + `osx-workflow-diagram`):
//! the candidate's workflow as a diagram in a featherlight lightbox, the
//! current state highlighted ("you are here"). The lightbox is
//! max(window − 300, 800) × max(window − 300, 400) on a solid #333 backdrop;
//! Esc or the ✕ or a click on the backdrop closes it (and only it: the
//! release dialog ignores Esc while it is open). The diagram pans and zooms
//! like d3-zoom (wheel, drag, double-click) and nodes can be dragged.
//!
//! Drawn as SVG (the edge labels follow their edges, rotated) and rendered
//! with resvg in Roboto.

use gpui_kit::*;
use std::fmt::Write as _;
use std::sync::Arc;

/// Transition ui hints (bundle `getTransitionUiHint`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hint {
    Primary,
    Success,
    Danger,
}

struct State {
    name: &'static str,
    label: &'static str,
    /// Layout position (the default workflow as Onshape lays it out, help
    /// image enterprise/custom-workflow-ex: Setup top left, Pending in the
    /// middle, Released bottom left, Rejected bottom right).
    at: (f32, f32),
}

struct Transition {
    label: &'static str,
    from: &'static str,
    to: &'static str,
    hint: Hint,
}

struct Workflow {
    states: &'static [State],
    transitions: &'static [Transition],
}

/// "Onshape Release" (help: the default release workflow; transitions
/// SUBMIT, CREATE_AND_RELEASE (APPROVE_DIRECT), RELEASE (APPROVE), REJECT;
/// discarding has no target state, so it is not drawn).
const RELEASE: Workflow = Workflow {
    states: &[
        State { name: "SETUP", label: "Setup", at: (756., 277.) },
        State { name: "PENDING", label: "Pending", at: (1043., 463.) },
        State { name: "RELEASED", label: "Released", at: (715., 576.) },
        State { name: "REJECTED", label: "Rejected", at: (1407., 576.) },
    ],
    transitions: &[
        Transition { label: "Submit", from: "SETUP", to: "PENDING", hint: Hint::Primary },
        Transition { label: "Release", from: "SETUP", to: "RELEASED", hint: Hint::Success },
        Transition { label: "Release", from: "PENDING", to: "RELEASED", hint: Hint::Success },
        Transition { label: "Reject", from: "PENDING", to: "REJECTED", hint: Hint::Danger },
    ],
};

/// "Onshape Obsoletion": the same shape ending in Obsoleted.
const OBSOLETION: Workflow = Workflow {
    states: &[
        State { name: "SETUP", label: "Setup", at: (756., 277.) },
        State { name: "PENDING", label: "Pending", at: (1043., 463.) },
        State { name: "OBSOLETED", label: "Obsoleted", at: (715., 576.) },
        State { name: "REJECTED", label: "Rejected", at: (1407., 576.) },
    ],
    transitions: &[
        Transition { label: "Submit", from: "SETUP", to: "PENDING", hint: Hint::Primary },
        Transition { label: "Obsolete", from: "SETUP", to: "OBSOLETED", hint: Hint::Success },
        Transition { label: "Obsolete", from: "PENDING", to: "OBSOLETED", hint: Hint::Success },
        Transition { label: "Reject", from: "PENDING", to: "REJECTED", hint: Hint::Danger },
    ],
};

/// Node kinds (bundle: no incoming edge = initial, in and out = inner,
/// only incoming = terminal_bad when one of them is a danger transition,
/// else terminal_good). The lightbox strokes the node in its colour and
/// fills only the current state with it.
fn node_color(wf: &Workflow, name: &str) -> &'static str {
    let ein: Vec<&Transition> = wf.transitions.iter().filter(|t| t.to == name).collect();
    let eout = wf.transitions.iter().any(|t| t.from == name);
    if ein.is_empty() {
        "#def1fb"
    } else if eout {
        "#90cef1"
    } else if ein.iter().any(|t| t.hint == Hint::Danger) {
        "#dc777d"
    } else {
        "#aee28c"
    }
}

/// Node size (bundle: 180 × 60, + 2 px, + 10 px for the current state).
fn node_size(here: bool) -> (f32, f32) {
    let pad = if here { 10. } else { 2. };
    (180. + pad, 60. + pad)
}

pub(super) struct Lightbox {
    obsoletion: bool,
    /// The current state's name.
    here: &'static str,
    /// Node positions (draggable).
    at: Vec<(f32, f32)>,
    /// d3-zoom transform: translate, then scale.
    tx: f32,
    ty: f32,
    k: f32,
    fitted: bool,
    drag: Option<Drag>,
    focus: FocusHandle,
    image: Option<(String, Arc<RenderImage>)>,
}

enum Drag {
    Pan { from: Point<Pixels>, tx: f32, ty: f32 },
    Node { i: usize, from: Point<Pixels>, at: (f32, f32) },
}

impl Lightbox {
    /// `here`: "SETUP" while the candidate is set up, else its final state.
    pub(super) fn new(obsoletion: bool, released: bool, cx: &mut App) -> Self {
        let wf = if obsoletion { &OBSOLETION } else { &RELEASE };
        let here = if !released {
            "SETUP"
        } else if obsoletion {
            "OBSOLETED"
        } else {
            "RELEASED"
        };
        Self { obsoletion, here, at: wf.states.iter().map(|s| s.at).collect(), tx: 0., ty: 0., k: 1., fitted: false, drag: None, focus: cx.focus_handle(), image: None }
    }

    fn workflow(&self) -> &'static Workflow {
        if self.obsoletion { &OBSOLETION } else { &RELEASE }
    }

    /// Diagram size for a window (bundle: max(w − 300, 800) × max(h − 300, 400)).
    fn size(window: &Window) -> (f32, f32) {
        let vp = window.viewport_size();
        ((f32::from(vp.width) - 300.).max(800.), (f32::from(vp.height) - 300.).max(400.))
    }

    /// zoomToFit: the nodes' box scaled to 95 % of the diagram, centred
    /// (scale clamped to d3's [1/4, 4]).
    fn fit(&mut self, w: f32, h: f32) {
        let wf = self.workflow();
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (s, &(x, y)) in wf.states.iter().zip(&self.at) {
            let (nw, nh) = node_size(s.name == self.here);
            x0 = x0.min(x - nw / 2.);
            y0 = y0.min(y - nh / 2.);
            x1 = x1.max(x + nw / 2.);
            y1 = y1.max(y + nh / 2.);
        }
        let (bw, bh) = (x1 - x0, y1 - y0);
        if bw <= 0. || bh <= 0. {
            return;
        }
        self.k = (0.95 / (bw / w).max(bh / h)).clamp(0.25, 4.);
        self.tx = w / 2. - self.k * (x0 + bw / 2.);
        self.ty = h / 2. - self.k * (y0 + bh / 2.);
    }

    fn svg(&self, w: f32, h: f32, window: &Window) -> String {
        let wf = self.workflow();
        let pos = |name: &str| wf.states.iter().position(|s| s.name == name).map(|i| self.at[i]).unwrap_or_default();
        let half = |name: &str| {
            let (nw, nh) = node_size(name == self.here);
            (nw / 2. + 2., nh / 2. + 2.)
        };
        // Where the segment from the centre `a` towards `b` leaves the box
        // of half size `hs` around `a`.
        let exit = |a: (f32, f32), b: (f32, f32), hs: (f32, f32)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let t = (hs.0 / dx.abs().max(1e-6)).min(hs.1 / dy.abs().max(1e-6)).min(1.);
            (a.0 + dx * t, a.1 + dy * t)
        };
        let mut s = String::new();
        let _ = write!(
            s,
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Roboto"><defs><marker id="wkfl-end-arrow" viewBox="0 -5 10 10" refX="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0,-5L10,0L0,5L2,0" fill="#cccccc" stroke-width="0"/></marker></defs><rect x="0.5" y="0.5" width="{}" height="{}" fill="#ffffff" stroke="#d7d7d7" stroke-width="1"/><g transform="translate({} {}) scale({})">"##,
            w - 1.,
            h - 1.,
            self.tx,
            self.ty,
            self.k
        );
        // Links: boundary to boundary, the arrow tip (8 px past the line's
        // end) on the target's border.
        for (i, t) in wf.transitions.iter().enumerate() {
            let (a, b) = (pos(t.from), pos(t.to));
            let p0 = exit(a, b, half(t.from));
            let p1 = exit(b, a, half(t.to));
            let len = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt().max(1e-3);
            let back = 8f32.min(len);
            let p1 = (p1.0 - (p1.0 - p0.0) / len * back, p1.1 - (p1.1 - p0.1) / len * back);
            let _ = write!(s, r##"<line id="e_{i}" x1="{}" y1="{}" x2="{}" y2="{}" stroke="#cccccc" stroke-width="2" marker-end="url(#wkfl-end-arrow)"/>"##, p0.0, p0.1, p1.0, p1.1);
        }
        for (st, &(x, y)) in wf.states.iter().zip(&self.at) {
            let here = st.name == self.here;
            let (nw, nh) = node_size(here);
            let color = node_color(wf, st.name);
            let _ = write!(
                s,
                r##"<rect x="{}" y="{}" width="{nw}" height="{nh}" rx="4" ry="4" fill="{}" stroke="{color}" stroke-width="4"/>"##,
                x - nw / 2.,
                y - nh / 2.,
                if here { color } else { "#ffffff" }
            );
            // Label: centred, baseline at y + bbox.height / 4.
            let _ = write!(s, r##"<text x="{x}" y="{}" text-anchor="middle" font-size="12" font-weight="{}" fill="#000000">{}</text>"##, y + 14. / 4., if here { 700 } else { 500 }, st.label);
        }
        // Edge labels: on the centre-to-centre path, 125 px from the source,
        // 3 px off it; turned to read left to right when the edge points left.
        for (i, t) in wf.transitions.iter().enumerate() {
            let (a, b) = (pos(t.from), pos(t.to));
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if b.0 < a.0 {
                let tw = crate::ui::text_width(window, t.label, 12.);
                let _ = write!(s, r##"<path id="ep_{i}" d="M {} {} L {} {}" fill="none" stroke="none"/>"##, b.0, b.1, a.0, a.1);
                // Rotating the label 180° about its box keeps the box on the
                // same side of the edge.
                let _ = write!(s, r##"<text font-size="12" fill="#555555" dy="{}"><textPath xlink:href="#ep_{i}" startOffset="{}">{}</textPath></text>"##, 3. + 12. * (0.927 - 0.244), (len - 125. - tw).max(0.), t.label);
            } else {
                let _ = write!(s, r##"<path id="ep_{i}" d="M {} {} L {} {}" fill="none" stroke="none"/>"##, a.0, a.1, b.0, b.1);
                let _ = write!(s, r##"<text font-size="12" fill="#555555" dy="-3"><textPath xlink:href="#ep_{i}" startOffset="125">{}</textPath></text>"##, t.label);
            }
        }
        s.push_str("</g></svg>");
        s
    }

    /// The node under a point of the diagram (diagram pixels).
    fn node_at(&self, p: (f32, f32)) -> Option<usize> {
        let (x, y) = ((p.0 - self.tx) / self.k, (p.1 - self.ty) / self.k);
        let wf = self.workflow();
        (0..wf.states.len()).rev().find(|&i| {
            let (nw, nh) = node_size(wf.states[i].name == self.here);
            let (cx, cy) = self.at[i];
            (x - cx).abs() <= nw / 2. + 2. && (y - cy).abs() <= nh / 2. + 2.
        })
    }

    /// Zoom by `factor` about the diagram point `p`.
    fn zoom_at(&mut self, p: (f32, f32), factor: f32) {
        let k = (self.k * factor).clamp(0.25, 4.);
        let f = k / self.k;
        self.tx = p.0 - (p.0 - self.tx) * f;
        self.ty = p.1 - (p.1 - self.ty) * f;
        self.k = k;
    }
}

/// The app's UI font for the diagram's text (Onshape's Flama → Roboto).
static FONTS: std::sync::LazyLock<Arc<resvg::usvg::fontdb::Database>> = std::sync::LazyLock::new(|| {
    let mut db = resvg::usvg::fontdb::Database::new();
    for data in [&include_bytes!("../../app/assets/fonts/Roboto-Regular.ttf")[..], include_bytes!("../../app/assets/fonts/Roboto-Medium.ttf"), include_bytes!("../../app/assets/fonts/Roboto-Bold.ttf")] {
        db.load_font_data(data.to_vec());
    }
    db.set_sans_serif_family("Roboto");
    Arc::new(db)
});

/// SVG → GPUI image (BGRA): `svg` (opaque: a background rect fills it) as
/// an image `w` x `h` at scale `sf`.
pub(crate) fn rasterize(svg: &str, w: f32, h: f32, sf: f32) -> Option<Arc<RenderImage>> {
    use resvg::{tiny_skia, usvg};
    let opt = usvg::Options { fontdb: FONTS.clone(), font_family: "Roboto".into(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &opt).ok()?;
    let mut pm = tiny_skia::Pixmap::new((w * sf).ceil() as u32, (h * sf).ceil() as u32)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(sf, sf), &mut pm.as_mut());
    // Opaque (the background rect fills it), so premultiplied = straight.
    let mut px = pm.take();
    for p in px.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    let buf = image::RgbaImage::from_raw((w * sf).ceil() as u32, (h * sf).ceil() as u32, px)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
}

impl super::ReleaseDialog {
    /// ⋯ › View workflow.
    pub(super) fn toggle_workflow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workflow.take().is_some() {
            // featherlight gives the focus back to what had it.
            if let Some(f) = self.workflow_return.take() {
                window.focus(&f, cx);
            }
        } else {
            self.workflow_return = window.focused(cx);
            let obsoletion = self.mode == super::Mode::Obsolete || self.package.as_ref().is_some_and(|p| p.kind == noshape_model::release::PackageKind::Obsoletion);
            let released = self.package.as_ref().is_some_and(|p| p.version.is_some());
            let lb = Lightbox::new(obsoletion, released, cx);
            window.focus(&lb.focus, cx);
            self.workflow = Some(lb);
        }
        // The modal's builder (its Esc flag) runs when the root renders.
        window.refresh();
        cx.notify();
    }

    /// The lightbox over the whole window.
    pub(super) fn render_workflow(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (w, h) = Lightbox::size(window);
        let sf = window.scale_factor();
        let lb = self.workflow.as_mut()?;
        if !lb.fitted {
            lb.fit(w, h);
            lb.fitted = true;
        }
        let key = format!("{w}x{h}@{sf}:{:.2},{:.2},{:.4}:{:?}", lb.tx, lb.ty, lb.k, lb.at);
        if lb.image.as_ref().is_none_or(|(k, _)| *k != key) {
            let svg = lb.svg(w, h, window);
            lb.image = rasterize(&svg, w, h, sf).map(|i| (key, i));
        }
        let image = lb.image.as_ref().map(|(_, i)| i.clone());
        let focus = lb.focus.clone();
        let vp = window.viewport_size();
        // featherlight-content: 25 px padding (bottom: a 25 px border), centred.
        let (cw, ch) = (w + 50., h + 50.);
        let origin = (((f32::from(vp.width) - cw) / 2.).max(0.) + 25., ((f32::from(vp.height) - ch) / 2.).max(0.) + 25.);
        let local = move |p: Point<Pixels>| (f32::from(p.x) - origin.0, f32::from(p.y) - origin.1);
        let close_hover = gpui_kit::rgb(0xd7d7d7);
        let content = div()
            .id("wkfl-content")
            .relative()
            .w(px(cw))
            .h(px(ch))
            .p(px(25.))
            .bg(gpui_kit::rgb(0xffffff))
            .cursor_default()
            // A click inside does not close the lightbox.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("wkfl-diagram")
                    .w(px(w))
                    .h(px(h))
                    .children(image.map(|i| img(ImageSource::Render(i)).w(px(w)).h(px(h))))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            let Some(lb) = this.workflow.as_mut() else { return };
                            let p = local(ev.position);
                            if ev.click_count == 2 {
                                // d3-zoom: double-click zooms in ×2 (Shift: out).
                                lb.zoom_at(p, if ev.modifiers.shift { 0.5 } else { 2. });
                            } else {
                                lb.drag = Some(match lb.node_at(p) {
                                    Some(i) => Drag::Node { i, from: ev.position, at: lb.at[i] },
                                    None => Drag::Pan { from: ev.position, tx: lb.tx, ty: lb.ty },
                                });
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        let Some(lb) = this.workflow.as_mut() else { return };
                        if ev.pressed_button != Some(MouseButton::Left) {
                            lb.drag = None;
                            return;
                        }
                        match lb.drag {
                            Some(Drag::Pan { from, tx, ty }) => {
                                lb.tx = tx + f32::from(ev.position.x - from.x);
                                lb.ty = ty + f32::from(ev.position.y - from.y);
                            }
                            Some(Drag::Node { i, from, at }) => {
                                lb.at[i] = (at.0 + f32::from(ev.position.x - from.x) / lb.k, at.1 + f32::from(ev.position.y - from.y) / lb.k);
                            }
                            None => return,
                        }
                        cx.notify();
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| {
                        if let Some(lb) = this.workflow.as_mut() {
                            lb.drag = None;
                        }
                    }))
                    .on_scroll_wheel(cx.listener(move |this, ev: &ScrollWheelEvent, _, cx| {
                        let Some(lb) = this.workflow.as_mut() else { return };
                        // d3-zoom wheelDelta: −deltaY × 0.002 per pixel (0.05 per line).
                        let dy = match ev.delta {
                            ScrollDelta::Pixels(d) => f32::from(d.y) * 0.002,
                            ScrollDelta::Lines(d) => d.y * 0.05,
                        };
                        lb.zoom_at(local(ev.position), 2f32.powf(dy));
                        cx.notify();
                    })),
            )
            // featherlight-close-icon: 25 px, top right, "✕" (Arial), white
            // 30 % behind it; hover #d7d7d7 and bold.
            .child(
                div()
                    .id("wkfl-close")
                    .absolute()
                    .top_0()
                    .right_0()
                    .w(px(25.))
                    .h(px(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(hsla(0., 0., 1., 0.3))
                    .text_size(px(14.))
                    .font_family("Arial")
                    .text_color(gpui_kit::rgb(0x000000))
                    .cursor_pointer()
                    .hover(move |d| d.bg(close_hover).font_weight(FontWeight::BOLD))
                    .child("✕")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_workflow(window, cx))),
            );
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("wkfl-lightbox")
                        .track_focus(&focus)
                        .occlude()
                        .w(vp.width)
                        .h(vp.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui_kit::rgb(0x333333))
                        .cursor_pointer()
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                            if ev.keystroke.key == "escape" {
                                cx.stop_propagation();
                                this.toggle_workflow(window, cx);
                            }
                        }))
                        // closeOnClick: 'background'.
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, window, cx| this.toggle_workflow(window, cx)))
                        .child(content),
                ),
            )
            .with_priority(10)
            .into_any_element(),
        )
    }
}
