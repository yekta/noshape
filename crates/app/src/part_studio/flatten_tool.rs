//! Flatten surfaces (Analysis tools menu): the dialog (Faces to flatten,
//! Rip edges, display options, Origin, Flattened position, offset and
//! angle, export controls, Flatten), and the flat sheet drawn below the
//! faces (green; facets, edges, checkerboard and distortion on demand).
//! Results show after Flatten is pressed (Onshape).

use super::PartStudioView;
use crate::theme::p;
use glam::{DVec2, DVec3};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::flatten::Flattened;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData};
use std::sync::Arc;

/// Export formats offered by "Show export controls".
pub const EXPORT_FORMATS: [&str; 4] = ["PARASOLID", "STL", "DXF", "SVG"];

/// Flattened sheet color (Onshape's green) and distortion colors.
const SHEET: u32 = 0x6cc06c;
const COMPRESSION: u32 = 0xe0201c;
const TENSION: u32 = 0x2a5ce0;

pub struct FlattenState {
    /// Active field: 0 faces, 1 rip edges, 2 origin, 3 flattened position.
    pub active: u8,
    pub faces: Vec<Reference>,
    pub rips: Vec<Reference>,
    pub origin: Option<Reference>,
    pub position: Option<Reference>,
    pub show_facets: bool,
    pub show_edges: bool,
    pub show_checker: bool,
    pub checker_scale: f64,
    pub show_distortion: bool,
    pub distortion_scale: f64,
    pub show_flattened: bool,
    pub offset: f64,
    pub angle_deg: f64,
    pub show_export: bool,
    pub format: usize,
    pub inner_edges: bool,
    pub result: Option<Arc<Flattened>>,
    /// Uploaded drawables for the current result and options.
    pub drawn: Vec<Instance>,
    inputs: Vec<(Entity<InputState>, Subscription)>,
}

impl PartStudioView {
    pub(super) fn open_flatten(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let unit = units.length.symbol();
        type Setter = fn(&mut FlattenState, f64);
        let defaults: [(String, Setter); 4] = [
            ("10".into(), |s, v| s.checker_scale = v),
            ("1".into(), |s, v| s.distortion_scale = v),
            (format!("0 {unit}"), |s, v| s.offset = v),
            ("0 deg".into(), |s, v| s.angle_deg = v),
        ];
        let factor = units.length.meters();
        let mut inputs = Vec::new();
        for (k, (text, set)) in defaults.into_iter().enumerate() {
            let st = cx.new(|cx| InputState::new(window, cx).default_value(text));
            let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    return;
                }
                let text = st.read(cx).value().to_string();
                let num = text.trim().trim_end_matches(|c: char| c.is_alphabetic()).trim().parse::<f64>();
                let Ok(v) = num else { return this.flash_info("Enter a number".into(), cx) };
                // Offset is a length in the document's unit.
                let v = if k == 2 { v * factor } else { v };
                if let Some(s) = &mut this.ui.flatten {
                    set(s, v);
                }
                this.redraw_flatten(cx);
            });
            inputs.push((st, sub));
        }
        self.ui.flatten = Some(FlattenState {
            active: 0,
            faces: self.selection.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect(),
            rips: Vec::new(),
            origin: None,
            position: None,
            show_facets: false,
            show_edges: true,
            show_checker: false,
            checker_scale: 10.0,
            show_distortion: false,
            distortion_scale: 1.0,
            show_flattened: true,
            offset: 0.0,
            angle_deg: 0.0,
            show_export: false,
            format: 0,
            inner_edges: false,
            result: None,
            drawn: Vec::new(),
            inputs,
        });
    }

    /// A pick while Flatten surfaces is open fills its active field.
    pub(super) fn flatten_pick(&mut self, r: &Reference, cx: &mut Context<Self>) {
        let Some(s) = &mut self.ui.flatten else { return };
        let ok = match s.active {
            0 if matches!(r, Reference::Face(_)) => {
                toggle(&mut s.faces, r);
                true
            }
            1 if matches!(r, Reference::Edge(_)) => {
                toggle(&mut s.rips, r);
                true
            }
            2 if matches!(r, Reference::Vertex(_)) => {
                s.origin = (s.origin.as_ref() != Some(r)).then(|| r.clone());
                true
            }
            3 if matches!(r, Reference::MateConnector { .. }) => {
                s.position = (s.position.as_ref() != Some(r)).then(|| r.clone());
                true
            }
            _ => false,
        };
        if !ok {
            let msg = ["Select faces", "Select edges", "Select a vertex", "Select a mate connector"][s.active.min(3) as usize];
            self.flash_info(msg.into(), cx);
        }
        self.redraw_flatten(cx);
    }

    /// The Flatten button: compute the sheet.
    fn run_flatten(&mut self, cx: &mut Context<Self>) {
        let Some(s) = &self.ui.flatten else { return };
        let (faces, rips) = (s.faces.clone(), s.rips.clone());
        if faces.is_empty() {
            self.flash_info("Select faces to flatten".into(), cx);
            return;
        }
        // The faces' display triangles, refined, as one mesh.
        let mut m = noshape_model::display::MeshBuffers::default();
        for b in &self.display.bodies {
            for (fi, r) in b.face_refs.iter().enumerate() {
                if !faces.contains(r) {
                    continue;
                }
                let Some(&(first, count)) = b.mesh.face_ranges.get(fi) else { continue };
                let start = m.indices.len() as u32;
                for &ix in b.mesh.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]) {
                    m.indices.push(m.positions.len() as u32);
                    m.positions.push(b.mesh.positions[ix as usize]);
                    m.normals.push(b.mesh.normals[ix as usize]);
                }
                m.face_ranges.push((start, m.indices.len() as u32 - start));
            }
        }
        let mut rip_lines: Vec<Vec<DVec3>> = rips.iter().map(|r| self.curve_points_of(r)).collect();
        // Faces that close on themselves (a cylinder) open along their seams.
        if let Some(ctx) = &self.context {
            rip_lines.extend(noshape_model::flatten::seam_edges(ctx, &faces));
        }
        let spacing = noshape_model::thickness::spacing_for(&m, 4000);
        let sheet = noshape_model::thickness::refine(&m, spacing);
        let flat = noshape_model::flatten::flatten(&sheet, &rip_lines, 300);
        if let Some(s) = &mut self.ui.flatten {
            s.result = Some(Arc::new(flat));
        }
        self.redraw_flatten(cx);
    }

    /// Polyline of a part edge (display tessellation).
    fn curve_points_of(&self, r: &Reference) -> Vec<DVec3> {
        let p3 = |p: &[f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        self.display.bodies.iter().find_map(|b| b.edge_refs.iter().position(|x| x == r).and_then(|i| b.mesh.edges.get(i)).map(|pl| pl.iter().map(p3).collect())).unwrap_or_default()
    }

    /// Where each vertex of the sheet lies in 3D: in the Top plane (Onshape),
    /// turned to match the faces' footprint and centered under them, or
    /// with the Origin vertex's point right under that vertex; on the chosen
    /// mate connector's XY plane (offset along its Z, turned about it).
    fn placed_sheet(&self, s: &FlattenState, f: &Flattened) -> Vec<DVec3> {
        if let Some(c) = s.position.as_ref().and_then(|r| self.display.connectors.iter().find(|c| c.reference == *r)) {
            let (a, b) = s.angle_deg.to_radians().sin_cos();
            let x = c.x.normalize_or_zero();
            let y = c.z.normalize_or_zero().cross(x);
            let (u, v) = (x * b + y * a, y * b - x * a);
            let origin = c.origin + c.z.normalize_or_zero() * s.offset;
            return f.flat.iter().map(|q| origin + u * q.x + v * q.y).collect();
        }
        // Rotation (no mirroring; the outer side up) best matching the footprint.
        let mirror = f.normal.z < 0.0;
        let q: Vec<DVec2> = f.flat.iter().map(|q| if mirror { DVec2::new(q.x, -q.y) } else { *q }).collect();
        let foot: Vec<DVec2> = f.positions.iter().map(|p| p.truncate()).collect();
        let fm = foot.iter().copied().sum::<DVec2>() / foot.len().max(1) as f64;
        let (mut c, mut s2) = (0.0, 0.0);
        for (a, b) in q.iter().zip(&foot) {
            c += a.dot(*b - fm);
            s2 += a.perp_dot(*b - fm);
        }
        let rot = DVec2::from_angle(s2.atan2(c));
        let mut placed: Vec<DVec2> = q.iter().map(|a| rot.rotate(*a) + fm).collect();
        if let Some(vtx) = s.origin.as_ref().and_then(|r| self.vertex_position(r))
            && let Some(i) = (0..f.positions.len()).min_by(|&a, &b| f.positions[a].distance(vtx).total_cmp(&f.positions[b].distance(vtx)))
        {
            let shift = vtx.truncate() - placed[i];
            for p in &mut placed {
                *p += shift;
            }
        }
        placed.into_iter().map(|p| p.extend(0.0)).collect()
    }

    fn vertex_position(&self, r: &Reference) -> Option<DVec3> {
        self.display.bodies.iter().find_map(|b| b.vertex_refs.iter().position(|x| x == r).and_then(|i| b.mesh.vertices.get(i)).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)))
    }

    /// Rebuild the drawables for the current result and options.
    pub(super) fn redraw_flatten(&mut self, cx: &mut Context<Self>) {
        let drawn = match self.ui.flatten.as_ref() {
            Some(s) => match s.result.clone() {
                Some(f) => self.flatten_instances(s, &f),
                None => Vec::new(),
            },
            None => Vec::new(),
        };
        if let Some(s) = &mut self.ui.flatten {
            s.drawn = drawn;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn flatten_instances(&self, s: &FlattenState, f: &Flattened) -> Vec<Instance> {
        let up = self.gpu.uploader();
        let placed = self.placed_sheet(s, f);
        let size = f.positions.iter().map(|p| p.distance(f.centroid)).fold(0.0, f64::max).max(1e-9);
        let f32v = |p: DVec3| [p.x as f32, p.y as f32, p.z as f32];
        // Checkerboard squares in flat coordinates: higher scales, smaller squares.
        let extent = f.flat.iter().fold(DVec2::ZERO, |m, q| m.max(q.abs())).max_element().max(1e-9);
        let checker = |q: DVec2| {
            let k = s.checker_scale.max(1.0) / (4.0 * extent);
            ((q.x * k).floor() as i64 + (q.y * k).floor() as i64).rem_euclid(2) == 0
        };
        let rgba = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
        let mix = |a: u32, b: u32, t: f64| {
            let ch = |c: u32, s: u32| ((c >> s) & 0xff) as f64;
            let m = |s: u32| (ch(a, s) * (1.0 - t) + ch(b, s) * t).round() as u32;
            (m(16) << 16) | (m(8) << 8) | m(0)
        };
        let sheet_colors: Vec<[u8; 4]> = (0..f.positions.len())
            .map(|i| {
                let mut c = SHEET;
                if s.show_checker {
                    c = if checker(f.flat[i]) { 0x202020 } else { 0xf4f4f4 };
                }
                if s.show_distortion {
                    let e = (f.strain[i] * 10.0 * s.distortion_scale).clamp(-1.0, 1.0);
                    c = mix(c, if e < 0.0 { COMPRESSION } else { TENSION }, e.abs());
                }
                rgba(c)
            })
            .collect();
        // The sheet's own plane (Top, or the mate connector's XY plane).
        let flat_normal = match f.indices.get(..3) {
            Some(&[a, b, c]) => (placed[b as usize] - placed[a as usize]).cross(placed[c as usize] - placed[a as usize]).try_normalize().unwrap_or(DVec3::Z),
            _ => DVec3::Z,
        };
        let normals = vec![f32v(flat_normal); f.positions.len()];
        let mut out = Vec::new();
        if s.show_flattened {
            let pos: Vec<[f32; 3]> = placed.iter().map(|p| f32v(*p)).collect();
            let mesh = up.upload_mesh_colored(&pos, &normals, &vec![0; pos.len()], &sheet_colors, &f.indices);
            // Translucent (Onshape's light green patch).
            out.push(Instance::new(Drawable::Mesh(mesh)).per_primitive_color().with_color([1.0, 1.0, 1.0, 0.7]).unpickable().ignore_clip().layer(6));
        }
        // The checkerboard on the original faces too (just off the surface).
        if s.show_checker {
            let lift = size * 1e-4;
            // Vertex normals from the triangles (the sheet keeps the faces' winding).
            let mut vn = vec![DVec3::ZERO; f.positions.len()];
            for t in f.indices.as_chunks::<3>().0 {
                let [a, b, c] = t.map(|i| i as usize);
                let n = (f.positions[b] - f.positions[a]).cross(f.positions[c] - f.positions[a]);
                for i in [a, b, c] {
                    vn[i] += n;
                }
            }
            let pos: Vec<[f32; 3]> = f.positions.iter().zip(&vn).map(|(p, n)| f32v(*p + n.normalize_or_zero() * lift)).collect();
            let colors: Vec<[u8; 4]> = f.flat.iter().map(|q| rgba(if checker(*q) { 0x202020 } else { 0xf4f4f4 })).collect();
            let normals3: Vec<[f32; 3]> = vn.iter().map(|n| f32v(n.normalize_or_zero())).collect();
            let mesh = up.upload_mesh_colored(&pos, &normals3, &vec![0; pos.len()], &colors, &f.indices);
            out.push(Instance::new(Drawable::Mesh(mesh)).per_primitive_color().with_color([1.0, 1.0, 1.0, 1.0]).unpickable().layer(7));
        }
        let mut lines = LineData::default();
        if s.show_facets {
            for t in f.indices.as_chunks::<3>().0 {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    if s.show_flattened {
                        lines.push_polyline(&[f32v(placed[a as usize]), f32v(placed[b as usize])], [70, 70, 70, 255], 0, 0.5, 0);
                    }
                    lines.push_polyline(&[f32v(f.positions[a as usize]), f32v(f.positions[b as usize])], [70, 70, 70, 255], 0, 0.5, 0);
                }
            }
        } else if s.show_edges && s.show_flattened {
            for [a, b] in f.outline(true) {
                lines.push_polyline(&[f32v(placed[a as usize]), f32v(placed[b as usize])], [30, 30, 30, 255], 0, 1.0, 0);
            }
        }
        if !lines.segments.is_empty() {
            out.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(8));
        }
        out
    }

    fn export_flattened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = &self.ui.flatten else { return };
        let Some(f) = s.result.clone() else { return self.flash_info("Press Flatten first".into(), cx) };
        let format = EXPORT_FORMATS[s.format];
        if format == "PARASOLID" {
            // A planar sheet where the flat pattern is drawn, one face per
            // flattened face with "Export inner edges".
            let sheet = match flat_sheet(&f, &self.placed_sheet(s, &f), s.inner_edges) {
                Ok(sh) => sh,
                Err(e) => return self.flash_info(format!("Export failed: {e}"), cx),
            };
            let view = cx.entity();
            crate::files::request_save_path("Export flattened surface", "Flattened.x_t".to_string(), "x_t", window, cx, move |path, cx| {
                if let Err(e) = noshape_kernel::write_parasolid(&path, &[sheet], noshape_kernel::LengthUnit::Meter, false) {
                    view.update(cx, |this, cx| this.flash_info(format!("Export failed: {e}"), cx));
                }
            });
            return;
        }
        let text = match format {
            "STL" => f.to_stl(&self.placed_sheet(s, &f)),
            "DXF" => f.to_dxf(s.inner_edges),
            _ => f.to_svg(s.inner_edges),
        };
        let ext = match format {
            "STL" => "stl",
            "DXF" => "dxf",
            _ => "svg",
        };
        let view = cx.entity();
        crate::files::request_save_path("Export flattened surface", format!("Flattened.{ext}"), ext, window, cx, move |path, cx| {
            if let Err(e) = std::fs::write(&path, text) {
                view.update(cx, |this, cx| this.flash_info(format!("Export failed: {e}"), cx));
            }
        });
    }

    /// The Flatten surfaces dialog body.
    pub(super) fn render_flatten_body(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.ui.flatten.as_ref()?;
        let pal = p();
        let labeler = self.labeler(cx);
        let field = |idx: u8, caption: &'static str, refs: Vec<Reference>, cx: &mut Context<Self>| {
            let active = s.active == idx;
            let empty = refs.is_empty();
            let rows: Vec<AnyElement> = refs
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let r2 = r.clone();
                    h_flex()
                        .id(SharedString::from(format!("fl-{idx}-{i}")))
                        .group("flrow")
                        .h(px(23.))
                        .px(px(6.))
                        .text_size(px(12.))
                        .child(div().flex_1().truncate().child(labeler(r)))
                        .child(
                            div()
                                .id(SharedString::from(format!("fl-{idx}-x-{i}")))
                                .invisible()
                                .group_hover("flrow", |d| d.visible())
                                .cursor_pointer()
                                .child(Icon::new(IconName::Close).with_size(px(11.)).text_color(pal.text_muted))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(s) = &mut this.ui.flatten {
                                        s.faces.retain(|x| *x != r2);
                                        s.rips.retain(|x| *x != r2);
                                        if s.origin.as_ref() == Some(&r2) {
                                            s.origin = None;
                                        }
                                        if s.position.as_ref() == Some(&r2) {
                                            s.position = None;
                                        }
                                    }
                                    this.redraw_flatten(cx);
                                })),
                        )
                        .into_any_element()
                })
                .collect();
            v_flex()
                .id(SharedString::from(format!("fl-field-{idx}")))
                .flex_1()
                .min_h(px(28.))
                .rounded(px(2.))
                .border_1()
                .border_color(if active { pal.accent } else { gpui_kit::rgb(0xd7d7d7) })
                .bg(if active { gpui_kit::rgb(0xdef1fb) } else { gpui_kit::rgb(0xffffff) })
                .cursor_pointer()
                .child(div().px(px(6.)).when(empty, |d| d.py(px(6.)).text_size(px(12.))).when(!empty, |d| d.pt(px(2.)).text_size(px(10.))).text_color(pal.text_muted).child(caption))
                .children(rows)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(s) = &mut this.ui.flatten {
                        s.active = idx;
                    }
                    this.refresh_scene(false, cx);
                    cx.notify();
                }))
        };
        // Live (analysis/70): both fields have the "Create selection" button.
        let cs_button = |idx: u8, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("fl-cs-{idx}")))
                .size(px(26.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                .child(crate::assets::icon("create-selection", 20., cx))
                .tooltip(crate::ui::tooltip("Create selection"))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(s) = &mut this.ui.flatten {
                        s.active = idx;
                    }
                    this.open_create_selection_for_flatten(idx, cx);
                }))
        };
        let check = |id: &'static str, label: &'static str, on: bool, set: fn(&mut FlattenState, bool), cx: &mut Context<Self>| {
            Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(s) = &mut this.ui.flatten {
                    set(s, *v);
                }
                this.redraw_flatten(cx);
            }))
        };
        let number = |label: &'static str, st: &Entity<InputState>| {
            h_flex().h(px(26.)).gap_2().text_size(px(12.)).child(div().text_color(pal.text_muted).child(label)).child(div().flex_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Input::new(st).appearance(false).xsmall().text_right()))
        };
        let inputs: Vec<Entity<InputState>> = s.inputs.iter().map(|(st, _)| st.clone()).collect();
        let (faces, rips) = (s.faces.clone(), s.rips.clone());
        let origin: Vec<Reference> = s.origin.iter().cloned().collect();
        let position: Vec<Reference> = s.position.iter().cloned().collect();
        let (facets, edges, checker, distortion, flattened, export, format, inner) = (s.show_facets, s.show_edges, s.show_checker, s.show_distortion, s.show_flattened, s.show_export, s.format, s.inner_edges);
        let view = cx.entity();
        let mut body = v_flex()
            .p(px(4.))
            .gap(px(6.))
            .child(h_flex().gap_1().items_start().child(field(0, "Faces to flatten", faces, cx)).child(cs_button(0, cx)))
            .child(h_flex().gap_1().items_start().child(field(1, "Rip edges", rips, cx)).child(cs_button(1, cx)))
            .child(check("fl-facets", "Show facets", facets, |s, v| s.show_facets = v, cx))
            .child(check("fl-edges", "Show edges", edges, |s, v| s.show_edges = v, cx))
            .child(check("fl-checker", "Show checkerboard", checker, |s, v| s.show_checker = v, cx));
        if checker {
            body = body.child(number("Checkerboard scale", &inputs[0]));
        }
        body = body.child(check("fl-distortion", "Show distortion", distortion, |s, v| s.show_distortion = v, cx));
        if distortion {
            body = body.child(number("Distortion scale", &inputs[1]));
        }
        body = body
            .child(check("fl-flattened", "Show flattened", flattened, |s, v| s.show_flattened = v, cx))
            .child(field(2, "Origin", origin, cx))
            .child(field(3, "Flattened position (mate connector)", position, cx))
            .child(number("Flattened offset", &inputs[2]))
            .child(number("Flattened angle", &inputs[3]))
            .child(check("fl-export", "Show export controls", export, |s, v| s.show_export = v, cx));
        if export {
            let v = view.clone();
            body = body
                .child(
                    h_flex().h(px(26.)).gap_2().text_size(px(12.)).child(div().text_color(pal.text_muted).child("Export format")).child(div().flex_1().child(Button::new("fl-format").ghost().xsmall().w_full().child(h_flex().w_full().child(div().flex_1().child(EXPORT_FORMATS[format])).child(Icon::new(IconName::ChevronDown).with_size(px(10.)))).dropdown_menu(move |mut m, _, _| {
                        for (i, name) in EXPORT_FORMATS.iter().enumerate() {
                            let v = v.clone();
                            m = m.item(PopupMenuItem::new(*name).checked(i == format).on_click(move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if let Some(s) = &mut this.ui.flatten {
                                        s.format = i;
                                    }
                                    cx.notify();
                                })
                            }));
                        }
                        m
                    }))),
                )
                .child(check("fl-inner", "Export inner edges", inner, |s, v| s.inner_edges = v, cx));
        }
        // [Flatten] (and [Export] with the export controls).
        body = body.child(
            h_flex()
                .pt(px(2.))
                .gap_2()
                .child(Button::new("fl-run").label("Flatten").outline().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run_flatten(cx))))
                .when(export, |d| d.child(Button::new("fl-export-btn").label("Export").outline().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.export_flattened(window, cx))))),
        );
        Some(body.into_any_element())
    }
}

/// The flat pattern as a planar sheet body at `placed` (one point per
/// vertex): each patch's longest loop bounds it, the others are holes.
fn flat_sheet(f: &Flattened, placed: &[DVec3], inner: bool) -> anyhow::Result<noshape_kernel::Shape> {
    let mut faces = Vec::new();
    for (_, loops) in f.loops(inner) {
        let pts: Vec<Vec<DVec3>> = loops.iter().map(|l| l.iter().map(|i| placed[*i as usize]).collect()).collect();
        // Newell normal of each loop; the largest is the outer one.
        let newell = |l: &[DVec3]| l.iter().zip(l.iter().cycle().skip(1)).fold(DVec3::ZERO, |n, (a, b)| n + DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y)));
        let Some(outer) = (0..pts.len()).max_by(|a, b| newell(&pts[*a]).length().total_cmp(&newell(&pts[*b]).length())) else { continue };
        let n = newell(&pts[outer]).normalize_or_zero();
        if n == DVec3::ZERO {
            continue;
        }
        let plane = noshape_kernel::Plane::from_normal(pts[outer][0], n);
        let wire = |l: &[DVec3]| noshape_kernel::make_polyline(l, true);
        let holes = pts.iter().enumerate().filter(|(i, _)| *i != outer).map(|(_, l)| wire(l)).collect::<Result<Vec<_>, _>>()?;
        faces.push(noshape_kernel::make_face(&plane, &wire(&pts[outer])?, &holes)?);
    }
    match faces.len() {
        0 => anyhow::bail!("the flat pattern has no boundary"),
        1 => Ok(faces.remove(0)),
        _ => Ok(noshape_kernel::sew(&faces, 1e-7, false)?.shape),
    }
}

fn toggle(list: &mut Vec<Reference>, r: &Reference) {
    match list.iter().position(|x| x == r) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(r.clone()),
    }
}
