//! The Part Studio's View cube and its *Camera and render options* menu; the widget itself is
//! `noshape_app_core::view_cube` (shared with the Assembly).

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::Selectable as _;
use gpui_kit::component::h_flex;
use gpui_kit::*;
use noshape_render::Projection;

pub use noshape_app_core::view_cube::*;

impl PartStudioView {
    /// Animate to a cube cell's view (remembered for Previous view).
    pub(super) fn cube_click(&mut self, cell: CubeCell, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        let rot = cell.rotation();
        self.viewport.update(cx, |vp, cx| vp.animate_to_rotation_fit(rot, cx));
    }

    /// Roll the view about the line of sight (cube arc arrows).
    pub(super) fn roll_view(&mut self, deg: f64, cx: &mut Context<Self>) {
        let dir = if deg > 0.0 { crate::navigation::RotateDir::CounterClockwise } else { crate::navigation::RotateDir::Clockwise };
        self.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg.abs(), cx));
    }

    pub(super) fn render_view_cube(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let hover = self.ui.cube_hover;
        self.viewport.update(cx, |vp, cx| vp.set_view_cube(Some(hover), cx));
        let menu = self.render_camera_menu(cx);
        let hooks = CubeHooks {
            state: |this: &mut Self| (&mut this.ui.cube_hover, &mut this.ui.cube_origin),
            rotation: |this: &Self, cx| this.viewport.read(cx).camera.rotation,
            click: Self::cube_click,
            rotate: |this, dx, dy, cx| this.rotate_by(&(), dx, dy, cx),
            roll: Self::roll_view,
            placed: |this, at, cx| this.viewport.update(cx, |vp, cx| vp.set_view_cube_origin(at, cx)),
        };
        let size = self.viewport.read(cx).size();
        view_cube(menu, hooks, size, cx)
    }

    /// Open Onshape's Graphics preferences dialog.
    pub(super) fn open_graphics_prefs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = cx.new(crate::graphics_prefs_dialog::GraphicsPrefsDialog::new);
        let sub = cx.subscribe(&dialog, |this: &mut Self, _, _: &crate::graphics_prefs_dialog::GraphicsPrefsEvent, cx| {
            this.ui.graphics_dialog = None;
            cx.notify();
        });
        window.focus(&dialog.focus_handle(cx), cx);
        self.ui.graphics_dialog = Some((dialog, sub));
        cx.notify();
    }

    /// The camera menu's picks ([`crate::camera_menu`]).
    fn on_camera_action(&mut self, a: crate::camera_menu::Action, w: &mut Window, cx: &mut Context<Self>) {
        use crate::camera_menu::Action;
        match a {
            Action::View(sv) => self.view(sv, cx),
            Action::GraphicsPrefs => self.open_graphics_prefs(w, cx),
            Action::NamedViews => self.toggle_named_views(w, cx),
            Action::PreviousView => self.previous_view(cx),
            Action::ZoomFit => self.zoom_fit(cx),
            Action::ZoomWindow => self.toggle_zoom_window(cx),
            Action::ZoomSelection => self.zoom_to_selection(cx),
            Action::Perspective => self.viewport.update(cx, |vp, cx| {
                let p = if vp.camera.projection == Projection::Perspective { Projection::Orthographic } else { Projection::Perspective };
                vp.set_projection(p, cx);
            }),
            Action::Orient => {
                self.ui.orient_on_edit = !self.ui.orient_on_edit;
                cx.notify();
            }
            Action::Display(dm) => self.set_display_mode(dm, w, cx),
            Action::HiddenEdges(on) => {
                self.opts.hidden_edges_visible = on;
                let d = self.display.clone();
                self.apply_display(d, w, cx);
            }
            Action::Tangent(t) => {
                self.opts.tangent_edges = t;
                let d = self.display.clone();
                self.apply_display(d, w, cx);
            }
            Action::HighQuality => self.toggle_high_quality(w, cx),
            Action::Boundary => {
                self.opts.highlight_boundary = !self.opts.highlight_boundary;
                let d = self.display.clone();
                self.apply_display(d, w, cx);
            }
            Action::Section if !self.ui.section.is_empty() => self.turn_off_section(w, cx),
            Action::Section => self.toggle_section(w, cx),
            Action::EditSection => self.edit_section(w, cx),
        }
    }

    /// Onshape's "Camera and render options" dropdown (under the cube).
    fn render_camera_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let mode = self.opts.display_mode;
        let perspective = self.viewport.read(cx).camera.projection == Projection::Perspective;
        let has_selection = !self.selection.is_empty() || !self.selected_features.is_empty();
        let section = !self.ui.section.is_empty();
        // Live (psb 10-21 f): no Section view item while a feature dialog is open.
        let dialog_open = self.edit.is_some();
        let has_history = !self.ui.cam_history.is_empty();
        let orient = self.ui.orient_on_edit;
        let hidden_edges = self.opts.hidden_edges_visible;
        let tangent = self.opts.tangent_edges;
        let high_quality = self.ui.high_quality;
        let boundary = self.opts.highlight_boundary;
        let state = crate::camera_menu::State { perspective, has_history, has_selection, mode, hidden_edges, tangent, high_quality, section_on: section, section_dialog: self.ui.section_dialog.is_some(), show_section: !dialog_open, orient: Some(orient), boundary: Some(boundary) };
        // Onshape: the view-mode cube icon with a ▾ under the cube.
        let pal = p();
        // Live (part-studio/21, 53): no fill at rest, #eaeaea on hover,
        // #e5e5e5 while its menu is open; an 8x4 #333 caret.
        let open = self.ui.camera_menu.as_ref().is_some_and(|m| crate::os_menu::is_open_menu(m, cx));
        let variant = gpui_kit::component::button::ButtonCustomVariant::new(cx).color(with_alpha(pal.panel, 0.35).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(gpui_kit::rgb(0xe5e5e5).into()).shadow(false);
        let button = Button::new("camera-menu")
            .custom(variant)
            // The open style paints the variant's active colour.
            .open(open)
            .child(h_flex().gap(px(4.)).child(crate::assets::icon("view-cube", 20., cx)).child(crate::ui::glyph("camera-caret", 12., gpui_kit::rgb(0x333333))))
            .w(px(44.))
            .h(px(28.))
            .p_0()
            .rounded(px(3.))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // Opens under the button, right-aligned with it (live: 222 px,
                // Onshape's context-menu style). The button's bottom-right
                // corner, from the widget's painted origin.
                // Live (part-studio/53): the menu's right border at the
                // widget's right edge + 1.
                let at = this.ui.cube_origin.map(|o| point(o.x + px(WIDGET_PX + 1.), o.y + px(156.))).unwrap_or_default();
                let view = view.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| crate::camera_menu::build(m.camera(), state, &view, Self::on_camera_action, window, cx));
                this.ui.camera_menu = Some(menu.downgrade());
                crate::os_menu::open_anchored(at, Anchor::TopRight, menu, window, cx);
            }));
        // Live (part-studio/21): the tooltip sits left of the button.
        crate::ui::with_tip(div().id("camera-menu-tip").child(button), "Camera and render options", crate::ui::TipSide::Left).into_any_element()
    }
}
