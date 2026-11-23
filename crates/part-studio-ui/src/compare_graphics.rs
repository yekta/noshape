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
    /// The blend, selected feature row and compared documents (by pointer) last drawn (None: not comparing).
    seen: Option<(f32, Option<noshape_model::reference::FeatureId>, (usize, usize))>,
}

/// A side's display: its Part Studio regenerated in that side's document, with its Variable Studio variables, other
/// Part Studios for Derived features and its Feature Studios' custom features, as the Part Studio itself (qa
/// COMPARE-COMMON-PARTS-MISSING: without them only the parts of built-in features were drawn). Also the Repair
/// panel's (repair_view.rs).
pub(crate) fn regenerate(doc: &Document, tab: TabId, config: &noshape_model::configuration::ConfigValues) -> StudioDisplay {
    let Some(TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else { return StudioDisplay::default() };
    let opts = noshape_model::regen::RegenOptions {
        document_variables: noshape_model::variables::element_variables(doc, tab),
        derived: Some(noshape_ops::derived::DocStudios::new(doc.clone(), Default::default())),
        custom_features: Some(noshape_model::custom::DocumentScripts::from_document(doc)),
        ..Default::default()
    };
    noshape_model::regen::Regenerator::new().regenerate(ps, config, &doc.settings.units, &opts).display
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
        let blend = crate::part_studio_hooks::compare_blend(doc, cx)?;
        let (base, base_config, target, target_config) = crate::part_studio_hooks::compare_documents(doc, cx)?;
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
        // Live (wf23-18 step 25): the selected row's feature lit on both sides.
        if let Some(f) = crate::part_studio_hooks::compare_selected(doc, cx) {
            for display in [&displays.0, &displays.1] {
                scene.instances.extend(noshape_scene::builder::compare_feature(display, f, up));
            }
        }
        scene.compare = Some(Compare { blend });
        Some(scene)
    }

    /// Whether the compare state changed since the last call (start, stop, a slider move, a feature row selected,
    /// Reverse compare: qa HIST-REVERSE-COMPARE-BAND): the owner then builds its scene again.
    pub fn changed(&mut self, doc: &Entity<DocModel>, cx: &App) -> bool {
        let sides = crate::part_studio_hooks::compare_documents(doc, cx).map_or((0, 0), |(b, _, t, _)| (Arc::as_ptr(&b) as usize, Arc::as_ptr(&t) as usize));
        let now = crate::part_studio_hooks::compare_blend(doc, cx).map(|b| (b, crate::part_studio_hooks::compare_selected(doc, cx), sides));
        std::mem::replace(&mut self.seen, now) != now
    }

    /// Compare is open (as last seen by [`Self::changed`]).
    pub fn comparing(&self) -> bool {
        self.seen.is_some()
    }

    /// The compared documents (by pointer: base, target) the displays are for.
    #[cfg(test)]
    pub(crate) fn sides(&self) -> Option<(usize, usize)> {
        self.key.map(|(b, t, _)| (b, t))
    }

    /// Not comparing (any more): forget the displays.
    pub fn clear(&mut self) {
        if self.key.is_some() {
            *self = Self { seen: self.seen, ..Self::default() };
        }
    }
}

#[cfg(test)]
mod tests {
    /// qa COMPARE-COMMON-PARTS-MISSING: a compared side's Part Studio regenerates with its document's variables (an
    /// extrude `#w` deep is drawn, not dropped).
    #[test]
    fn a_compared_side_regenerates_with_its_document() {
        let path = std::env::temp_dir().join(format!("noshape-compare-side-{}.noshape", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        {
            let mut ctx = noshape_ops::Ctx::new();
            let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
            call("document_create", serde_json::json!({ "path": path.to_str().unwrap() }));
            call("variable_create_studio", serde_json::json!({}));
            call("variable_create", serde_json::json!({ "tab": "Variable Studio 1", "name": "w", "value": "10 mm" }));
            let sk = call("sketch_create", serde_json::json!({ "plane": "plane:top" }))["sketch"].clone();
            let d = call("sketch_draw", serde_json::json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
            call("feature_add", serde_json::json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "#w" } }));
        }
        let store = noshape_store::Document::open(&path).unwrap();
        let main = store.list_branches().unwrap().remove(0);
        let doc = noshape_model::document::Document::from_blobs(&store.read_all(store.head(main.id).unwrap()).unwrap()).unwrap();
        let tab = doc.tabs.iter().find(|t| t.name == "Part Studio 1").expect("Part Studio 1").id;
        let display = super::regenerate(&doc, tab, &Default::default());
        assert_eq!(display.bodies.len(), 1, "the #w deep extrude's part");
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
    }

    /// qa HIST-COMPARE-FEATURE-HIGHLIGHT (live wf23-18 step 25): a feature row selected in Compare lights the faces and
    /// edges that feature made, and only those: Extrude 1's block (up to 10 mm), not Extrude 2's boss on it (to 25 mm).
    #[test]
    fn a_selected_compare_row_lights_its_features_faces() {
        let path = std::env::temp_dir().join(format!("noshape-compare-lit-{}.noshape", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        {
            let mut ctx = noshape_ops::Ctx::new();
            let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
            call("document_create", serde_json::json!({ "path": path.to_str().unwrap() }));
            let sk = call("sketch_create", serde_json::json!({ "plane": "plane:top" }))["sketch"].clone();
            let d = call("sketch_draw", serde_json::json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
            call("feature_add", serde_json::json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "10 mm" } }));
            let sk = call("sketch_create", serde_json::json!({ "plane": "plane:top" }))["sketch"].clone();
            let d = call("sketch_draw", serde_json::json!({ "sketch": sk, "shapes": [{ "tool": "circle", "points": [[20, 10], [25, 10]] }] }));
            call("feature_add", serde_json::json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "25 mm", "operationType": "ADD" } }));
        }
        let store = noshape_store::Document::open(&path).unwrap();
        let main = store.list_branches().unwrap().remove(0);
        let doc = noshape_model::document::Document::from_blobs(&store.read_all(store.head(main.id).unwrap()).unwrap()).unwrap();
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        let tab = doc.tabs.iter().find(|t| t.name == "Part Studio 1").expect("Part Studio 1");
        let noshape_model::document::TabContent::PartStudio(ps) = &tab.content else { panic!("a Part Studio") };
        let extrudes: Vec<_> = ps.features.iter().filter(|f| f.type_id == "extrude").map(|f| f.id).collect();
        assert_eq!(extrudes.len(), 2);
        let display = super::regenerate(&doc, tab.id, &Default::default());
        assert_eq!(display.bodies.len(), 1, "the boss is added to the block");
        let top = |f| {
            let (faces, lines) = noshape_scene::builder::compare_feature_geometry(&display, f);
            let faces = faces.expect("the feature's faces");
            assert!(!lines.segments.is_empty(), "the feature's edges");
            faces.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max)
        };
        assert!(top(extrudes[0]) < 0.0101, "Extrude 1 lights the block only: {}", top(extrudes[0]));
        assert!(top(extrudes[1]) > 0.024, "Extrude 2 lights the boss: {}", top(extrudes[1]));
    }
}
