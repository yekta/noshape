//! Analysis tools (Onshape "Analysis tools" menu): curvature color map
//! (Shift+C) and draft analysis (Shift+D) as per-vertex color maps, with a
//! legend panel. View in high quality (Shift+R) refines the tessellation.

use super::PartStudioView;
use glam::DVec3;
use gpui_kit::component::h_flex;
use gpui_kit::*;
use noshape_model::display::BodyDisplay;

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis {
    /// Mean curvature magnitude, blue (flat) to red (tight).
    Curvature,
    /// Draft relative to the mold's pull direction (Onshape): side 1 light
    /// blue (draft ≥ the minimum toward the pull), side 2 blue (away),
    /// steep faces yellow, undercut regions red. `parts` limits it to some
    /// parts (empty: all).
    Draft { pull: [f64; 3], angle_deg: f64, undercut: bool, parts: Vec<noshape_model::BodyId> },
}


/// Blue → cyan → green → yellow → red ramp for t in 0..1.
pub fn ramp(t: f64) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let stops: [[f64; 3]; 5] = [[40.0, 80.0, 220.0], [40.0, 200.0, 220.0], [60.0, 200.0, 80.0], [240.0, 210.0, 40.0], [220.0, 50.0, 40.0]];
    let x = t * 4.0;
    let i = (x.floor() as usize).min(3);
    let f = x - i as f64;
    let a = stops[i];
    let b = stops[i + 1];
    let c = |k: usize| (a[k] + (b[k] - a[k]) * f).round() as u8;
    [c(0), c(1), c(2), 255]
}

/// Draft analysis colors (Onshape's legend: Steep, Undercut, Side 1, Side 2).
pub const DRAFT_STEEP: [u8; 4] = [0xf8, 0xe7, 0x1c, 255];
pub const DRAFT_UNDERCUT: [u8; 4] = [0xd0, 0x02, 0x1b, 255];
pub const DRAFT_SIDE1: [u8; 4] = [0x83, 0xd1, 0xff, 255];
pub const DRAFT_SIDE2: [u8; 4] = [0x00, 0x6f, 0xff, 255];

/// Per-vertex colors of a body for an analysis.
pub fn analysis_colors(b: &BodyDisplay, a: &Analysis) -> Vec<[u8; 4]> {
    let m = &b.mesh;
    match a {
        Analysis::Draft { pull, angle_deg, undercut, parts } => {
            if !parts.is_empty() && !parts.contains(&b.id) {
                // Not checked: the part's own color.
                return vec![[0, 0, 0, 0]; m.positions.len()];
            }
            let size = b.bounds.map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1);
            noshape_model::analysis::draft_classes(m, size, DVec3::from_array(*pull), angle_deg.to_radians(), *undercut)
                .into_iter()
                .map(|(_, c)| match c {
                    noshape_model::analysis::DraftClass::Steep => DRAFT_STEEP,
                    noshape_model::analysis::DraftClass::Undercut => DRAFT_UNDERCUT,
                    noshape_model::analysis::DraftClass::Side1 => DRAFT_SIDE1,
                    noshape_model::analysis::DraftClass::Side2 => DRAFT_SIDE2,
                })
                .collect()
        }
        Analysis::Curvature => {
            let k = vertex_curvature(&m.positions, &m.normals, &m.indices);
            let size = b.bounds.map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1).max(1e-6);
            // Log scale: curvature radius from 1000x to 1/10 the part size.
            k.iter()
                .map(|&k| if k * size < 1e-3 { ramp(0.0) } else { ramp(((k * size).log10() + 3.0) / 4.0) })
                .collect()
        }
    }
}

/// Curvature color map (Onshape): each part's vertices colored by the
/// chosen curvature (Gaussian, mean, max or min radius) on one scale for
/// all parts, the bar's palette and bounds (values beyond user bounds get
/// the bar's outside colors); vertices where nothing curves keep the part's
/// color. Returns the colors and the data range (display units).
/// Per-vertex colors of each body.
pub type BodyColors = std::collections::HashMap<noshape_model::BodyId, std::sync::Arc<Vec<[u8; 4]>>>;

pub fn curvature_colors(bodies: &[BodyDisplay], map: Option<&super::thickness::CurvatureMap>, meters_per_unit: f64) -> (BodyColors, Option<(f64, f64)>) {
    use super::thickness::{CURVATURE_PALETTES, palette_color_in};
    let kind = map.map(|m| m.kind).unwrap_or_default();
    let values: Vec<(&BodyDisplay, Vec<Option<f64>>)> = bodies
        .iter()
        .filter(|b| !b.hidden)
        .map(|b| {
            let size = b.bounds.map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1);
            (b, noshape_model::analysis::curvature_values(&b.mesh, size, kind, meters_per_unit))
        })
        .collect();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for v in values.iter().flat_map(|(_, v)| v.iter().flatten()) {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    let range = (lo <= hi).then_some((lo, hi));
    let bounds = map.map(|m| m.bar.bounds).unwrap_or([None; 2]);
    let palette = map.map(|m| m.bar.palette).unwrap_or(0);
    let (blo, bhi) = (bounds[0].unwrap_or(lo), bounds[1].unwrap_or(hi));
    let rgba = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
    let colors = values
        .into_iter()
        .map(|(b, v)| {
            let c: Vec<[u8; 4]> = v
                .into_iter()
                .map(|v| match v {
                    // Alpha 0: the part's own color.
                    None => [0, 0, 0, 0],
                    Some(v) if bounds[0].is_some() && v < blo => rgba(0xcc5fc0),
                    Some(v) if bounds[1].is_some() && v > bhi => rgba(0x6b3a14),
                    Some(v) => rgba(palette_color_in(&CURVATURE_PALETTES, palette, if bhi > blo { (v - blo) / (bhi - blo) } else { 0.5 })),
                })
                .collect();
            (b.id.clone(), std::sync::Arc::new(c))
        })
        .collect();
    (colors, range)
}

/// Discrete curvature magnitude per vertex from normal variation along
/// triangle edges: |(n_i - n_j) · (p_i - p_j)| / |p_i - p_j|².
pub fn vertex_curvature(positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &[u32]) -> Vec<f64> {
    let p3 = |v: [f32; 3]| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
    let mut sum = vec![0.0f64; positions.len()];
    let mut cnt = vec![0u32; positions.len()];
    for t in indices.as_chunks::<3>().0 {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (a as usize, b as usize);
            let (Some(pa), Some(pb), Some(na), Some(nb)) = (positions.get(a), positions.get(b), normals.get(a), normals.get(b)) else { continue };
            let e = p3(*pa) - p3(*pb);
            let l2 = e.length_squared();
            if l2 < 1e-24 {
                continue;
            }
            let k = ((p3(*na) - p3(*nb)).dot(e) / l2).abs();
            for v in [a, b] {
                sum[v] += k;
                cnt[v] += 1;
            }
        }
    }
    sum.iter().zip(&cnt).map(|(s, c)| if *c == 0 { 0.0 } else { s / *c as f64 }).collect()
}

impl PartStudioView {
    /// Shift+C / Shift+D / Analysis menu: toggle an analysis map.
    pub(super) fn toggle_analysis(&mut self, a: Analysis, window: &mut Window, cx: &mut Context<Self>) {
        let same = std::mem::discriminant(&a) == self.ui.analysis.as_ref().map(std::mem::discriminant).unwrap_or(std::mem::discriminant(&Analysis::Curvature)) && self.ui.analysis.is_some();
        // Another analysis replaces an open Draft analysis dialog.
        if self.ui.draft.is_some() {
            self.close_analysis_tool(cx);
        }
        self.ui.analysis = if same { None } else { Some(a) };
        self.ui.curvature_map = match self.ui.analysis {
            Some(Analysis::Curvature) => self.ui.curvature_map.take().or_else(|| Some(Default::default())),
            _ => None,
        };
        // The curvature color map turns zebra stripes off (Onshape).
        if matches!(self.ui.analysis, Some(Analysis::Curvature)) {
            if self.ui.analysis_tool.as_ref().is_some_and(|t| t.kind == super::analysis_tools::ToolKind::Zebra) {
                self.close_analysis_tool(cx);
            }
            self.ui.zebra = None;
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    /// Shift+R: view in high quality (finer tessellation for every part).
    pub(super) fn toggle_high_quality(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.high_quality = !self.ui.high_quality;
        self.ui.high_quality_seen |= self.ui.high_quality;
        // Onshape's aggressive refinement: bodies refine down to a 0.1 px
        // chordal error (Very fine meshes come with the regen); off, the
        // ones refined stay so (Onshape, until reload).
        let error_px = if self.ui.high_quality { noshape_render::scene::MESH_ERROR_PX_HIGH_QUALITY_STUDIO } else { noshape_render::scene::MESH_ERROR_PX };
        self.viewport.update(cx, |vp, _| vp.style.detail.mesh_error_px = error_px);
        self.schedule_regen(window, cx);
    }

    /// The "View in high quality" notice. Live (part-studio/lod4-fill-shiftR):
    /// a 433 x 28 #90cef1 bar flush with the graphics area's top, centred,
    /// square; 13 px #333 text 12 px in (cap height 10, 288 px long); a white
    /// 65 x 21 "Disable" button (1 px #aaa, 3 px radius) 4 px from the top and
    /// 8 px from the right end.
    pub(super) fn render_analysis_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.ui.high_quality {
            return None;
        }
        let bar = h_flex()
            .id("analysis-panel")
            .occlude()
            .w(px(433.))
            .h(px(28.))
            .pl(px(12.))
            .pr(px(8.))
            .pt(px(1.))
            .bg(gpui_kit::rgb(0x90cef1))
            .text_size(px(13.))
            .text_color(gpui_kit::rgb(0x333333))
            .child(div().flex_1().relative().top(px(-1.)).child("Viewing in high quality. Performance can decrease."))
            .child(
                div()
                    .id("hq-off")
                    .w(px(65.))
                    .h(px(21.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xaaaaaa))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .child(div().relative().top(px(-1.)).child("Disable"))
                    .on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_high_quality(w, cx))),
            );
        Some(div().absolute().top_0().left_0().right_0().flex().justify_center().child(bar).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn curvature_of_points_on_a_circle_is_one_over_radius() {
        // Three points on a circle of radius 2 with radial normals.
        let r = 2.0f32;
        let pt = |a: f32| [r * a.cos(), r * a.sin(), 0.0];
        let nm = |a: f32| [a.cos(), a.sin(), 0.0];
        let positions = vec![pt(0.0), pt(0.3), pt(0.6)];
        let normals = vec![nm(0.0), nm(0.3), nm(0.6)];
        let k = vertex_curvature(&positions, &normals, &[0, 1, 2]);
        assert!(k.iter().all(|k| (k - 0.5).abs() < 1e-4), "{k:?}");
        assert!(ramp(0.0)[2] > ramp(0.0)[0] && ramp(1.0)[0] > ramp(1.0)[2]);
    }
}
