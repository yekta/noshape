//! FeatureScript reference parameters (`TableData`, `ImageData`,
//! `JSONData`, `TextData`, `PartStudioData`…) in the feature dialog: the
//! field (label above a 214×27 box with the type icon and "Select CSV
//! file…" or the chosen name) and the 300×559 "Select …" panel that opens
//! beside the dialog (live dom/fs-refparams*.txt, part-studio/481–486).

use crate::feature_dialog::{DialogEvent, FeatureDialog};
use crate::select_item::{PanelMode, PanelSetup, SelectItemEvent, SelectItemPanel};
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{Document, TabContent};
use noshape_model::file_tab::FileKind;
use noshape_model::params::{ParamValue, ReferenceKind};

const BLUE: u32 = 0x1651b0;

/// A tab a reference parameter can point at.
#[derive(Clone, Debug)]
pub struct RefSource {
    pub tab: String,
    pub name: String,
    pub what: RefSourceKind,
    /// A configured Part Studio's configuration (edited in the panel).
    pub configuration: Option<noshape_model::configuration::Configuration>,
}

#[derive(Clone, Debug)]
pub enum RefSourceKind {
    File(FileKind, String),
    PartStudio(Vec<(noshape_model::BodyId, String)>),
    VariableStudio,
}

/// The open "Select …" panel of a reference parameter.
pub struct RefPicker {
    pub path: String,
    panel: Entity<SelectItemPanel>,
    _sub: Subscription,
}

/// A stored `ext:` value's part-less key (`ext:mv|tab#part|path` → `ext:mv|tab|path`).
fn ext_key(value: &str) -> Option<crate::derived::ExternalRef> {
    let value = value.split('\t').next().unwrap_or(value);
    let rest = value.strip_prefix("ext:")?;
    let mut it = rest.splitn(3, '|');
    let (mv, tab, path) = (it.next()?, it.next()?, it.next()?);
    crate::derived::ExternalRef::parse(&format!("ext:{mv}|{}|{path}", source_tab(tab)))
}

/// The name a stored value shows: a tab of this document, or of another
/// document or version (the field shows only the file name, live 610, 628).
fn value_name(sources: &[RefSource], value: &str) -> Option<String> {
    if let Some(r) = ext_key(value) {
        let doc = crate::derived::external_snapshot(&r.path, r.microversion).ok()?;
        return doc.tabs.iter().find(|t| t.id.to_string() == r.tab).map(|t| t.name.clone());
    }
    sources.iter().find(|s| s.tab == source_tab(value.split('\t').next().unwrap_or(value))).map(|s| s.name.clone())
}

/// A Part Studio's parts, then its sketches (live 741: "Part 1", "Sketch 1";
/// a sketch is listed as `assembly::eval::sketch_body_id`).
pub fn parts_and_sketches(ctx: &noshape_model::regen::context::Context) -> Vec<(noshape_model::BodyId, String)> {
    let parts = ctx.bodies.iter().map(|b| (b.id.clone(), b.name.clone()));
    let sketches = ctx.sketches.iter().map(|s| (noshape_model::assembly::eval::sketch_body_id(s.feature), s.name.clone()));
    parts.chain(sketches).collect()
}

/// The file tabs and studios of `doc` (Part Studios without `except`,
/// the one being edited), sorted by name like Onshape's lists.
pub fn sources(doc: &Document, except: &str, parts: impl Fn(&str) -> Vec<(noshape_model::BodyId, String)>) -> Vec<RefSource> {
    let mut out: Vec<RefSource> = doc
        .tabs
        .iter()
        .filter(|t| t.id.to_string() != except)
        .filter_map(|t| {
            let what = match &t.content {
                TabContent::File(f) => RefSourceKind::File(f.kind(), f.ext()),
                TabContent::PartStudio(_) => RefSourceKind::PartStudio(parts(&t.id.to_string())),
                TabContent::VariableStudio(_) => RefSourceKind::VariableStudio,
                _ => return None,
            };
            let configuration = match &t.content {
                TabContent::PartStudio(ps) if !ps.configuration.inputs.is_empty() => Some(ps.configuration.clone()),
                _ => None,
            };
            Some(RefSource { tab: t.id.to_string(), name: t.name.clone(), what, configuration })
        })
        .collect();
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

pub(crate) fn accepts(kind: ReferenceKind, s: &RefSource) -> bool {
    match (&s.what, kind) {
        (RefSourceKind::File(_, ext), ReferenceKind::Csv) => ext == "csv",
        (RefSourceKind::File(k, _), ReferenceKind::Image) => *k == FileKind::Image,
        (RefSourceKind::File(_, ext), ReferenceKind::Json) => ext == "json",
        (RefSourceKind::File(_, ext), ReferenceKind::Text) => ext == "txt",
        (RefSourceKind::File(k, _), ReferenceKind::CadImport) => *k == FileKind::Cad,
        (RefSourceKind::PartStudio(_), ReferenceKind::PartStudio) => true,
        (RefSourceKind::VariableStudio, ReferenceKind::VariableStudio) => true,
        _ => false,
    }
}

/// Field icon and placeholder (live dom/fs-refparams-dialog-empty).
fn texts(kind: ReferenceKind) -> (&'static str, &'static str) {
    match kind {
        ReferenceKind::Csv => ("file", "Select CSV file…"),
        ReferenceKind::Image => ("image", "Select image…"),
        ReferenceKind::Json => ("file", "Select JSON file…"),
        ReferenceKind::Text => ("file", "Select text file…"),
        ReferenceKind::PartStudio => ("part-studio", "Select Part Studio…"),
        ReferenceKind::VariableStudio => ("variable-studio", "Select Variable Studio…"),
        ReferenceKind::CadImport => ("file-cad", "Select CAD file…"),
    }
}

/// The stored value: the tab, or `tab#part` for one part of a Part Studio.
fn source_tab(value: &str) -> &str {
    value.split_once('#').map_or(value, |(t, _)| t)
}

/// The value of one part of Part Studio `key`: `tab#part`, or for another
/// document / version `ext:mv|tab#part|path`.
pub(crate) fn part_value(key: &str, body: &noshape_model::BodyId) -> String {
    match crate::derived::ExternalRef::parse(key) {
        Some(r) => format!("ext:{}|{}#{body}|{}", r.microversion, r.tab, r.path.display()),
        None => format!("{key}#{body}"),
    }
}

impl FeatureDialog {
    /// The parameter row: label, then the bordered field (blue while its
    /// panel is open).
    pub(crate) fn render_reference_field(&mut self, label: &'static str, path: &str, kind: ReferenceKind, value: &str, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (icon, placeholder) = texts(kind);
        let open = self.ref_picker.as_ref().is_some_and(|r| r.path == path);
        let chosen = value_name(&self.ref_sources, value);
        let empty = chosen.is_none();
        // Live 611: "Table: fsrefs.csv".
        let tip = chosen.as_ref().map(|c| format!("{label}: {c}"));
        let path_s = path.to_string();
        v_flex()
            .mx(px(1.))
            .mt(px(5.))
            .child(div().h(px(22.)).px(px(7.)).text_size(px(12.)).line_height(px(21.6)).text_color(gpui_kit::rgb(0x666666)).child(label))
            .child(
                h_flex()
                    .id(SharedString::from(format!("ref-{path}")))
                    .h(px(27.))
                    .p(px(2.))
                    .gap(px(4.))
                    .rounded(px(2.))
                    .border_1()
                    .border_color(gpui_kit::rgb(if open { BLUE } else { 0xd7d7d7 }))
                    .cursor_pointer()
                    .when_some(tip, |d, t| d.tooltip(crate::ui::tooltip(t)))
                    .child(div().when(empty, |d| d.opacity(0.6)).child(crate::assets::icon(icon, 20., cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(14.))
                            .line_height(px(21.))
                            .text_color(pal.text)
                            .when(empty, |d| d.opacity(0.6))
                            .when(open, |d| d.font_weight(FontWeight::MEDIUM))
                            .child(chosen.unwrap_or_else(|| placeholder.to_string())),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.ref_picker.as_ref().is_some_and(|r| r.path == path_s) {
                            this.ref_picker = None;
                        } else {
                            this.open_reference_picker(path_s.clone(), kind, window, cx);
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn open_reference_picker(&mut self, path: String, kind: ReferenceKind, window: &mut Window, cx: &mut Context<Self>) {
        // Derived's source (psb): the studio, part and configuration params.
        let derived = self.ty.id == "importDerived" && path == "partStudio";
        let value = if derived { self.derived_reference_value() } else { self.values_text(&path) };
        let setup = PanelSetup {
            mode: PanelMode::Reference(kind),
            sources: self.ref_sources.clone(),
            doc_name: self.ref_document_name.clone(),
            doc_path: self.ref_document_path.clone(),
            except: self.ref_except.clone(),
            gpu: self.ref_gpu.clone(),
            value,
        };
        let panel = cx.new(|cx| SelectItemPanel::new(setup, window, cx));
        let p2 = path.clone();
        let sub = cx.subscribe(&panel, move |this: &mut Self, _, ev: &SelectItemEvent, cx| match ev {
            // Picking a file closes the panel (live 483, 610); a Part
            // Studio's stays open to pick its parts (486).
            SelectItemEvent::Picked(v) => {
                if !matches!(kind, ReferenceKind::PartStudio | ReferenceKind::VariableStudio) {
                    this.ref_picker = None;
                }
                if derived {
                    this.pick_derived_source(v, cx);
                } else {
                    this.set_value(&p2, ParamValue::Text(v.clone()), cx);
                }
            }
            SelectItemEvent::Close => {
                this.ref_picker = None;
                cx.notify();
            }
            SelectItemEvent::CreateVersion(p) => cx.emit(DialogEvent::CreateVersion(p.clone())),
            SelectItemEvent::ImportFile => this.ref_import = Some(kind),
        });
        self.ref_picker = Some(RefPicker { path, panel, _sub: sub });
    }

    /// The 300×559 "Select …" panel right of the dialog, or nothing.
    pub(crate) fn render_reference_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(kind) = self.ref_import.take() {
            self.import_reference(kind, window, cx);
        }
        let picker = self.ref_picker.as_ref()?;
        // Pictures rendered after the panel opened.
        let thumbs = &self.ref_thumbs;
        picker.panel.update(cx, |p, _| {
            if p.thumbs.len() < thumbs.len() {
                p.thumbs.extend(thumbs.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        });
        Some(div().absolute().top_0().left_full().ml(px(1.)).child(picker.panel.clone()).into_any_element())
    }

    /// "Import…": choose a file; the Part Studio adds it as a tab and picks it.
    fn import_reference(&mut self, kind: ReferenceKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.ref_picker.as_ref().map(|r| r.path.clone()) else { return };
        let exts: &'static [&'static str] = match kind {
            ReferenceKind::Image => &["png", "jpg", "jpeg", "gif", "PNG", "JPG", "JPEG", "GIF"],
            _ => &["csv", "CSV"],
        };
        let view = cx.entity();
        crate::files::request_open_path("Import", "Files", exts, window, cx, move |file, cx| {
            view.update(cx, |_, cx| cx.emit(DialogEvent::ImportReference { param: path, file }));
        });
    }

    fn values_text(&self, path: &str) -> String {
        match self.feature.params.get(path) {
            Some(ParamValue::Text(t)) => t.clone(),
            _ => String::new(),
        }
    }
}

/// 70×40 pictures of Part Studios and their parts for the panel, keyed like
/// the stored values (`tab`, `tab#part`): each studio regenerated and
/// rendered with the document-thumbnail look (Onshape's select-item rows).
pub async fn render_thumbnails(gpu: crate::gpu::Gpu, doc: Document, tabs: Vec<noshape_model::document::TabId>, key: impl Fn(noshape_model::document::TabId) -> String) -> Vec<(String, std::sync::Arc<RenderImage>)> {
    use noshape_model::display::BodyKind;
    use noshape_model::regen::{RegenOptions, Regenerator};
    let custom = noshape_model::custom::DocumentScripts::from_document(&doc);
    let mut out = Vec::new();
    for tab in tabs {
        let Some(TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else { continue };
        let opts = RegenOptions { custom_features: Some(custom.clone()), ..Default::default() };
        let r = Regenerator::new().regenerate(ps, &ps.configuration.defaults(), &doc.settings.units, &opts);
        let display = &r.display;
        let parts = std::iter::once(None).chain(display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface) && !b.hidden).map(|b| Some(b.id.clone())));
        for part in parts {
            let mut d = display.clone();
            if let Some(p) = &part {
                for b in &mut d.bodies {
                    b.hidden = &b.id != p;
                }
            }
            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
            let mut cache = crate::scene_builder::GpuCache::default();
            let built = crate::scene_builder::build(&d, &opts, &mut cache, gpu.uploader());
            let Some((lo, hi)) = built.scene.bounds() else { continue };
            let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), 140. / 80.);
            let Ok(png) = gpu.render_png(std::sync::Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style(), 140, 80).await else { continue };
            if let Some(img) = decode(&png) {
                let studio = key(tab);
                out.push((part.map_or_else(|| studio.clone(), |p| part_value(&studio, &p)), img));
            }
        }
        // Sketches: each alone, seen normal to its plane.
        for sk in &display.sketches {
            let mut d = display.clone();
            for b in &mut d.bodies {
                b.hidden = true;
            }
            d.sketches.retain(|x| x.feature == sk.feature);
            if let Some(x) = d.sketches.first_mut() {
                x.hidden = false;
            }
            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: true, show_origin: false, ..Default::default() };
            let mut cache = crate::scene_builder::GpuCache::default();
            let built = crate::scene_builder::build(&d, &opts, &mut cache, gpu.uploader());
            let Some((lo, hi)) = built.scene.bounds() else { continue };
            let camera = fit_camera_normal(lo.as_dvec3(), hi.as_dvec3(), 140. / 80., &sk.frame);
            let Ok(png) = gpu.render_png(std::sync::Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style(), 140, 80).await else { continue };
            if let Some(img) = decode(&png) {
                out.push((part_value(&key(tab), &noshape_model::assembly::eval::sketch_body_id(sk.feature)), img));
            }
        }
    }
    out
}

/// A camera looking at the box normal to a sketch plane (its X right).
fn fit_camera_normal(lo: glam::DVec3, hi: glam::DVec3, aspect: f64, frame: &noshape_model::display::PlaneFrame) -> noshape_render::Camera {
    let mut camera = noshape_render::Camera { rotation: noshape_render::Camera::look_rotation(-frame.normal, frame.y), ..Default::default() };
    let (right, up) = (camera.right(), camera.up());
    let center = (lo + hi) * 0.5;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..8 {
        let c = glam::DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }) - center;
        let (x, y) = (c.dot(right), c.dot(up));
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let target = center + right * ((x0 + x1) * 0.5) + up * ((y0 + y1) * 0.5);
    let half_h = ((y1 - y0) * 0.5).max((x1 - x0) * 0.5 / aspect).max(1e-5);
    camera.fit_sphere(target, half_h * 1.1 / 1.15, aspect.max(1.0));
    camera
}

/// PNG -> GPUI image (BGRA), white made transparent so it sits on the
/// selected (blue) row.
fn decode(png: &[u8]) -> Option<std::sync::Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
