//! Drawing editor: Onshape's Hatch region dialog. Sketch a closed profile
//! with the sketch tools at the top of the dialog, or click inside a closed
//! sketch profile; then Standard (ANSI / ISO / General), Pattern, Scale
//! (Auto or 1–10), Angle (Auto or 0–180° in 15° steps) and Color (the
//! drawing's, or a custom one). The checkmark adds (or, from "Edit hatch",
//! updates) the region.

use crate::drawing_sketch::SkTool;
use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::sheet_sketch::{HATCH_PATTERNS, HatchRegion};
use noshape_sketch::DVec2;

/// A closed profile: outer boundary and holes (sheet mm).
pub type Profile = (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>);
/// A dialog setting change.
type SetFn = std::rc::Rc<dyn Fn(&mut HatchDlg)>;
/// A dropdown item: label, checked, action.
type Item = (String, bool, SetFn);

/// Hatch region dialog state.
#[derive(Clone, Debug)]
pub struct HatchDlg {
    /// Editing this region (None: a new one).
    pub edit: Option<u64>,
    pub standard: usize,
    pub pattern: usize,
    /// None = Auto (1).
    pub scale: Option<u32>,
    /// None = Auto (the pattern's 45°).
    pub angle: Option<u32>,
    /// Use the drawing's hatch color, else `color`.
    pub drawing_color: bool,
    pub color: Option<[u8; 3]>,
    /// The picked profile: outer boundary and holes (sheet mm).
    pub profile: Option<Profile>,
    /// The profile is a region of this view (view coordinates).
    pub view: Option<noshape_model::drawing::ViewId>,
}

impl HatchDlg {
    fn region(&self, id: u64) -> Option<HatchRegion> {
        let (boundary, holes) = self.profile.clone()?;
        Some(HatchRegion {
            id,
            boundary,
            holes,
            view: self.view,
            standard: self.standard,
            pattern: self.pattern,
            scale: self.scale.unwrap_or(1) as f64,
            angle: self.angle.map(|a| a as f64 - 45.0).unwrap_or(0.0),
            color: if self.drawing_color { None } else { self.color.or(Some([0, 0, 0])) },
        })
    }
}

impl DrawingEditor {
    /// Toolbar Hatch region / context menu "Edit hatch".
    pub fn open_hatch_dialog(&mut self, edit: Option<u64>, cx: &mut Context<Self>) {
        if self.hatch_dlg.is_some() && edit.is_none() {
            self.hatch_dlg = None;
            self.set_preview(None, cx);
            return;
        }
        let existing = edit.and_then(|id| self.sketch().hatches.iter().find(|h| h.id == id).cloned());
        self.hatch_dlg = Some(match existing {
            Some(h) => HatchDlg {
                edit,
                standard: h.standard,
                pattern: h.pattern,
                scale: Some(h.scale.round() as u32).filter(|s| *s != 1),
                angle: (h.angle != 0.0).then_some((h.angle + 45.0).rem_euclid(180.0).round() as u32),
                drawing_color: h.color.is_none(),
                color: h.color,
                profile: Some((h.boundary, h.holes)),
                view: h.view,
            },
            None => HatchDlg { edit: None, standard: 0, pattern: 0, scale: None, angle: None, drawing_color: true, color: None, profile: None, view: None },
        });
        self.tool = Tool::Select;
        self.status = Some("Hatch region: sketch a closed profile or click inside one".into());
        self.hatch_preview(cx);
    }

    /// A click while the dialog is open (no sketch tool): pick the closed
    /// sketch profile under the cursor. True when used.
    pub fn hatch_click(&mut self, cx: &mut Context<Self>) -> bool {
        if self.hatch_dlg.is_none() || !matches!(self.tool, Tool::Select) {
            return false;
        }
        let c = DVec2::new(self.cursor[0], self.cursor[1]);
        let regions = self.sketch().sketch.regions();
        let pts = |l: &noshape_sketch::ProfileLoop| l.tessellate(0.05).into_iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
        // The smallest closed sketch profile containing the point, else the
        // smallest closed region of the view's visible edges.
        let picked = match regions.iter().filter(|r| r.contains(c)).min_by(|a, b| a.area().abs().total_cmp(&b.area().abs())) {
            Some(r) => Some(((pts(&r.outer), r.holes.iter().map(pts).collect()), None)),
            None => self.view_region_at(self.cursor),
        };
        let Some((profile, view)) = picked else {
            self.status = Some("Click inside a closed sketch profile or a region of a view".into());
            cx.notify();
            return true;
        };
        if let Some(d) = &mut self.hatch_dlg {
            d.profile = Some(profile);
            d.view = view;
        }
        self.status = None;
        self.hatch_preview(cx);
        true
    }

    /// The closed region of a view's visible edges around a sheet point
    /// (view coordinates). Detail and break views take no hatches.
    fn view_region_at(&mut self, s: [f64; 2]) -> Option<(Profile, Option<noshape_model::drawing::ViewId>)> {
        use noshape_model::drawing::ViewKind;
        let v = self.view_at(s)?;
        let view = self.current_sheet().view(v)?;
        if matches!(view.kind, ViewKind::Detail { .. }) || !view.breaks.is_empty() {
            self.status = Some("Hatches can't be added to detail or break views".into());
            return None;
        }
        let pl = self.placement(v)?;
        let g = self.geoms.get(&v)?;
        let mut sk = noshape_sketch::Sketch::new();
        let mut n = 0;
        for e in g.edges.iter().filter(|e| e.visible) {
            for w in e.points.windows(2) {
                if ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt() > 1e-9 {
                    sk.add_line(DVec2::new(w[0][0], w[0][1]), DVec2::new(w[1][0], w[1][1]));
                    n += 1;
                }
            }
            if n > 6000 {
                break;
            }
        }
        let p = pl.to_view(s);
        let c = DVec2::new(p[0], p[1]);
        let regions = sk.regions();
        let r = regions.iter().filter(|r| r.contains(c)).min_by(|a, b| a.area().abs().total_cmp(&b.area().abs()))?;
        let pts = |l: &noshape_sketch::ProfileLoop| l.tessellate(1e-5).into_iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
        Some(((pts(&r.outer), r.holes.iter().map(pts).collect()), Some(v)))
    }

    /// Show the hatch in the preview while the dialog is open.
    fn hatch_preview(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.hatch_dlg else { return };
        let id = d.edit.unwrap_or_else(|| self.sketch().next_hatch_id());
        let preview = d.region(id).map(|h| {
            let mut dr = self.drawing.clone();
            let hs = &mut dr.sheets[self.sheet].sketch.hatches;
            hs.retain(|x| x.id != id);
            hs.push(h);
            dr
        });
        self.set_preview(preview, cx);
        cx.notify();
    }

    fn accept_hatch(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.hatch_dlg.take() else { return };
        self.preview = None;
        let id = d.edit.unwrap_or_else(|| self.sketch().next_hatch_id());
        if let Some(h) = d.region(id) {
            self.edit_sheet(if d.edit.is_some() { "Edit hatch" } else { "Hatch region" }, cx, |s| {
                s.sketch.hatches.retain(|x| x.id != id);
                s.sketch.hatches.push(h);
            });
        } else {
            self.refresh(cx);
        }
        self.status = None;
        cx.notify();
    }

    pub fn cancel_hatch(&mut self, cx: &mut Context<Self>) -> bool {
        if self.hatch_dlg.take().is_none() {
            return false;
        }
        self.set_preview(None, cx);
        true
    }

    pub fn render_hatch_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.hatch_dlg.clone()?;
        let pal = p();
        let ed = cx.entity();
        let set = |f: SetFn, ed: &Entity<DrawingEditor>| {
            let ed = ed.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let f = f.clone();
                ed.update(cx, |t, cx| {
                    if let Some(d) = &mut t.hatch_dlg {
                        f(d);
                    }
                    t.hatch_preview(cx);
                })
            }
        };
        let select = |id: &'static str, label: String, items: Vec<Item>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().small().w(px(130.)).dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(set(f.clone(), &ed)));
                }
                m
            })
        };
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(30.)).px(px(6.)).child(div().w(px(80.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        let rc = |f: Box<dyn Fn(&mut HatchDlg)>| -> SetFn { std::rc::Rc::from(f) };
        let standards: Vec<Item> = HATCH_PATTERNS
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (s.0.to_string(), i == d.standard, rc(Box::new(move |d: &mut HatchDlg| {
                    d.standard = i;
                    d.pattern = 0;
                })))
            })
            .collect();
        let patterns: Vec<Item> = HATCH_PATTERNS[d.standard].1.iter().enumerate().map(|(i, n)| (n.to_string(), i == d.pattern, rc(Box::new(move |d: &mut HatchDlg| d.pattern = i)))).collect();
        let scales: Vec<Item> = std::iter::once(("Auto".to_string(), d.scale.is_none(), rc(Box::new(|d: &mut HatchDlg| d.scale = None))))
            .chain((1..=10u32).map(|k| (format!("{k}.0"), d.scale == Some(k), rc(Box::new(move |d: &mut HatchDlg| d.scale = Some(k))))))
            .collect();
        let angles: Vec<Item> = std::iter::once(("Auto".to_string(), d.angle.is_none(), rc(Box::new(|d: &mut HatchDlg| d.angle = None))))
            .chain((0..=12u32).map(|k| k * 15).map(|a| (format!("{a}.0 deg"), d.angle == Some(a), rc(Box::new(move |d: &mut HatchDlg| d.angle = Some(a))))))
            .collect();
        let colors: Vec<Item> = crate::drawing_drawprops::COLORS.iter().map(|&(n, c)| (n.to_string(), d.color == c, rc(Box::new(move |d: &mut HatchDlg| d.color = c)))).collect();
        let tools = [(SkTool::Line, "line", "Line (l)"), (SkTool::Circle, "circle", "Center point circle (c)"), (SkTool::CornerRect, "corner-rectangle", "Corner rectangle (g)"), (SkTool::CenterRect, "center-rectangle", "Center point rectangle (r)"), (SkTool::Spline, "spline", "Spline")];
        let cur = match &self.tool {
            Tool::Sketch { tool, .. } => Some(*tool),
            _ => None,
        };
        let mut tool_row = h_flex().px(px(4.)).py(px(3.)).gap(px(1.));
        for (t, icon, tip) in tools {
            tool_row = tool_row.child(crate::ui::tool_button(SharedString::from(format!("hd-{icon}")), icon, cur == Some(t), cx).tooltip(tip).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_tool(Tool::Sketch { tool: t, pts: vec![], from: None }, cx);
            })));
        }
        let color_label = crate::drawing_drawprops::COLORS.iter().find(|c| c.1 == d.color).map(|c| c.0).unwrap_or("Black").to_string();
        Some(
            v_flex()
                .id("dr-hatch-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(250.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Hatch region"))
                        .child(
                            div()
                                .id("hd-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .when(d.profile.is_none(), |e| e.opacity(0.2))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_hatch(cx))),
                        )
                        .child(
                            div()
                                .id("hd-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::ui::glyph("close", 14., pal.cancel))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.cancel_hatch(cx);
                                    cx.notify();
                                })),
                        ),
                )
                .child(tool_row)
                .child(div().px(px(6.)).text_size(px(12.)).text_color(pal.text_muted).child(if d.profile.is_some() { "Profile selected" } else { "Sketch or select a closed profile" }))
                .child(row("Standard", select("hd-std", HATCH_PATTERNS[d.standard].0.to_string(), standards).into_any_element()))
                .child(row("Pattern", select("hd-pat", HATCH_PATTERNS[d.standard].1[d.pattern].to_string(), patterns).into_any_element()))
                .child(row("Scale", select("hd-scale", d.scale.map(|k| format!("{k}.0")).unwrap_or_else(|| "Auto".into()), scales).into_any_element()))
                .child(row("Angle", select("hd-angle", d.angle.map(|a| format!("{a}.0 deg")).unwrap_or_else(|| "Auto".into()), angles).into_any_element()))
                .child(
                    h_flex()
                        .h(px(30.))
                        .px(px(6.))
                        .gap(px(6.))
                        .child(div().w(px(80.)).text_size(px(12.)).child("Color"))
                        .child(Checkbox::new("hd-dcolor").label("Drawing properties").small().checked(d.drawing_color).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some(d) = &mut this.hatch_dlg {
                                d.drawing_color = *v;
                            }
                            this.hatch_preview(cx);
                        }))),
                )
                .when(!d.drawing_color, |e| e.child(row("", select("hd-color", color_label, colors).into_any_element())))
                .into_any_element(),
        )
    }
}
