//! Analysis tools › Reflection analysis… in an assembly (help
//! View__reflection_analysis: "Available in: Sketch, Part Studio, Assembly";
//! live analysis/10–12, dom/dialog-reflection-analysis.txt): the Part
//! Studio's 220 px dialog (Environment, Rotation angle + opposite, Show
//! edges, Blur environment, Lock environment) and its 300 px "Select image"
//! panel beside it (Current document | Other documents | Libraries). Picking
//! an image applies it at once; ✓ keeps the reflection on after the dialog,
//! ✕ / Esc turns it off.

use super::*;
use noshape_scene::environment_maps;
use crate::reflection::{EnvImageRef, ReflectionSettings};

/// The open Reflection analysis dialog.
pub struct ReflectionDialog {
    pub settings: ReflectionSettings,
    rotation: Entity<crate::quantity_field::QuantityInput>,
    /// The "Select image" panel: tab (0 Current document, 1 Other
    /// documents, 2 Libraries), the library image shown, its search box.
    picker: Option<(u8, &'static str, Entity<InputState>)>,
    _sub: Subscription,
}

impl AssemblyView {
    pub(super) fn open_reflection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Re-opening keeps the last environment (live analysis/36); edges
        // start hidden (live analysis/10).
        let mut settings = self.ui.reflection.clone().or_else(|| self.ui.reflection_last.clone()).unwrap_or_default();
        settings.show_edges = self.ui.reflection.as_ref().is_some_and(|r| r.show_edges);
        // Onshape's number field, as the Part Studio's: a commit applies.
        use crate::quantity_field::{QuantityEvent, QuantityInput, QuantitySpec};
        let units = self.units(cx);
        let variables = self.variable_scope(cx);
        let rotation = cx.new(|cx| {
            let mut q = QuantityInput::new(format!("{} deg", settings.rotation_deg), QuantitySpec::quantity(noshape_units::Dim::ANGLE).label("Rotation angle"), units, window, cx);
            q.set_variables(variables, cx);
            q
        });
        let sub = cx.subscribe(&rotation, |this: &mut Self, st, ev: &QuantityEvent, cx| {
            if let QuantityEvent::Committed(_) = ev
                && let Some(v) = st.read(cx).value()
                && let Some(d) = &mut this.ui.reflection_dialog
            {
                d.settings.rotation_deg = v.to_degrees();
                this.rebuild_scene(false, cx);
            }
        });
        self.ui.reflection_dialog = Some(ReflectionDialog { settings, rotation, picker: None, _sub: sub });
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    pub(super) fn close_reflection(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.ui.reflection_dialog.take() else { return };
        if d.settings.image.is_some() {
            self.ui.reflection_last = Some(d.settings.clone());
        }
        self.ui.reflection = (accept && d.settings.image.is_some()).then_some(d.settings);
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// The reflection on display (the open dialog's, else the accepted one).
    pub(super) fn active_reflection(&self) -> Option<ReflectionSettings> {
        self.ui.reflection_dialog.as_ref().map(|d| d.settings.clone()).or_else(|| self.ui.reflection.clone())
    }

    /// The renderer's reflection: the environment decoded and uploaded once
    /// per image.
    pub(super) fn scene_reflection(&self) -> Option<noshape_render::Reflection> {
        let s = self.active_reflection()?;
        let image = s.image.as_ref()?;
        let key = image.key();
        let mut cache = self.ui.env_gpu.borrow_mut();
        if cache.as_ref().is_none_or(|(k, _)| *k != key) {
            let (width, height, rgba) = image.decode()?;
            *cache = Some((key, self.gpu.uploader().upload_image(&noshape_render::ImageData { corners: [[0.0; 3]; 4], width, height, rgba })));
        }
        let environment = cache.as_ref()?.1.clone();
        let rotation = (s.rotation_deg.to_radians() * if s.opposite { -1.0 } else { 1.0 }) as f32;
        Some(noshape_render::Reflection { environment, rotation, blur: s.blur, lock: s.lock })
    }

    fn set_environment(&mut self, image: EnvImageRef, close_picker: bool, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.ui.reflection_dialog {
            d.settings.image = Some(image);
            if close_picker {
                d.picker = None;
            }
        }
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    /// Images of this document: those inserted in sketches.
    fn document_images(&self, cx: &App) -> Vec<EnvImageRef> {
        let doc = &self.doc.read(cx).session.doc;
        let mut out: Vec<EnvImageRef> = Vec::new();
        for tab in &doc.tabs {
            let TabContent::PartStudio(ps) = &tab.content else { continue };
            for f in &ps.features {
                let Some(noshape_model::ParamValue::Sketch(data)) = f.params.get("sketch") else { continue };
                for img in &data.images {
                    let Some(bytes) = noshape_model::sketch_feature::base64_decode(&img.data_base64) else { continue };
                    let r = EnvImageRef::Encoded { name: img.name.clone(), bytes: bytes.into() };
                    if !out.iter().any(|o| o.name() == r.name()) {
                        out.push(r);
                    }
                }
            }
        }
        out
    }

    /// The dialog, with the "Select image" panel beside it when open.
    pub(super) fn render_reflection_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.reflection_dialog.as_ref()?;
        let pal = p();
        let s = d.settings.clone();
        let picking = d.picker.is_some();
        let rotation = d.rotation.clone();
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut ReflectionSettings), cx: &mut Context<Self>| {
            crate::os_ui::checkbox(id, label, on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.ui.reflection_dialog {
                    f(&mut d.settings);
                }
                this.rebuild_scene(false, cx);
                cx.notify();
            }))
        };
        let dialog = v_flex()
            .id("arefl-dialog")
            .occlude()
            .w(px(220.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(29.))
                    .pl(px(6.))
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xd7d7d7))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Reflection analysis"))
                    .child(
                        div()
                            .id("arefl-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(pal.accept)
                            .cursor_pointer()
                            .child(crate::assets::icon("dialog-ok", 24., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_reflection(true, cx))),
                    )
                    .child(
                        div()
                            .id("arefl-cancel")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_reflection(false, cx))),
                    ),
            )
            .child(
                div().px(px(3.)).pt(px(5.)).child(
                    h_flex()
                        .id("arefl-env")
                        .min_h(px(26.))
                        .px(px(6.))
                        .gap(px(6.))
                        .rounded(px(2.))
                        .border_1()
                        .border_color(if picking { pal.accent } else { rgb(0xd7d7d7) })
                        .when(picking, |d| d.bg(rgb(0xdef1fb)))
                        .cursor_pointer()
                        .child(crate::assets::icon("image", 16., cx))
                        .child(div().flex_1().truncate().text_size(px(13.33)).text_color(if s.image.is_some() { rgb(0x555555) } else { rgb(0x999999) }).child(s.image.as_ref().map_or("Select image…".to_string(), |i| i.name().to_string())))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_env_picker(window, cx))),
                ),
            )
            .child(
                h_flex()
                    .child(div().child(crate::os_ui::param_row_plain("Rotation angle", rotation)))
                    .child(
                        // Live: the 26 px column right of the 194 px row (icon at 443).
                        div()
                            .id("arefl-opposite")
                            .size(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .cursor_pointer()
                            .when(s.opposite, |d| d.bg(rgb(0xeaeaea)))
                            .tooltip(crate::ui::tooltip("Opposite direction"))
                            .child(crate::assets::icon("flip-circular-opposite", 20., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.ui.reflection_dialog {
                                    d.settings.opposite = !d.settings.opposite;
                                }
                                this.rebuild_scene(false, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(check("arefl-edges", "Show edges", s.show_edges, |r| r.show_edges = !r.show_edges, cx))
            .child(check("arefl-blur", "Blur environment", s.blur, |r| r.blur = !r.blur, cx))
            .child(check("arefl-lock", "Lock environment", s.lock, |r| r.lock = !r.lock, cx))
            .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(super::props::help_dot()));
        let picker = self.render_env_picker(cx);
        Some(h_flex().items_start().gap(px(6.)).child(dialog).children(picker).into_any_element())
    }

    fn toggle_env_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &mut self.ui.reflection_dialog else { return };
        if d.picker.take().is_none() {
            let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search by name, folder, or configuration"));
            let choice = match &d.settings.image {
                Some(EnvImageRef::Library(n)) => *n,
                _ => environment_maps::NAMES[0],
            };
            d.picker = Some((2, choice, search));
        }
        cx.notify();
    }

    /// The 300 px "Select image" panel (live analysis/11–12).
    fn render_env_picker(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (tab, choice, search) = self.ui.reflection_dialog.as_ref()?.picker.clone()?;
        let pal = p();
        let query = search.read(cx).value().to_lowercase();
        let tab_btn = |label: &'static str, i: u8, cx: &mut Context<Self>| {
            let on = tab == i;
            div()
                .id(SharedString::from(format!("arefl-tab-{i}")))
                .flex_1()
                .h(px(44.))
                .px(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .text_center()
                .whitespace_normal()
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.emphasis).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(rgb(0xd7d7d7)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(p) = this.ui.reflection_dialog.as_mut().and_then(|d| d.picker.as_mut()) {
                        p.0 = i;
                    }
                    cx.notify();
                }))
        };
        let row = |label: &'static str, value: String| {
            h_flex()
                .h(px(28.))
                .gap_2()
                .text_size(px(13.))
                .child(div().w(px(48.)).text_color(pal.text_muted).child(label))
                .child(h_flex().flex_1().border_b_1().border_color(rgb(0xd7d7d7)).child(div().flex_1().text_color(pal.text).child(value)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., pal.text)))
        };
        let button = |id: &'static str, label: &'static str| div().id(id).px(px(12.)).py(px(4.)).border_1().border_color(rgb(0xbdbdbd)).rounded(px(2.)).text_size(px(13.)).cursor_pointer().hover(|d| d.bg(pal.panel_hover)).child(label);
        let mut body = v_flex().flex_1().p(px(6.)).gap(px(6.)).child(gpui_kit::base::input::input_frame(div().border_1().border_color(rgb(0xbdbdbd)).rounded(px(2.)).child(Input::new(&search).appearance(false).small())));
        match tab {
            2 => {
                let view = cx.entity();
                let names: Vec<&'static str> = environment_maps::NAMES.iter().copied().filter(|n| query.is_empty() || n.to_lowercase().contains(&query)).collect();
                let image_row = h_flex().h(px(28.)).gap_2().text_size(px(13.)).child(div().w(px(48.)).text_color(pal.text_muted).child("Image")).child(
                    div().flex_1().border_b_1().border_color(rgb(0xd7d7d7)).child(
                        Button::new("arefl-image")
                            .ghost()
                            .small()
                            .w_full()
                            .child(h_flex().w_full().child(div().flex_1().text_color(pal.text).child(choice)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., pal.text)))
                            .os_dropdown_menu("arefl-image-dd", true, move |mut m, _, _| {
                                for n in names.clone() {
                                    let v = view.clone();
                                    // Picking an image applies it right away (live 12).
                                    m = m.item(crate::os_menu::OsMenuItem::new(n).checked(n == choice).on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| {
                                            if let Some(p) = this.ui.reflection_dialog.as_mut().and_then(|d| d.picker.as_mut()) {
                                                p.1 = n;
                                            }
                                            this.set_environment(EnvImageRef::Library(n), false, cx);
                                        })
                                    }));
                                }
                                m
                            }),
                    ),
                );
                body = body.child(row("Library", "Onshape images".into())).child(row("Type", "Environment maps".into())).child(image_row);
                if let Some(preview) = self.env_preview(choice) {
                    body = body.child(div().pt(px(24.)).flex().justify_center().child(img(ImageSource::Render(preview)).w(px(240.)).h(px(120.))));
                }
                body = body.child(div().flex_1()).child(h_flex().justify_center().pb(px(6.)).child(button("arefl-select", "Select").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_environment(EnvImageRef::Library(choice), true, cx)))));
            }
            0 => {
                let images: Vec<EnvImageRef> = self.document_images(cx).into_iter().filter(|i| query.is_empty() || i.name().to_lowercase().contains(&query)).collect();
                if images.is_empty() {
                    body = body.child(div().py_2().text_size(px(12.)).text_color(pal.text_muted).child("No images in this document"));
                }
                for (i, image) in images.into_iter().enumerate() {
                    body = body.child(
                        h_flex()
                            .id(SharedString::from(format!("arefl-doc-{i}")))
                            .h(px(26.))
                            .px(px(6.))
                            .gap_2()
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(crate::assets::icon("image", 16., cx))
                            .child(image.name().to_string())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_environment(image.clone(), true, cx))),
                    );
                }
            }
            _ => {
                body = body.child(div().py_2().text_size(px(12.)).text_color(pal.text_muted).child("No images in other documents"));
            }
        }
        Some(
            v_flex()
                .id("arefl-picker")
                .occlude()
                .w(px(300.))
                .h(px(558.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(29.))
                        .pl(px(6.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Select image"))
                        .child(
                            div()
                                .id("arefl-picker-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .opacity(0.5)
                                .cursor_pointer()
                                .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 20., rgb(0x000000)))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(d) = &mut this.ui.reflection_dialog {
                                        d.picker = None;
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .child(h_flex().child(tab_btn("Current document", 0, cx)).child(tab_btn("Other documents", 1, cx)).child(tab_btn("Libraries", 2, cx)))
                .child(body)
                .into_any_element(),
        )
    }

    /// Preview of a library image (decoded once per image).
    fn env_preview(&mut self, name: &'static str) -> Option<Arc<RenderImage>> {
        if self.ui.env_preview.as_ref().is_none_or(|(n, _)| *n != name) {
            let img = environment_maps::generate(name)?;
            let mut buf = image::RgbaImage::from_raw(img.width, img.height, img.rgba)?;
            for px in buf.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
            self.ui.env_preview = Some((name, Arc::new(RenderImage::new(vec![image::Frame::new(buf)]))));
        }
        self.ui.env_preview.as_ref().map(|p| p.1.clone())
    }
}
