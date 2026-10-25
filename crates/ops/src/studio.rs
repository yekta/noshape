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

/// Rename a feature; an empty name gives it back its default name (live
/// qa4 r5 s8 (5): "Fillet 2"), the lowest free "<type> N".
pub fn rename_feature(ps: &mut PartStudio, id: FeatureId, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        let Some(ty) = ps.feature(id).and_then(|f| f.feature_type()) else { return };
        if let Some(f) = ps.feature_mut(id) {
            f.name.clear();
        }
        let default = ps.next_name(ty);
        if let Some(f) = ps.feature_mut(id) {
            f.name = default;
            f.custom_name = false;
        }
        return;
    }
    if let Some(f) = ps.feature_mut(id) {
        f.name = name.to_string();
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

// ------------------------------------------------------------ new-feature dialog logic

/// Parameter changes a new feature's dialog makes on its own.
pub type AutoChanges = Vec<(&'static str, noshape_model::params::ParamValue)>;

/// Onshape's automatic choices in a new Extrude / Revolve / Sweep / Thicken
/// from the parts its profiles lie on: Thicken's operation, merge scope and
/// flip (thickenEditLogic); a Remove / Intersect Extrude on a part points
/// into it. `user_set`: the fields the user set (left alone). `display`: the
/// Part Studio before the feature.
pub fn auto_operation(ps: &PartStudio, display: &noshape_model::display::StudioDisplay, f: &Feature, user_set: &dyn Fn(&str) -> bool) -> AutoChanges {
    use noshape_model::params::ParamValue;
    let ty = f.type_id.as_str();
    if !matches!(ty, "extrude" | "revolve" | "sweep" | "thicken") {
        return Vec::new();
    }
    // Sweep's profiles are `profiles` (sweep.fs); the others' `entities`.
    let profiles_id = if ty == "sweep" { "profiles" } else { "entities" };
    let p = &f.params;
    let (op_user, flip_user, scope_user) = (user_set("operationType"), user_set("oppositeDirection"), user_set("booleanScope"));
    let (op, flipped) = (p.enum_("operationType").to_string(), p.bool("oppositeDirection"));
    // thickenEditLogic counts only faces of solids (live L18: a surface's
    // face leaves the operation on New).
    let solid = |b: &BodyId| ty != "thicken" || display.bodies.iter().find(|x| &x.id == b).is_some_and(|x| x.kind == noshape_model::display::BodyKind::Solid);
    // Parts the profiles lie on: a picked face, or the face a sketch was
    // drawn on.
    let host_of = |r: &Reference| match r {
        Reference::Face(_) => body_of(display, r).filter(|b| solid(b)),
        Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) if ty != "thicken" => {
            let plane = ps.feature(*sketch)?.params.query("sketchPlane").first().cloned()?;
            body_of(display, &plane)
        }
        _ => None,
    };
    let profiles = p.query(profiles_id);
    let hosts: Vec<BodyId> = if ty == "thicken" {
        let mut v: Vec<BodyId> = Vec::new();
        for b in profiles.iter().filter_map(host_of) {
            if !v.contains(&b) {
                v.push(b);
            }
        }
        v
    } else {
        profiles.iter().find_map(host_of).into_iter().collect()
    };
    let mut changes: AutoChanges = Vec::new();
    if !op_user && ty == "thicken" {
        // thickenEditLogic: Remove when the wall points into the part.
        let want = match (hosts.is_empty(), flipped) {
            (true, _) => "NEW",
            (false, true) => "REMOVE",
            _ => "ADD",
        };
        if op != want {
            changes.push(("operationType", ParamValue::Enum(want.into())));
        }
    }
    // The merge scope starts as the profiles' part, also when the user
    // picked the operation (live, qa5 EXT1: Remove fills "Part 1").
    if ty == "thicken" && !hosts.is_empty() && !scope_user && p.query("booleanScope").is_empty() {
        changes.push(("booleanScope", ParamValue::Query(hosts.iter().map(|b| Reference::Body(b.clone())).collect())));
    }
    // thickenEditLogic's canSetBooleanFlip: a user-picked Remove points the
    // wall into the part, Add out of it.
    if ty == "thicken" && op_user && !flip_user && !hosts.is_empty() && matches!(op.as_str(), "ADD" | "REMOVE") {
        let into = op == "REMOVE";
        if flipped != into {
            changes.push(("oppositeDirection", ParamValue::Bool(into)));
        }
    }
    if ty == "extrude" && !flip_user && !hosts.is_empty() {
        let into = matches!(op.as_str(), "REMOVE" | "INTERSECT");
        if flipped != into {
            changes.push(("oppositeDirection", ParamValue::Bool(into)));
        }
    }
    changes
}

/// Onshape's booleanStepEditLogic (booleanHeuristics.fs) for a new
/// Extrude / Revolve / Sweep / Loft, from the parts its tool touches in the
/// preview: one part → Add into it; none or several → New with an empty
/// scope. Nothing once the user set the scope or "Merge with all"; a
/// user-picked operation keeps it and only an empty scope is filled (live:
/// an Add over three parts starts with an empty scope and fails "No merge
/// scope selected.", qa5 BOOL4; a Remove over Part 1 gets "Part 1",
/// part-studio/234).
pub fn boolean_edit_logic(f: &Feature, user_set: &dyn Fn(&str) -> bool, touched: &[BodyId]) -> AutoChanges {
    use noshape_model::params::ParamValue;
    if !matches!(f.type_id.as_str(), "extrude" | "revolve" | "sweep" | "loft") {
        return Vec::new();
    }
    let p = &f.params;
    if user_set("booleanScope") || user_set("defaultScope") || p.enum_("bodyType") == "SURFACE" {
        return Vec::new();
    }
    let (op, scope, op_user) = (p.enum_("operationType").to_string(), p.query("booleanScope").to_vec(), user_set("operationType"));
    if op_user && op == "NEW" {
        return Vec::new();
    }
    let can_scope = !op_user || scope.is_empty();
    let (want_op, want_scope): (&str, Vec<Reference>) = match touched {
        [one] => ("ADD", vec![Reference::Body(one.clone())]),
        _ => ("NEW", Vec::new()),
    };
    let mut changes: AutoChanges = Vec::new();
    // Remove / Intersect picked by the flip logic stay (canSetBooleanFlip).
    if !op_user && op != want_op && !matches!(op.as_str(), "REMOVE" | "INTERSECT") {
        changes.push(("operationType", ParamValue::Enum(want_op.into())));
    }
    if can_scope && scope != want_scope && !(op_user && want_scope.is_empty()) {
        changes.push(("booleanScope", ParamValue::Query(want_scope)));
    }
    changes
}

/// The new feature dialog's Boolean choices after a preview, as the open
/// dialog makes them (std booleanStepEditLogic): when the tool itself can't
/// be built (Extrude's Direction ticked with nothing picked) the operation
/// and merge scope stay as they are; only a failing boolean (its merge
/// scope at fault) is judged. Otherwise [`boolean_edit_logic`] on the parts
/// the tool touched. The app's dialog and the agent API both call this.
pub fn boolean_choices(f: &Feature, user_set: &dyn Fn(&str) -> bool, state: &noshape_model::display::FeatureState) -> AutoChanges {
    if state.status == noshape_model::display::FeatureStatus::Error && !state.faulty.iter().any(|p| p == "booleanScope") {
        return Vec::new();
    }
    boolean_edit_logic(f, user_set, &state.touched)
}

/// A new Hole's Merge scope is the parts its holes reach and follows a
/// deeper or shallower hole until the user sets it (live L57); more parts
/// behind the locations than in front flips it (holeScopeFlipHeuristics)
/// until the user sets the flip. Returns the changes (a flip alone first).
pub fn hole_scope_logic(f: &Feature, user_set: &dyn Fn(&str) -> bool, touched: &[BodyId], behind: &[BodyId]) -> AutoChanges {
    use noshape_model::params::ParamValue;
    let p = &f.params;
    // std's holeEditLogic decides them where it runs (the dialog, dialog_logic).
    if f.type_id != "hole" || p.query("locations").is_empty() || noshape_model::custom::std_feature::has_editing_logic("hole") {
        return Vec::new();
    }
    let user = user_set("scope");
    if !user_set("oppositeDirection") && !user && behind.len() > touched.len() {
        return vec![("oppositeDirection", ParamValue::Bool(!p.bool("oppositeDirection")))];
    }
    if user {
        return Vec::new();
    }
    let want: Vec<Reference> = touched.iter().cloned().map(Reference::Body).collect();
    if want.as_slice() == p.query("scope") {
        return Vec::new();
    }
    vec![("scope", ParamValue::Query(want))]
}

// ------------------------------------------------------------ configured sketch dimensions

pub const CONFIGURE_DIMENSION: &str = "Configure dimension";
pub const UNCONFIGURE_DIMENSION: &str = "Unconfigure dimension";

/// The configured-parameter key of sketch dimension `cid`.
pub fn configured_dim_key(cid: u32) -> String {
    format!("{}{}", noshape_model::regen::features::sketch::CONFIGURED_DIMENSION_PREFIX, cid)
}

/// Sketch dimension menu › Configure dimension with a List or Checkbox
/// input: every option starts with the dimension's current value `text`.
pub fn configure_dimension(ps: &mut PartStudio, sketch: FeatureId, cid: u32, input: &str, text: &str) -> bool {
    use noshape_model::configuration::{ConfigInputKind, ConfiguredParam};
    use noshape_model::params::ParamValue;
    let key = configured_dim_key(cid);
    let keys: Vec<String> = match ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
        Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
        _ => return false,
    };
    ps.configuration.configured.retain(|c| !(c.feature == sketch && c.param == key));
    let values = keys.into_iter().map(|k| (k, ParamValue::Expr(noshape_units::ExprValue::new(text)))).collect();
    ps.configuration.configured.push(ConfiguredParam { input: input.to_string(), feature: sketch, param: key, values });
    true
}

/// Sketch dimension menu › Unconfigure dimension.
pub fn unconfigure_dimension(ps: &mut PartStudio, sketch: FeatureId, cid: u32) {
    let key = configured_dim_key(cid);
    ps.configuration.configured.retain(|c| !(c.feature == sketch && c.param == key));
}

/// A sketch dimension's value as its value box shows it: the expression
/// typed, else the measured value in the workspace units.
pub fn dimension_text(data: &noshape_model::sketch_feature::SketchData, cid: u32, units: &noshape_units::UnitPrefs) -> Option<String> {
    data.dimension_exprs.get(&cid).map(|e| e.text.clone()).or_else(|| {
        let c = data.sketch.constraint(noshape_sketch::ConstraintId(cid))?;
        Some(noshape_units::format_quantity_exact(noshape_units::Quantity::new(c.kind.value()?, noshape_scene::sketch_mode::dim_unit(&c.kind)), units))
    })
}

pub const SIZE_PLANE_DESCRIPTION: &str = "Size to Part Studio";

/// Plane menu › Size to Part Studio: the plane's half width and height that
/// cover the shown parts' boxes, 10 % over. None without parts.
pub fn plane_size_to_studio(display: &noshape_model::display::StudioDisplay, r: &Reference) -> Option<(f64, f64)> {
    let f = display.planes.iter().find(|p| &p.reference == r)?.frame;
    let (mut hw, mut hh) = (0.0f64, 0.0f64);
    for (lo, hi) in display.bodies.iter().filter(|b| !b.hidden).filter_map(|b| b.bounds) {
        for k in 0..8 {
            let c = glam::DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }) - f.origin;
            hw = hw.max(c.dot(f.x).abs());
            hh = hh.max(c.dot(f.y).abs());
        }
    }
    (hw > 0.0 && hh > 0.0).then_some((hw * 1.1, hh * 1.1))
}

pub fn set_plane_size(ps: &mut PartStudio, r: &Reference, half_width: f64, half_height: f64) {
    ps.plane_sizes.retain(|(x, ..)| x != r);
    ps.plane_sizes.push((r.clone(), half_width, half_height));
}

/// Plane frame of a plane, planar face, sketch or mate connector (a section
/// view's or a sketch's plane).
pub fn frame_of_ref(display: &noshape_model::display::StudioDisplay, ctx: Option<&noshape_model::regen::context::Context>, r: &Reference) -> Option<noshape_model::display::PlaneFrame> {
    use noshape_model::display::PlaneFrame;
    match r {
        Reference::Plane(_) | Reference::FeaturePlane(_) => display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
        Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => display.sketches.iter().find(|s| s.feature == *sketch).map(|s| s.frame),
        Reference::MateConnector { .. } => display.connectors.iter().find(|c| &c.reference == r).map(|c| PlaneFrame { origin: c.origin, x: c.x, y: c.z.cross(c.x), normal: c.z }),
        Reference::Face(n) => ctx?.bodies.iter().find_map(|b| {
            let fi = *b.find_faces(n).first()?;
            let pl = b.shape.sub_shape(noshape_kernel::ShapeType::Face, fi).ok()?.face_plane().ok()??;
            let (normal, x) = (pl.unit_normal(), pl.unit_x());
            Some(PlaneFrame { origin: pl.origin, x, y: normal.cross(x), normal })
        }),
        _ => None,
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    /// Live (qa4 r5 s8 (5)): renaming to an empty name gives the feature
    /// its default name back.
    #[test]
    fn an_empty_name_restores_the_default_name() {
        let mut ps = noshape_model::demo::plate_studio();
        let ext = ps.features.iter().find(|f| f.type_id == "extrude").map(|f| (f.id, f.name.clone())).expect("extrude");
        rename_feature(&mut ps, ext.0, "Renamed");
        assert_eq!(ps.feature(ext.0).map(|f| (f.name.as_str(), f.custom_name)), Some(("Renamed", true)));
        rename_feature(&mut ps, ext.0, "  ");
        assert_eq!(ps.feature(ext.0).map(|f| (f.name.clone(), f.custom_name)), Some((ext.1, false)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_model::display::{FeatureState, FeatureStatus};
    use noshape_model::params::ParamValue;

    fn state(status: FeatureStatus, faulty: &[&str], touched: Vec<BodyId>) -> FeatureState {
        FeatureState { id: FeatureId::new_random(), status, message: None, underdefined: false, time_ms: 0.0, faulty: faulty.iter().map(|s| s.to_string()).collect(), computed: Vec::new(), touched, error_mesh: None, touched_behind: Vec::new(), debug: Default::default(), missing: 0 }
    }

    /// An Extrude already Adding into Part 1 (a first preview touched it).
    fn adding() -> (Feature, BodyId) {
        let part = BodyId { feature: FeatureId::new_random(), index: 0 };
        let mut f = PartStudio::new().new_feature("extrude", &noshape_units::UnitPrefs::default()).unwrap();
        f.params.set("operationType", ParamValue::Enum("ADD".into()));
        f.params.set("booleanScope", ParamValue::Query(vec![Reference::Body(part.clone())]));
        (f, part)
    }

    #[test]
    fn a_tool_that_fails_keeps_the_operation() {
        let (f, _) = adding();
        // Direction ticked with nothing picked: the tool fails, touches nothing.
        let st = state(FeatureStatus::Error, &["extrudeDirection"], Vec::new());
        assert!(boolean_choices(&f, &|_| false, &st).is_empty());
        // Without the rule the empty preview would turn it into New.
        assert!(!boolean_edit_logic(&f, &|_| false, &[]).is_empty());
    }

    #[test]
    fn a_failing_boolean_is_judged() {
        let (f, part) = adding();
        let st = state(FeatureStatus::Error, &["booleanScope"], Vec::new());
        let c = boolean_choices(&f, &|_| false, &st);
        assert!(c.iter().any(|(k, v)| *k == "operationType" && *v == ParamValue::Enum("NEW".into())), "{c:?}");
        // A working tool over one part: Add into it, nothing to change.
        let ok = state(FeatureStatus::Ok, &[], vec![part]);
        assert!(boolean_choices(&f, &|_| false, &ok).is_empty());
    }
}
