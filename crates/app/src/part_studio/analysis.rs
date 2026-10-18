//! Analysis tools (Onshape "Analysis tools" menu): curvature color map
//! (Shift+C) and draft analysis (Shift+D) as per-vertex color maps, with a
//! legend panel. View in high quality (Shift+R) refines the tessellation.

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use glam::DVec3;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::display::BodyDisplay;

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis {
    /// Mean curvature magnitude, blue (flat) to red (tight).
    Curvature,
    /// Draft relative to a pull direction: green >= angle, red <= -angle,
    /// yellow in between (needs draft).
    Draft { pull: [f64; 3], angle_deg: f64 },
}

impl Analysis {
    pub fn title(&self) -> &'static str {
        match self {
            Analysis::Curvature => "Curvature map",
            Analysis::Draft { .. } => "Draft analysis",
        }
    }
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

pub const DRAFT_POSITIVE: [u8; 4] = [70, 180, 80, 255];
pub const DRAFT_NEGATIVE: [u8; 4] = [210, 60, 50, 255];
pub const DRAFT_NEEDED: [u8; 4] = [235, 200, 40, 255];

/// Per-vertex colors of a body for an analysis.
pub fn analysis_colors(b: &BodyDisplay, a: &Analysis) -> Vec<[u8; 4]> {
    let m = &b.mesh;
    let p3 = |v: [f32; 3]| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
    match a {
        Analysis::Draft { pull, angle_deg } => {
            let d = DVec3::from_array(*pull).normalize_or(DVec3::Z);
            let lim = angle_deg.to_radians().sin();
            m.normals
                .iter()
                .map(|n| {
                    // Draft angle = asin(n · pull).
                    let s = p3(*n).normalize_or_zero().dot(d);
                    if s >= lim - 1e-9 {
                        DRAFT_POSITIVE
                    } else if s <= -lim + 1e-9 {
                        DRAFT_NEGATIVE
                    } else {
                        DRAFT_NEEDED
                    }
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
        self.ui.analysis = if same { None } else { Some(a) };
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    /// Default draft analysis: pull along the selected plane/face normal,
    /// else Top (Z), 3°.
    pub(super) fn draft_analysis(&self) -> Analysis {
        let pull = self.selection.iter().find_map(|r| self.frame_of_ref(r)).map(|f| f.normal).unwrap_or(DVec3::Z);
        Analysis::Draft { pull: pull.to_array(), angle_deg: 3.0 }
    }

    /// Shift+R: view in high quality (finer tessellation for every part).
    pub(super) fn toggle_high_quality(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.high_quality = !self.ui.high_quality;
        self.schedule_regen(window, cx);
    }

    /// Legend of the active analysis (and the high-quality notice).
    pub(super) fn render_analysis_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        if self.ui.analysis.is_none() && !self.ui.high_quality {
            return None;
        }
        let mut col = v_flex()
            .id("analysis-panel")
            .occlude()
            .absolute()
            .bottom(px(46.))
            .left_3()
            .w(px(230.))
            .p_2()
            .gap_1()
            .rounded_md()
            .bg(with_alpha(pal.panel_raised, 0.96))
            .border_1()
            .border_color(pal.border_strong);
        if self.ui.high_quality {
            col = col.child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_xs().text_color(pal.info).child("Viewing in high quality. Performance can decrease."))
                    .child(Button::new("hq-off").icon(IconName::Close).xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_high_quality(w, cx)))),
            );
        }
        if let Some(a) = self.ui.analysis.clone() {
            let swatch = |c: [u8; 4], label: String| {
                h_flex()
                    .gap_2()
                    .child(div().size(px(12.)).rounded_sm().bg(gpui_kit::Rgba { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a: 1.0 }))
                    .child(div().text_xs().text_color(pal.text).child(label))
            };
            col = col.child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(a.title().to_uppercase()))
                    .child(Button::new("analysis-off").icon(IconName::Close).xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, w, cx| {
                        this.ui.analysis = None;
                        let d = this.display.clone();
                        this.apply_display(d, w, cx);
                    }))),
            );
            match a {
                Analysis::Curvature => {
                    col = col
                        .child(swatch(ramp(1.0), "High curvature (small radius)".into()))
                        .child(swatch(ramp(0.5), "Medium".into()))
                        .child(swatch(ramp(0.0), "Flat".into()));
                }
                Analysis::Draft { angle_deg, .. } => {
                    let step = |id: &'static str, label: &'static str, d: f64, cx: &mut Context<Self>| {
                        Button::new(id).label(label).xsmall().ghost().on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                            if let Some(Analysis::Draft { angle_deg, .. }) = &mut this.ui.analysis {
                                *angle_deg = (*angle_deg + d).clamp(0.0, 89.0);
                            }
                            let disp = this.display.clone();
                            this.apply_display(disp, w, cx);
                        }))
                    };
                    col = col
                        .child(
                            h_flex()
                                .gap_1()
                                .child(div().text_xs().text_color(pal.text_muted).child("Draft angle"))
                                .child(step("draft-m", "−", -0.5, cx))
                                .child(div().text_xs().text_color(pal.text).child(format!("{angle_deg:.1}°")))
                                .child(step("draft-p", "+", 0.5, cx))
                                .child(Button::new("draft-dir").label("Pull: selection").xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, w, cx| {
                                    let angle = match &this.ui.analysis {
                                        Some(Analysis::Draft { angle_deg, .. }) => *angle_deg,
                                        _ => 3.0,
                                    };
                                    if let Analysis::Draft { pull, .. } = this.draft_analysis() {
                                        this.ui.analysis = Some(Analysis::Draft { pull, angle_deg: angle });
                                    }
                                    let d = this.display.clone();
                                    this.apply_display(d, w, cx);
                                }))),
                        )
                        .child(swatch(DRAFT_POSITIVE, format!("Positive draft ≥ {angle_deg:.1}°")))
                        .child(swatch(DRAFT_NEEDED, "Requires draft".into()))
                        .child(swatch(DRAFT_NEGATIVE, format!("Negative draft ≤ −{angle_deg:.1}°")));
                }
            }
        }
        Some(col.into_any_element())
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
