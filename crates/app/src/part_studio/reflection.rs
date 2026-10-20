//! Reflection analysis (Analysis tools menu): the parts drawn as mirrors of
//! an environment image, chosen in Onshape's "Select image" dialog (images
//! of this document, or the Onshape images library's environment maps),
//! turned by a rotation angle, optionally blurred and locked to the camera.

use super::PartStudioView;
use super::environment_maps;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::sync::Arc;

/// An environment image: a library map or an image file (encoded bytes).
#[derive(Clone, Debug, PartialEq)]
pub enum EnvImageRef {
    Library(&'static str),
    Encoded { name: String, bytes: Arc<[u8]> },
}

impl EnvImageRef {
    pub fn name(&self) -> &str {
        match self {
            EnvImageRef::Library(n) => n,
            EnvImageRef::Encoded { name, .. } => name,
        }
    }

    /// Cache key of the decoded image.
    pub(crate) fn key(&self) -> String {
        match self {
            EnvImageRef::Library(n) => format!("lib:{n}"),
            EnvImageRef::Encoded { name, bytes } => format!("img:{name}:{}:{:p}", bytes.len(), bytes.as_ptr()),
        }
    }

    /// Decoded sRGB RGBA8 (width, height, pixels).
    pub(crate) fn decode(&self) -> Option<(u32, u32, Vec<u8>)> {
        match self {
            EnvImageRef::Library(n) => environment_maps::generate(n).map(|i| (i.width, i.height, i.rgba)),
            EnvImageRef::Encoded { bytes, .. } => {
                let img = image::load_from_memory(bytes).ok()?.to_rgba8();
                Some((img.width(), img.height(), img.into_raw()))
            }
        }
    }
}

/// Reflection analysis settings (they stay on after the dialog is accepted).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReflectionSettings {
    pub image: Option<EnvImageRef>,
    pub rotation_deg: f64,
    pub opposite: bool,
    pub show_edges: bool,
    pub blur: bool,
    pub lock: bool,
}

/// The "Select image" dialog.
pub struct EnvPicker {
    /// 0 Current document, 1 Other documents, 2 Libraries.
    pub tab: u8,
    /// The library image shown.
    pub choice: &'static str,
    search: Entity<InputState>,
    preview: Option<(String, Arc<RenderImage>)>,
    _sub: Subscription,
}

impl PartStudioView {
    /// The reflection on display: the open dialog's, else accepted settings.
    pub(super) fn active_reflection(&self) -> Option<ReflectionSettings> {
        match &self.ui.analysis_tool {
            Some(t) if t.kind == super::analysis_tools::ToolKind::Reflection => Some(t.options.reflection.clone()),
            _ => self.ui.reflection.clone(),
        }
    }

    /// The renderer's reflection for the active settings (the environment
    /// is decoded and uploaded once per image).
    pub(super) fn scene_reflection(&mut self) -> Option<noshape_render::Reflection> {
        let s = self.active_reflection()?;
        let image = s.image.as_ref()?;
        let key = image.key();
        if self.ui.env_gpu.as_ref().is_none_or(|(k, _)| *k != key) {
            let (width, height, rgba) = image.decode()?;
            let handle = self.gpu.uploader().upload_image(&noshape_render::ImageData { corners: [[0.0; 3]; 4], width, height, rgba });
            self.ui.env_gpu = Some((key, handle));
        }
        let environment = self.ui.env_gpu.as_ref()?.1.clone();
        let rotation = (s.rotation_deg.to_radians() * if s.opposite { -1.0 } else { 1.0 }) as f32;
        Some(noshape_render::Reflection { environment, rotation, blur: s.blur, lock: s.lock })
    }

    /// Images of this document: those inserted in sketches.
    fn document_images(&self, cx: &App) -> Vec<EnvImageRef> {
        let doc = &self.doc.read(cx).session.doc;
        let mut out: Vec<EnvImageRef> = Vec::new();
        for tab in &doc.tabs {
            let noshape_model::document::TabContent::PartStudio(ps) = &tab.content else { continue };
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

    pub(super) fn open_env_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search by name, folder, or configuration"));
        let sub = cx.subscribe_in(&search, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        let choice = match self.ui.analysis_tool.as_ref().and_then(|t| t.options.reflection.image.clone()) {
            Some(EnvImageRef::Library(n)) => n,
            _ => environment_maps::NAMES[0],
        };
        self.ui.env_picker = Some(EnvPicker { tab: 2, choice, search, preview: None, _sub: sub });
        // Onshape applies the library's first image at once.
        if self.ui.analysis_tool.as_ref().is_some_and(|t| t.options.reflection.image.is_none()) {
            self.apply_environment(EnvImageRef::Library(choice), cx);
        }
        cx.notify();
    }

    /// Show an image on the parts (the picker stays open).
    fn apply_environment(&mut self, image: EnvImageRef, cx: &mut Context<Self>) {
        if let Some(t) = &mut self.ui.analysis_tool {
            t.options.reflection.image = Some(image);
        }
        self.rebuild_scene(cx);
    }

    /// Use an image (Select in the picker, or Upload).
    pub(super) fn choose_environment(&mut self, image: EnvImageRef, cx: &mut Context<Self>) {
        if let Some(t) = &mut self.ui.analysis_tool {
            t.options.reflection.image = Some(image);
        }
        self.ui.env_picker = None;
        self.rebuild_scene(cx);
    }

    fn upload_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::files::request_open_path("Upload image", "Image", &["png", "jpg", "jpeg", "gif", "bmp", "PNG", "JPG", "JPEG"], window, cx, move |path, cx| {
            let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "image".into());
            match std::fs::read(&path) {
                Ok(bytes) if image::load_from_memory(&bytes).is_ok() => view.update(cx, |this, cx| this.choose_environment(EnvImageRef::Encoded { name, bytes: bytes.into() }, cx)),
                _ => view.update(cx, |this, cx| this.flash_info("Could not read the image".into(), cx)),
            }
        });
    }

    /// Preview of a library image (cached for the open picker).
    fn env_preview(&mut self, name: &'static str) -> Option<Arc<RenderImage>> {
        let picker = self.ui.env_picker.as_mut()?;
        if picker.preview.as_ref().is_none_or(|(n, _)| n != name) {
            let img = environment_maps::generate(name)?;
            let mut buf = image::RgbaImage::from_raw(img.width, img.height, img.rgba)?;
            for px in buf.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
            picker.preview = Some((name.to_string(), Arc::new(RenderImage::new(vec![image::Frame::new(buf)]))));
        }
        picker.preview.as_ref().map(|p| p.1.clone())
    }

    /// The "Select image" dialog beside the tool's dialog.
    pub(super) fn render_env_picker(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (tab, choice, search) = {
            let pk = self.ui.env_picker.as_ref()?;
            (pk.tab, pk.choice, pk.search.clone())
        };
        let query = search.read(cx).value().to_lowercase();
        let tab_btn = |label: &'static str, i: u8, cx: &mut Context<Self>| {
            let on = tab == i;
            div()
                .id(SharedString::from(format!("envp-tab-{i}")))
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
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(pk) = &mut this.ui.env_picker {
                        pk.tab = i;
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
                .child(h_flex().flex_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(div().flex_1().text_color(pal.text).child(value)).child(Icon::new(IconName::ChevronDown).with_size(px(12.)).text_color(pal.text_muted)))
        };
        let mut body = v_flex().flex_1().p(px(6.)).gap(px(6.)).child(div().border_1().border_color(gpui_kit::rgb(0xbdbdbd)).rounded(px(2.)).child(Input::new(&search).appearance(false).small()));
        match tab {
            2 => {
                let view = cx.entity();
                let names: Vec<&'static str> = environment_maps::NAMES.iter().copied().filter(|n| query.is_empty() || n.to_lowercase().contains(&query)).collect();
                let image_row = h_flex().h(px(28.)).gap_2().text_size(px(13.)).child(div().w(px(48.)).text_color(pal.text_muted).child("Image")).child(div().flex_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Button::new("envp-image").ghost().small().w_full().child(h_flex().w_full().child(div().flex_1().text_color(pal.text).child(choice)).child(Icon::new(IconName::ChevronDown).with_size(px(12.)).text_color(pal.text_muted))).dropdown_menu(move |mut m, _, _| {
                    for n in names.clone() {
                        let v = view.clone();
                        m = m.item(PopupMenuItem::new(n).checked(n == choice).on_click(move |_, _, cx| {
                            // Picking an image applies it right away (Onshape).
                            v.update(cx, |this, cx| {
                                if let Some(pk) = &mut this.ui.env_picker {
                                    pk.choice = n;
                                }
                                this.apply_environment(EnvImageRef::Library(n), cx);
                            })
                        }));
                    }
                    m
                })));
                body = body.child(row("Library", "Onshape images".into())).child(row("Type", "Environment maps".into())).child(image_row);
                if let Some(img) = self.env_preview(choice) {
                    body = body.child(div().pt(px(24.)).flex().justify_center().child(img_source(img).w(px(240.)).h(px(120.))));
                }
                body = body.child(div().flex_1()).child(
                    h_flex().justify_center().pb(px(6.)).child(
                        div()
                            .id("envp-select")
                            .px(px(12.))
                            .py(px(4.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xbdbdbd))
                            .rounded(px(2.))
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child("Select")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_environment(EnvImageRef::Library(choice), cx))),
                    ),
                );
            }
            0 => {
                let images: Vec<EnvImageRef> = self.document_images(cx).into_iter().filter(|i| query.is_empty() || i.name().to_lowercase().contains(&query)).collect();
                if images.is_empty() {
                    body = body.child(div().py_2().text_size(px(12.)).text_color(pal.text_muted).child("No images in this document"));
                }
                for (i, img) in images.into_iter().enumerate() {
                    body = body.child(
                        h_flex()
                            .id(SharedString::from(format!("envp-doc-{i}")))
                            .h(px(26.))
                            .px(px(6.))
                            .gap_2()
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(crate::assets::icon("image", 16., cx))
                            .child(img.name().to_string())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_environment(img.clone(), cx))),
                    );
                }
                body = body.child(div().flex_1()).child(
                    h_flex().justify_center().pb(px(6.)).child(
                        div()
                            .id("envp-upload")
                            .px(px(12.))
                            .py(px(4.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xbdbdbd))
                            .rounded(px(2.))
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child("Upload")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.upload_environment(window, cx))),
                    ),
                );
            }
            _ => {
                body = body.child(div().py_2().text_size(px(12.)).text_color(pal.text_muted).child("No images in other documents"));
            }
        }
        Some(
            v_flex()
                .id("env-picker")
                .occlude()
                .absolute()
                .top(px(0.))
                .left(px(226.))
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
                                .id("envp-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(Icon::new(IconName::Close).with_size(px(14.)).text_color(pal.text_muted))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.env_picker = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(h_flex().child(tab_btn("Current document", 0, cx)).child(tab_btn("Other documents", 1, cx)).child(tab_btn("Libraries", 2, cx)))
                .child(body)
                .into_any_element(),
        )
    }
}

fn img_source(image: Arc<RenderImage>) -> Img {
    img(ImageSource::Render(image))
}
