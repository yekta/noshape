//! Part Studio edits, shared by the Part Studio view and the agent API. Each
//! function is what one UI command does to the model; the caller commits it
//! as one microversion with the matching description.

use noshape_model::configuration::ConfigValues;
use noshape_model::partstudio::{Feature, PartStudio};
use noshape_model::reference::{BodyId, FeatureId, Reference};

/// "Insert feature : Extrude 1".
pub fn insert_description(name: &str) -> String {
    format!("Insert feature : {name}")
}

/// "Edit : Extrude 1".
pub fn edit_description(name: &str) -> String {
    format!("Edit : {name}")
}

/// Accept a new feature's dialog: insert it at `index` (the rollback bar
/// when it opened), advance the bar, and hide the sketch it consumes (Onshape
/// hides the sketch a new feature uses). Returns where it went.
pub fn insert_feature(ps: &mut PartStudio, feature: Feature, index: usize) -> usize {
    if let Some(sid) = noshape_model::interaction::source_sketch(&ps.features, &feature)
        && let Some(sk) = ps.feature_mut(sid)
    {
        sk.hidden = true;
    }
    let at = index.min(ps.features.len());
    ps.features.insert(at, feature);
    if let Some(r) = ps.rollback {
        ps.rollback = Some(r + 1);
    }
    at
}

/// Accept an edited feature's dialog: configured parameters go to the active
/// configuration's row, the rest to the feature.
pub fn edit_feature(ps: &mut PartStudio, index: usize, feature: Feature, config: &ConfigValues) {
    let PartStudio { features, configuration, .. } = ps;
    if let Some(slot) = features.get_mut(index) {
        let mut feature = feature;
        configuration.store_edit(slot, &mut feature, config);
        *slot = feature;
    }
}

/// "Delete : Extrude 1, Fillet 1".
pub fn delete_description(ps: &PartStudio, ids: &[FeatureId]) -> String {
    let names: Vec<String> = ids.iter().filter_map(|id| ps.feature(*id).map(|f| f.name.clone())).collect();
    format!("Delete : {}", names.join(", "))
}

pub fn delete_features(ps: &mut PartStudio, ids: &[FeatureId]) {
    for id in ids {
        ps.remove(*id);
    }
}

/// "Suppress" / "Unsuppress".
pub fn suppress_description(suppressed: bool) -> &'static str {
    if suppressed { "Suppress" } else { "Unsuppress" }
}

/// Suppress / Unsuppress. Unsuppress on a feature with a dynamic
/// suppression expression drops the expression too (the menu's
/// "Unsuppress" there).
pub fn set_suppressed(ps: &mut PartStudio, ids: &[FeatureId], suppressed: bool) {
    for id in ids {
        if let Some(f) = ps.feature_mut(*id) {
            f.suppressed = suppressed;
            if !suppressed {
                f.suppression_expr = None;
            }
        }
    }
}

pub fn suppression_expr_description(expr: Option<&str>) -> &'static str {
    if expr.is_some() { "Dynamic suppression" } else { "Remove dynamic suppression" }
}

/// Dynamic suppression ("Unsuppressed if" expression; None removes it).
pub fn set_suppression_expr(ps: &mut PartStudio, id: FeatureId, expr: Option<String>) {
    if let Some(f) = ps.feature_mut(id) {
        f.suppression_expr = expr.filter(|e| !e.trim().is_empty());
    }
}

/// The "Folder name" popup's default (live part-studio/586).
pub const NEW_FOLDER_NAME: &str = "New folder";
pub const ADD_FOLDER_DESCRIPTION: &str = "Add folder";
pub const UNPACK_FOLDER_DESCRIPTION: &str = "Remove folder";

pub fn rename_folder_description(name: &str) -> String {
    format!("Rename folder : {name}")
}

/// Add selection to folder… / the list header's New folder (an empty
/// folder at the rollback bar). An empty name is "New folder".
pub fn add_folder(ps: &mut PartStudio, ids: &[FeatureId], name: &str) -> u32 {
    let name = name.trim();
    ps.new_folder(ids, Some(if name.is_empty() { NEW_FOLDER_NAME } else { name }.to_string()))
}

pub fn rename_folder(ps: &mut PartStudio, folder: u32, name: &str) {
    if let Some(f) = ps.folder_mut(folder) {
        f.name = name.to_string();
    }
}

pub const ROLLBACK_DESCRIPTION: &str = "Move rollback bar";

/// Rollback bar after `index` features (None: at the end).
pub fn set_rollback(ps: &mut PartStudio, index: Option<usize>) {
    ps.rollback = index.filter(|i| *i < ps.features.len());
}

/// "Rename : <new name>".
pub fn rename_description(name: &str) -> String {
    format!("Rename : {name}")
}

pub fn rename_feature(ps: &mut PartStudio, id: FeatureId, name: &str) {
    if let Some(f) = ps.feature_mut(id) {
        f.name = name.trim().to_string();
        f.custom_name = true;
    }
}

/// What Hide / Show changes: parts, features (sketches, planes, mate
/// connectors) and default geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisibilityTargets {
    pub bodies: Vec<BodyId>,
    pub features: Vec<FeatureId>,
    pub defaults: Vec<Reference>,
}

impl VisibilityTargets {
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty() && self.features.is_empty() && self.defaults.is_empty()
    }
}

/// Sort selected references into visibility targets. `made_by` lists the
/// parts a feature made (a feature that made parts hides its parts;
/// sketches, planes and connectors hide themselves); `body_of` finds the
/// part a face, edge or vertex belongs to.
pub fn visibility_targets(targets: &[Reference], made_by: impl Fn(FeatureId) -> Vec<BodyId>, body_of: impl Fn(&Reference) -> Option<BodyId>) -> VisibilityTargets {
    let mut t = VisibilityTargets::default();
    for r in targets {
        match r {
            Reference::Origin | Reference::Plane(_) => t.defaults.push(r.clone()),
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => t.features.push(*sketch),
            Reference::FeaturePlane(f) | Reference::MateConnector { feature: f, .. } => t.features.push(*f),
            Reference::Feature(f) => {
                let made = made_by(*f);
                if made.is_empty() {
                    t.features.push(*f);
                } else {
                    t.bodies.extend(made);
                }
            }
            r => {
                if let Some(b) = body_of(r) {
                    t.bodies.push(b);
                }
            }
        }
    }
    t
}

/// The part a reference belongs to: a part itself, or one of its faces,
/// edges or vertices.
pub fn body_of(display: &noshape_model::display::StudioDisplay, r: &Reference) -> Option<BodyId> {
    match r {
        Reference::Body(b) => Some(b.clone()),
        _ => display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| b.id.clone()),
    }
}

/// "Hide" / "Show".
pub fn visibility_description(hidden: bool) -> &'static str {
    if hidden { "Hide" } else { "Show" }
}

pub fn set_visibility(ps: &mut PartStudio, t: &VisibilityTargets, hidden: bool) {
    for b in &t.bodies {
        ps.part_properties.entry(b.clone()).or_default().hidden = hidden;
    }
    for f in &t.features {
        if let Some(f) = ps.feature_mut(*f) {
            f.hidden = hidden;
        }
    }
    for d in &t.defaults {
        ps.hidden_defaults.retain(|x| x != d);
        if hidden {
            ps.hidden_defaults.push(d.clone());
        }
    }
}

pub const SHOW_ALL_DESCRIPTION: &str = "Show all";

/// Context menu "Show all": parts, sketches, planes and default geometry.
pub fn show_all(ps: &mut PartStudio) {
    for p in ps.part_properties.values_mut() {
        p.hidden = false;
    }
    for f in &mut ps.features {
        f.hidden = false;
    }
    ps.hidden_defaults.clear();
}

/// Parts list › Rename: "Rename : <name>".
pub fn rename_part_description(name: &str) -> String {
    format!("Rename : {name}")
}

/// Parts list › Rename: the part's name.
pub fn rename_part(ps: &mut PartStudio, body: &BodyId, name: &str) {
    ps.part_properties.entry(body.clone()).or_default().name = Some(name.to_string());
}

/// The part Properties dialog's values.
#[derive(Clone, Debug, Default)]
pub struct PartPropertyValues {
    pub name: String,
    pub description: String,
    pub part_number: String,
    pub revision: String,
    pub meta: noshape_model::partstudio::ItemMeta,
    /// Mass override, kg.
    pub mass_override: Option<f64>,
}

/// Properties › Save: "Properties : <name>".
pub fn properties_description(name: &str) -> String {
    format!("Properties : {name}")
}

/// Properties › Save: the part's values. Fields configured for the active
/// configuration (`configured`, by their label: "Name", "Description",
/// "Part number", "Revision", "Vendor", "Project", "Product line", "Title
/// 1"…"Title 3") keep the part's own value; empty texts clear.
pub fn set_part_properties(ps: &mut PartStudio, body: &BodyId, v: &PartPropertyValues, configured: &[&str]) {
    let has = |f: &str| configured.contains(&f);
    let p = ps.part_properties.entry(body.clone()).or_default();
    let old = p.meta.clone();
    if !has("Name") {
        p.name = Some(v.name.clone());
    }
    if !has("Description") {
        p.description = Some(v.description.clone()).filter(|s| !s.is_empty());
    }
    if !has("Part number") {
        p.part_number = Some(v.part_number.clone()).filter(|s| !s.is_empty());
    }
    if !has("Revision") {
        p.revision = Some(v.revision.clone()).filter(|s| !s.is_empty());
    }
    p.meta = v.meta.clone();
    for (f, new, was) in [
        ("Vendor", &mut p.meta.vendor, old.vendor),
        ("Project", &mut p.meta.project, old.project),
        ("Product line", &mut p.meta.product_line, old.product_line),
        ("Title 1", &mut p.meta.title1, old.title1),
        ("Title 2", &mut p.meta.title2, old.title2),
        ("Title 3", &mut p.meta.title3, old.title3),
    ] {
        if has(f) {
            *new = was;
        }
    }
    p.mass_override = v.mass_override;
}

/// Context menu "Paste sketch" on a plane or planar face (after "Copy
/// sketch"): the sketches pasted at the rollback bar and moved onto `plane`.
/// One edit, "Paste sketch". Returns the new sketches.
pub fn paste_sketches_onto(ps: &mut PartStudio, sketches: &[noshape_model::partstudio::Feature], plane: noshape_model::reference::Reference) -> Vec<noshape_model::reference::FeatureId> {
    let at = ps.rollback_index();
    let ids = ps.paste_features(sketches, at);
    for id in &ids {
        if let Some(f) = ps.feature_mut(*id) {
            f.params.set("sketchPlane", noshape_model::params::ParamValue::Query(vec![plane.clone()]));
        }
    }
    ids
}
