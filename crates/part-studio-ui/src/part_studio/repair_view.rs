//! Versions and history › View in repair: the Repair panel's graphics (live rp-906–910, repair_mode.rs). This Part
//! Studio as it was at the repaired history entry, regenerated off the UI thread and view only; while "Synchronize
//! view" is on it follows the main graphics' camera; a right-click offers Zoom to fit.

use crate::gpu::Gpu;
use crate::viewport::{Viewport, ViewportEvent};
use gpui_kit::*;
use noshape_model::document::TabId;
use noshape_render::Scene;
use std::sync::Arc;

pub struct RepairView {
    pub viewport: Entity<Viewport>,
    /// The snapshot (by pointer) and tab the scene is for.
    key: Option<(usize, TabId)>,
    task: Option<Task<()>>,
    cache: noshape_scene::builder::GpuCache,
    gpu: Gpu,
    /// Fitted once the regenerated part is in (when the view doesn't follow the main camera).
    fitted: bool,
    /// The main camera last followed: only its changes move this view (its own Zoom to fit stays until then).
    followed: Option<noshape_render::Camera>,
    _sub: Subscription,
}

impl RepairView {
    pub fn new(gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewport = cx.new(|cx| Viewport::new(gpu.clone(), window, cx));
        let sub = cx.subscribe_in(&viewport, window, |this: &mut Self, _, ev: &ViewportEvent, window, cx| {
            if let ViewportEvent::ContextMenu { position, .. } = ev {
                let vp = this.viewport.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                    let vp = vp.clone();
                    m.item(crate::os_menu::OsMenuItem::new("Zoom to fit").on_click(move |_, _, cx| vp.update(cx, |vp, cx| vp.zoom_to_fit(true, cx))))
                });
                crate::os_menu::open(*position, menu, window, cx);
            }
        });
        Self { viewport, key: None, task: None, cache: Default::default(), gpu, fitted: false, followed: None, _sub: sub }
    }

    /// Show `tab` of the document's repair snapshot; follow `camera` (the main graphics') while `synchronized`.
    pub fn update(&mut self, doc: &Entity<crate::doc_model::DocModel>, tab: TabId, camera: noshape_render::Camera, cx: &mut Context<Self>) {
        let Some(at) = crate::repair_mode::document(doc, cx) else { return };
        let key = (Arc::as_ptr(&at) as usize, tab);
        if self.key != Some(key) {
            self.key = Some(key);
            self.fitted = false;
            self.task = Some(cx.spawn(async move |this, cx| {
                let display = cx.background_executor().spawn(async move { crate::compare_graphics::regenerate(&at, tab, &Default::default()) }).await;
                let _ = this.update(cx, |this, cx| {
                    if this.key != Some(key) {
                        return;
                    }
                    let built = noshape_scene::builder::build(&display, &noshape_scene::builder::SceneOptions::default(), &mut this.cache, this.gpu.uploader());
                    let scene: Scene = built.scene;
                    this.viewport.update(cx, |vp, cx| vp.set_scene(Arc::new(scene), true, cx));
                    cx.notify();
                });
            }));
        }
        if crate::repair_mode::synchronized(doc, cx) {
            self.fitted = true;
            if self.followed != Some(camera) {
                self.followed = Some(camera);
                self.viewport.update(cx, |vp, cx| {
                    vp.camera = camera;
                    vp.request_redraw(cx);
                });
            }
        } else if !self.fitted && self.viewport.read(cx).can_fit() {
            self.fitted = true;
            self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(false, cx));
        }
    }
}

impl Render for RepairView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.viewport.clone())
    }
}
