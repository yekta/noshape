//! Pictures of a document's Part Studios, Assemblies and Drawings for the
//! tab hover card (live part-studio/17): rendered off screen when a tab is
//! first hovered, again when its content changed.

use crate::doc_model::DocModel;
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::document::{TabContent, TabId};
use std::collections::HashMap;
use std::sync::Arc;

/// Picture size: the card's content box (live 312 x 182), at 2x.
pub const CARD_PICTURE: (u32, u32) = (624, 364);

pub struct TabThumbs {
    doc: Entity<DocModel>,
    /// `None`: the element has nothing to draw (an empty Assembly).
    images: HashMap<TabId, Option<Arc<RenderImage>>>,
    made_for: HashMap<TabId, u64>,
    busy: bool,
    /// Tabs asked for while another picture was being made (the Ctrl+Space
    /// switcher asks for several at once), made in turn.
    queued: Vec<TabId>,
    evaluator: Arc<parking_lot::Mutex<noshape_model::assembly::eval::AssemblyEvaluator>>,
    /// Part Studios not open in a view (incremental across pictures).
    regen: Arc<parking_lot::Mutex<noshape_model::regen::Regenerator>>,
}

impl TabThumbs {
    pub fn new(doc: Entity<DocModel>) -> Self {
        Self { doc, images: HashMap::new(), made_for: HashMap::new(), busy: false, queued: Vec::new(), evaluator: Default::default(), regen: Default::default() }
    }

    /// The tab's picture: `None` while it is being made, `Some(None)` when
    /// the element is empty.
    pub fn picture(&self, tab: TabId) -> Option<Option<Arc<RenderImage>>> {
        self.images.get(&tab).cloned()
    }

    pub(crate) fn is_assembly(&self, tab: TabId, cx: &App) -> bool {
        matches!(self.doc.read(cx).session.doc.tab(tab).map(|t| &t.content), Some(TabContent::Assembly(_)))
    }

    /// Make (or remake) `tab`'s picture if its content changed.
    pub fn request(&mut self, tab: TabId, cx: &mut Context<Self>) {
        use std::hash::{Hash as _, Hasher as _};
        if self.busy {
            if !self.queued.contains(&tab) {
                self.queued.push(tab);
            }
            return;
        }
        let doc = self.doc.read(cx).session.doc.clone();
        let Some(t) = doc.tab(tab) else { return };
        let hash = match &t.content {
            TabContent::PartStudio(_) => noshape_model::drawing::views::source_hash(&doc, &noshape_model::drawing::views::SourceKey::studio(tab)),
            // Assemblies and drawings depend on other tabs: hash them all.
            TabContent::Assembly(_) | TabContent::Drawing(_) => {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                serde_json::to_vec(&doc.tabs.iter().map(|t| &t.content).collect::<Vec<_>>()).unwrap_or_default().hash(&mut h);
                Some(h.finish())
            }
            _ => None,
        };
        let Some(hash) = hash else { return };
        if self.made_for.get(&tab) == Some(&hash) {
            return;
        }
        self.busy = true;
        let versions = self.doc.read(cx).session.pinned_documents();
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        let evaluator = self.evaluator.clone();
        // A Part Studio: the open view's display, else regenerated here with coarse meshes (enough for the card),
        // stopped when a view of the document regenerates (qa's dock: the hovered tab's picture, regenerated with
        // fine meshes, ran beside the clicked tab's opening and doubled it).
        let studio = match &t.content {
            TabContent::PartStudio(ps) => {
                let path = self.doc.read(cx).session.path.clone();
                let doc_vars = noshape_model::variables::element_variables(&doc, tab);
                match crate::start_thumbnail::shown_display(&path, tab, ps, &doc_vars, cx) {
                    Some(d) => Some(Ok(d)),
                    None => {
                        let cancel = noshape_model::regen::RegenCancel::default();
                        let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
                        let custom: Arc<dyn noshape_model::custom::CustomFeatureProvider> = self.doc.read(cx).custom_features_lazy();
                        let opts = noshape_model::regen::RegenOptions { document_variables: doc_vars, preview_meshes: true, derived: Some(derived), custom_features: Some(custom), cancel: Some(cancel.clone()), ..Default::default() };
                        crate::start_thumbnail::picture_job_started(&path, cancel, cx);
                        Some(Err((self.regen.clone(), opts)))
                    }
                }
            }
            _ => None,
        };
        cx.spawn(async move |this, cx| {
            let picture = cx
                .background_spawn(async move {
                    // `None`: stopped (made again later); `Some(None)`: nothing to draw.
                    let Some(t) = doc.tab(tab) else { return Some(None) };
                    if let TabContent::Drawing(d) = &t.content {
                        return Some(crate::tab_manager::drawing_thumbnail(&doc, &versions, d, t.name.clone()));
                    }
                    let scene = match (&t.content, studio) {
                        (TabContent::Assembly(a), _) => {
                            let result = evaluator.lock().evaluate(&doc, a, &Default::default(), None);
                            crate::assembly_scene::build(&result, &Default::default(), &mut Default::default(), gpu.uploader()).scene
                        }
                        (TabContent::PartStudio(ps), Some(made)) => {
                            let display = match made {
                                Ok(d) => d,
                                Err((regen, opts)) => {
                                    let r = regen.lock().regenerate(ps, &ps.configuration.defaults(), &doc.settings.units, &opts);
                                    if r.cancelled {
                                        return None;
                                    }
                                    Arc::new(r.display)
                                }
                            };
                            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
                            crate::scene_builder::build(&display, &opts, &mut Default::default(), gpu.uploader()).scene
                        }
                        _ => return Some(None),
                    };
                    let Some((lo, hi)) = scene.bounds() else { return Some(None) };
                    let (w, h) = CARD_PICTURE;
                    let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
                    let Ok(png) = gpu.render_png(Arc::new(scene), camera, crate::start_thumbnail::thumbnail_style(), w, h).await else { return Some(None) };
                    Some(crate::tab_manager::decode(&png))
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.busy = false;
                // Stopped for a view's regeneration: made again at the next hover.
                if let Some(picture) = picture {
                    this.made_for.insert(tab, hash);
                    this.images.insert(tab, picture);
                }
                // The next queued tab that needs a picture (an unchanged one
                // or one without any returns at once).
                while !this.busy && !this.queued.is_empty() {
                    let next = this.queued.remove(0);
                    this.request(next, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// The card a hovered inactive tab shows above itself (live part-studio/17):
/// 314 x 184 white, 1 px `#d7d7d7`, rounded 4, soft shadow, centered on the
/// tab with a notch pointing at it (tip 3 px above the tab), holding the
/// element's picture; an empty Assembly shows Onshape's gray cube.
pub struct TabCard {
    thumbs: Entity<TabThumbs>,
    tab: TabId,
    /// The tab's bounds.
    anchor: Bounds<Pixels>,
    _watch: Subscription,
}

impl TabCard {
    pub fn new(thumbs: Entity<TabThumbs>, tab: TabId, anchor: Bounds<Pixels>, cx: &mut Context<Self>) -> Self {
        thumbs.update(cx, |t, cx| t.request(tab, cx));
        let _watch = cx.observe(&thumbs, |_, _, cx| cx.notify());
        Self { thumbs, tab, anchor, _watch }
    }
}

const CARD_W: f32 = 314.;
const CARD_H: f32 = 184.;

impl Render for TabCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picture: AnyElement = match self.thumbs.read(cx).picture(self.tab) {
            Some(Some(img)) => img_element(img),
            // (What an empty Part Studio shows is not captured yet:
            // REQUESTS.md 19 Oct; it stays blank.)
            Some(None) if self.thumbs.read(cx).is_assembly(self.tab, cx) => crate::assets::icon("tab-card-empty-assembly", 100., cx).into_any_element(),
            _ => div().into_any_element(),
        };
        let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.15), offset: point(px(0.), px(1.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }];
        let card = div()
            .relative()
            .w(px(CARD_W))
            .h(px(CARD_H))
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(0xd7d7d7))
            .rounded(px(4.))
            .shadow(shadow)
            .child(picture)
            // Over the bottom border (the notch's top half is the art).
            .child(div().absolute().top(px(CARD_H - 2.)).left(px(CARD_W * 0.5 - 9.)).child(crate::assets::icon("tab-card-notch", 16., cx)));
        let a = self.anchor;
        let at = point(a.center().x - px(CARD_W * 0.5), a.top() - px(10. + CARD_H));
        // Not while a tab is dragged (live qa5 TABS1).
        div().when(!crate::os_menu::is_open(cx) && !cx.has_active_drag(), |d| d.child(anchored().position(at).snap_to_window().child(card)))
    }
}

pub(crate) fn img_element(image: Arc<RenderImage>) -> AnyElement {
    let (w, h) = CARD_PICTURE;
    img(ImageSource::Render(image)).w(px(w as f32 * 0.5)).h(px(h as f32 * 0.5)).into_any_element()
}
