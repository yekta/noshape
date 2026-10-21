//! Versions and history › Compare: the Part Studio's graphics (live
//! "Compare mode graphics: the blend slider"; Onshape's compare pass). Both
//! sides of the compare are regenerated (off the UI thread) and drawn
//! together, tagged base and target for the renderer (`noshape_render::
//! compare`): common geometry grey, base-only blue, target-only red-orange,
//! mixed by the slider. Planes, the origin and sketches are not drawn
//! (live), and the compare geometry is not pickable.

use crate::doc_model::DocModel;
use gpui_kit::*;
use noshape_model::display::StudioDisplay;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_render::renderer::Uploader;
use noshape_render::{Compare, Scene, compare};
use std::sync::Arc;

/// A Part Studio's compare graphics state.
#[derive(Default)]
pub struct CompareGraphics {
    /// The documents (by pointer) and tab the displays are for.
    key: Option<(usize, usize, TabId)>,
    displays: Option<Arc<(StudioDisplay, StudioDisplay)>>,
    task: Option<Task<()>>,
    cache: noshape_scene::builder::GpuCache,
    /// The blend last drawn (None: not comparing).
    seen: Option<f32>,
}

/// A side's display: its Part Studio regenerated.
fn regenerate(doc: &Document, tab: TabId, config: &noshape_model::configuration::ConfigValues) -> StudioDisplay {
    match doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => noshape_model::regen::Regenerator::new().regenerate(ps, config, &doc.settings.units, &noshape_model::regen::RegenOptions::default()).display,
        _ => StudioDisplay::default(),
    }
}

/// The view holding a [`CompareGraphics`]: how to reach it, and what to run
/// when both sides are regenerated.
pub struct Owner<V> {
    pub get: fn(&mut V) -> &mut CompareGraphics,
    pub then: fn(&mut V, &mut Context<V>),
}

impl CompareGraphics {
    /// The compare scene of `tab` while `doc` is comparing, once both sides
    /// are regenerated (`owner.then` runs when they are).
    pub fn scene<V: 'static>(&mut self, doc: &Entity<DocModel>, tab: TabId, mode: noshape_scene::builder::DisplayMode, up: &Uploader, owner: Owner<V>, cx: &mut Context<V>) -> Option<Scene> {
        let Owner { get, then } = owner;
        let blend = crate::compare_mode::blend(doc, cx)?;
        let (base, base_config, target, target_config) = crate::compare_mode::documents(doc, cx)?;
        let key = (Arc::as_ptr(&base) as usize, Arc::as_ptr(&target) as usize, tab);
        if self.key != Some(key) {
            self.key = Some(key);
            self.displays = None;
            self.task = Some(cx.spawn(async move |this, cx| {
                let displays = cx.background_executor().spawn(async move { (regenerate(&base, tab, &base_config), regenerate(&target, tab, &target_config)) }).await;
                let _ = this.update(cx, |v, cx| {
                    let g = get(v);
                    if g.key == Some(key) {
                        g.displays = Some(Arc::new(displays));
                    }
                    then(v, cx);
                });
            }));
        }
        let displays = self.displays.clone()?;
        let opts = noshape_scene::builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, display_mode: mode, ..Default::default() };
        let mut scene = Scene::default();
        for (display, layer) in [(&displays.0, compare::BASE), (&displays.1, compare::TARGET)] {
            let built = noshape_scene::builder::build(display, &opts, &mut self.cache, up);
            scene.instances.extend(built.scene.instances.into_iter().map(|mut i| {
                i.pickable = false;
                // Past any highlight state.
                i.pick_base = 1 << 30;
                i.compare_layer(layer)
            }));
        }
        scene.compare = Some(Compare { blend });
        Some(scene)
    }

    /// Whether the compare state changed since the last call (start, stop,
    /// a slider move): the owner then builds its scene again.
    pub fn changed(&mut self, doc: &Entity<DocModel>, cx: &App) -> bool {
        let blend = crate::compare_mode::blend(doc, cx);
        std::mem::replace(&mut self.seen, blend) != blend
    }

    /// Not comparing (any more): forget the displays.
    pub fn clear(&mut self) {
        if self.key.is_some() {
            *self = Self { seen: self.seen, ..Self::default() };
        }
    }
}
