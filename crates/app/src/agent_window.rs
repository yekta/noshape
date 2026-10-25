//! What an agent's call asks of the window (live mode): each
//! [`noshape_ops::UiRequest`] done the way its menu item or shortcut does
//! it. A child module of `workspace` (it works on the window's views);
//! what a Part Studio or Assembly does is in `agent_window/studio.rs` and
//! `agent_window/assembly.rs`, child modules of those views.

use super::{TabView, Workspace, request_activate_tab};
use crate::viewport::Viewport;
use gpui_kit::*;
use noshape_ops::window::{CameraMove, SheetMove};
use noshape_render::Projection;
use noshape_scene::camera_ops as co;

impl Workspace {
    /// What an agent's call asked the window to do (live mode), as the menu
    /// items and shortcuts do it.
    pub(super) fn agent_request(&mut self, req: noshape_ops::UiRequest, window: &mut Window, cx: &mut Context<Self>) {
        use noshape_ops::UiRequest as R;
        match req {
            R::ActivateTab(tab) => self.activate(tab, window, cx),
            R::OpenDocument { path, version, external_key } => {
                let opened = match version {
                    Some(mv) => crate::files::open_version_window(path, mv, self.gpu.clone(), cx).map(|(handle, _)| Some(handle)),
                    None => crate::files::open_document_window(path, None, self.gpu.clone(), cx).map(|_| None),
                };
                match opened {
                    Ok(handle) => {
                        // The referenced tab in the opened version (Open linked document).
                        use noshape_model::assembly::eval::ExternalStudios as _;
                        if let (Some(handle), Some(key)) = (handle, external_key)
                            && let Some((_, tab)) = noshape_ops::derived::ExternalDocs.studio(&key)
                        {
                            let _ = handle.update(cx, |_, window, cx| window.defer(cx, move |window, cx| request_activate_tab(tab, window, cx)));
                        }
                    }
                    Err(e) => {
                        self.toast = Some((crate::ui::AlertKind::Error, format!("Could not open the document: {e:#}")));
                        cx.notify();
                    }
                }
            }
            R::TabManager(search) => {
                if search.is_some() != self.tab_manager.is_some() {
                    self.toggle_tab_manager(window, cx);
                }
                if let (Some(text), Some((tm, _))) = (search, &self.tab_manager) {
                    tm.update(cx, |tm, cx| tm.set_search(text, window, cx));
                }
            }
            R::SheetsPanel { tab, open } => {
                if let Some(TabView::Drawing(v)) = self.views.get(&tab) {
                    v.update(cx, |v, cx| {
                        v.sheets_open = open;
                        cx.notify();
                    });
                }
            }
            R::Compare(c) => {
                let doc = self.doc.clone();
                match c {
                    Some(c) => {
                        let side = |s: noshape_ops::window::CompareSide| match s {
                            noshape_ops::window::CompareSide::Workspace(b) => crate::compare_mode::Side::Workspace(b),
                            noshape_ops::window::CompareSide::Entry(mv) => crate::compare_mode::Side::Entry(mv),
                        };
                        if let Err(e) = crate::compare_mode::compare(&doc, (side(c.base), c.base_label), (side(c.target), c.target_label), cx) {
                            self.toast = Some((crate::ui::AlertKind::Error, format!("{e:#}")));
                        }
                    }
                    None => crate::compare_mode::stop(&doc, cx),
                }
                cx.notify();
            }
            R::Repair(r) => {
                let doc = self.doc.clone();
                match r {
                    Some((mv, label)) => {
                        if let Err(e) = crate::repair_mode::open(&doc, mv, label, cx) {
                            self.toast = Some((crate::ui::AlertKind::Error, format!("{e:#}")));
                        }
                    }
                    None => crate::repair_mode::close(&doc, cx),
                }
                cx.notify();
            }
            R::Camera { tab, mv } => match self.views.get(&tab) {
                Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| {
                    if mv == CameraMove::Previous {
                        return v.previous_view(cx);
                    }
                    v.push_camera_history(cx);
                    camera_move(&v.viewport, mv, cx);
                }),
                Some(TabView::Assembly(v)) => v.update(cx, |v, cx| {
                    if mv == CameraMove::Previous {
                        return v.on_camera_action(crate::camera_menu::Action::PreviousView, window, cx);
                    }
                    v.push_camera_history(cx);
                    camera_move(&v.viewport, mv, cx);
                }),
                _ => {}
            },
            R::Display { tab, set } => match self.views.get(&tab) {
                Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| v.agent_display(set, window, cx)),
                Some(TabView::Assembly(v)) => v.update(cx, |v, cx| v.agent_display(set, window, cx)),
                _ => {}
            },
            R::Select { tab, sel } => match self.views.get(&tab) {
                Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| v.agent_select(sel, cx)),
                Some(TabView::Assembly(v)) => v.update(cx, |v, cx| v.agent_select(sel, cx)),
                Some(TabView::Drawing(v)) => v.update(cx, |v, cx| {
                    // The sheet menu's Clear selection.
                    v.selection.clear();
                    v.redraw(cx);
                }),
                _ => {}
            },
            R::List { tab, act } => match self.views.get(&tab) {
                Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| v.agent_list(act, window, cx)),
                Some(TabView::Assembly(v)) => v.update(cx, |v, cx| v.agent_list(act, cx)),
                _ => {}
            },
            R::SheetZoom { tab, mv } => {
                if let Some(TabView::Drawing(v)) = self.views.get(&tab) {
                    v.update(cx, |v, cx| {
                        let s = v.size_px();
                        match mv {
                            SheetMove::Fit => v.zoom_to_fit(cx),
                            SheetMove::Step(f) => v.zoom_by(f, [s[0] / 2.0, s[1] / 2.0], cx),
                            SheetMove::Window(a, b) => {
                                v.zoom_window = Some(Some(([a[0] * s[0], a[1] * s[1]], [b[0] * s[0], b[1] * s[1]])));
                                v.zoom_window_up(cx);
                            }
                        }
                    });
                }
            }
        }
    }
}

/// A 3D view's camera move, animated as the view menus do it.
fn camera_move(vp: &Entity<Viewport>, mv: CameraMove, cx: &mut App) {
    vp.update(cx, |vp, cx| match mv {
        CameraMove::Fit(None) => vp.zoom_to_fit(true, cx),
        CameraMove::Fit(Some(rotation)) => vp.animate_to_rotation_fit(rotation, cx),
        CameraMove::FitBox { lo, hi, sphere } => {
            let to = if sphere {
                co::fit_instances(&vp.camera, lo, hi, vp.size())
            } else {
                let mut to = vp.camera;
                co::fit_camera_to_box(&mut to, lo, hi, vp.size(), vp.fit_inset_left);
                to
            };
            vp.animate_to(to, cx);
        }
        CameraMove::Step { zoom_in } => vp.zoom_step(zoom_in, cx),
        CameraMove::Window(a, b) => {
            let s = vp.size();
            vp.zoom_to_box(a * s, b * s, cx);
        }
        CameraMove::Direction { forward, up } => vp.animate_to_direction(forward, up, cx),
        CameraMove::Perspective(on) => vp.set_projection(if on { Projection::Perspective } else { Projection::Orthographic }, cx),
        CameraMove::Previous => {}
    });
}

