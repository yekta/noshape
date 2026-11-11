//! Built-in features run by the standard library's own FeatureScript, as
//! Onshape evaluates them: the std feature (`sheetMetalFlange` of
//! sheetMetalFlange.fs, ...) is called with the definition its precondition
//! declares, built from the feature's parameter values. Its results are
//! live's wherever the host natives are (tests/fs/sm_harness.fs against
//! dom/fs-smprobe*.txt).

use super::studio::{ParamDecl, collect_params, defined_lambda};
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_featurescript::ast::TopKind;
use noshape_featurescript::compile::DeclKind;
use noshape_featurescript::ir::GlobalCell;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

/// A std feature: its function, the parameters of its precondition, and
/// the built-in schema plus std's parameters it lacks (every parameter gets
/// a value, as in Onshape's definition).
struct StdFeature {
    cell: Arc<GlobalCell>,
    params: Vec<ParamDecl>,
    specs: Vec<crate::params::ParamSpec>,
    /// Its "Editing Logic Function" (e.g. sheetMetalTabEditingLogic).
    editing: Option<Arc<noshape_featurescript::ir::OverloadSet>>,
    /// Its "Manipulator Change Function" (e.g. flangeManipulatorChange).
    manipulator_change: Option<Arc<noshape_featurescript::ir::OverloadSet>>,
}

/// Loaded std features by name (`None`: not found).
type Loaded = HashMap<&'static str, Option<Arc<StdFeature>>>;

static FEATURES: LazyLock<Mutex<Loaded>> = LazyLock::new(Default::default);

/// Std's feature `name` from `module`, loaded once. Loading (parsing and
/// compiling std, ~0.2 s, Hole's tables ~0.7 s) runs outside the lock, so a
/// dialog asking for a loaded feature never waits on another's load.
fn loaded(module: &str, name: &'static str) -> Option<Arc<StdFeature>> {
    if let Some(f) = FEATURES.lock().get(name) {
        return f.clone();
    }
    let f = load(module, name);
    FEATURES.lock().entry(name).or_insert(f).clone()
}

/// Load the std features whose editing logic dialogs run, on a background
/// thread (at startup: the first dialog then doesn't wait for std).
pub fn prewarm() {
    crate::register();
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("std prewarm".into()).spawn(|| {
            for ty in STD_EDITING_LOGIC {
                if let Some((module, name)) = logic_std_name(ty) {
                    loaded(module, name);
                }
            }
            // Then the sheet metal features std runs: a first Hem in a
            // session paid ~1 s compiling sheetMetalHem.fs (vibe3 regen profile).
            if crate::regen::std_route::std_sheet_metal_on() {
                for ty in crate::regen::std_route::STD_TYPES {
                    if let Some((module, _)) = std_name(ty) {
                        let _ = noshape_featurescript::std_module(module);
                    }
                }
            }
        });
    });
}

fn load(module: &str, name: &str) -> Option<Arc<StdFeature>> {
    // Natives bind when std compiles: register them first.
    super::host::register();
    let m = noshape_featurescript::std_module(module).ok()?;
    let decl = m.decls.iter().find(|d| &*d.name == name)?;
    let DeclKind::Const(cell) = &decl.kind else { return None };
    let item = m.ast.items.iter().find(|it| matches!(&it.kind, TopKind::Const { name: n, .. } if &**n == name))?;
    let TopKind::Const { init, .. } = &item.kind else { return None };
    let ann = item.annotation.as_ref().map(|a| noshape_featurescript::annot::eval(&m, a));
    let function = |key: &str| ann.as_ref().and_then(|ann| ann.str(key).map(str::to_string)).and_then(|f| m.functions(&f));
    let (editing, manipulator_change) = (function("Editing Logic Function"), function("Manipulator Change Function"));
    let lambda = defined_lambda(init, "defineSheetMetalFeature").or_else(|| defined_lambda(init, "defineFeature"))?;
    let mut params = Vec::new();
    if let Some(p) = &lambda.precondition {
        collect_params(&m, p, &mut params);
    }
    // The built-in feature std's feature stands for (Transform: fTransform).
    let ty = crate::regen::std_route::core_type(name);
    let mut specs = crate::features::get(ty).map(|t| t.params.clone()).unwrap_or_default();
    let extra: Vec<_> = super::studio::param_specs(&params).into_iter().filter(|s| !specs.iter().any(|b| b.id == s.id)).collect();
    specs.extend(extra);
    Some(Arc::new(StdFeature { cell: cell.clone(), params, specs, editing, manipulator_change }))
}

/// Std's feature of built-in `type_id` (its function's name): the parameters its precondition declares, in order
/// (a Part Studio's Show code writes every one). Loads std once.
pub fn std_params(type_id: &str) -> Option<Vec<ParamDecl>> {
    let (module, name) = std_name(type_id).or_else(|| {
        let files = noshape_featurescript::std_files(noshape_featurescript::STD_VERSION).ok()?;
        let head = format!("export const {type_id} = define");
        files.iter().find_map(|(module, src)| {
            let at = src.find(&head)? + "export const ".len();
            Some((*module, &src[at..at + type_id.len()]))
        })
    })?;
    loaded(module, name).map(|f| f.params.clone())
}

/// Evaluate the current feature with std's `name` from `module` (e.g.
/// "sheetMetalFlange.fs", "sheetMetalFlange").
pub fn evaluate(ev: &mut EvalCtx, module: &'static str, name: &'static str) -> FResult<()> {
    super::host::register();
    let Some(f) = loaded(module, name) else { return err(format!("The standard library has no feature {name}")) };
    // The document's tabs, for reference parameters (Variable's CSV file).
    let provider = ev.custom.clone().unwrap_or_else(super::fallback_provider);
    super::run::run_feature(ev, &f.cell, &f.params, &f.specs, name.to_string(), Some(&*provider), None)
}

/// The std evaluator of a built-in feature type, if std runs it; std's module and feature of one (the routing is
/// regeneration's, `regen::std_route`).
pub use crate::regen::std_route::{evaluator, std_name};

/// Run the built-in sheet metal features by std (`NOSHAPE_STD_SHEET_METAL=1` at startup).
pub fn set_std_sheet_metal(on: bool) {
    crate::register();
    crate::regen::std_route::set_std_sheet_metal(on);
}

/// Run these core feature types by std (empty: all native again).
pub fn set_std_route(types: &[&str]) {
    crate::register();
    crate::regen::std_route::set_std_route(types);
}

/// Std's editing logic for feature `index` of `studio` (a built-in feature
/// std runs, e.g. Tab: picking the profile fills "Flange to merge" and the
/// subtraction offset, live qa5 4.2), run on the Part Studio just before it:
/// the parameters it changes when `old` becomes `new`. `specified`: the
/// parameters the user has set. `None`: no editing logic, or it failed.
#[allow(clippy::too_many_arguments)]
pub fn editing_logic(
    studio: &crate::partstudio::PartStudio,
    index: usize,
    config: &crate::configuration::ConfigValues,
    units: &noshape_units::UnitPrefs,
    opts: &crate::regen::RegenOptions,
    old: &crate::params::ParamValues,
    new: &crate::params::ParamValues,
    specified: &[String],
    creating: bool,
) -> Option<crate::params::ParamValues> {
    let feature = studio.features.get(index)?;
    let before = crate::regen::RegenOptions { upto: Some(index), ..opts.clone() };
    let ctx = crate::regen::Regenerator::new().regenerate(studio, config, units, &before).context;
    editing_logic_in((*ctx).clone(), &feature.type_id, feature.id, &feature.name, units, old, new, specified, creating)
}

/// Core features whose dialogs run std's editing logic although they
/// regenerate natively (std Extrude: Add and the merge scope from the trial
/// tool's collisions, flips for Up to and Remove, the second direction;
/// std Fillet: a variable fillet's first vertex takes the fillet's values;
/// std Loft: Add and the merge scope as Extrude, profiles kept across a
/// creation type change, adjacent faces for matched end conditions; Revolve,
/// Sweep, Mirror: the same boolean step; Plane: its type from the picks;
/// Hole: Merge scope and flip from an opHole probe, standard tables, tapped
/// depths (holeEditLogic);
/// Draft: parting-line hint faces and flips; Split: Keep tools for a face or
/// plane tool, live "Split"; Modify fillet: Radius from the picked
/// fillet; 3D fit spline: preselected edges switch it to Edges, the start /
/// end direction toggles follow their fields; External thread: the
/// standard size from the picked cylinder, chamfer and undercut from it;
/// Frame: a new corner override takes the default corner type and flip;
/// Cut list: only the first override hides "Same frames as previous";
/// Delete part: its two entity fields in step; Wrap: Merge scope and
/// Add / Remove from the target part, the flip; Boundary surface, Ruled
/// surface: Add on a surface's open edge, Ruled's reference faces;
/// Bridging curve: the side flags its fields show by, sides from the
/// preselection; Projected curve: the preselection, the flip; Edit curve: an
/// edit's offsets reset with its reference; Body draft: the draft scope from
/// the preselection, a parting plane as the pull direction; Routing curve:
/// "Point 0" at the origin to start, points kept in step).
const STD_EDITING_LOGIC: &[&str] = &["extrude", "fillet", "loft", "revolve", "sweep", "mirror", "cPlane", "draft", "splitPart", "hole", "modifyFillet", "fitSpline", "externalThread", "frame", "cutlist", "deleteBodies", "wrap", "boundarySurface", "ruledSurface", "bridgingCurve", "projectCurves", "editCurve", "bodyDraft", "routingCurve"];

/// Built-in feature types std only runs the editing logic (or, for Helix,
/// the manipulator change function) of (std's module and feature; they
/// regenerate natively and std can't run them). Thicken's
/// dialog keeps its native logic (`ops::studio::auto_operation`, live L18);
/// std's is the T2 check of it.
const LOGIC_ONLY: &[(&str, &str, &str)] = &[("thicken", "thicken.fs", "thicken"), ("modifyFillet", "modifyFillet.fs", "modifyFillet"), ("fitSpline", "fitSpline.fs", "fitSpline"), ("externalThread", "externalThread.fs", "externalThread"), ("frame", "frame.fs", "frame"), ("cutlist", "cutlist.fs", "cutlist"), ("deleteBodies", "deleteBodies.fs", "deleteBodies"), ("wrap", "wrap.fs", "wrap"), ("boundarySurface", "bsurf.fs", "boundarySurface"), ("ruledSurface", "ruledSurface.fs", "ruledSurface"), ("bridgingCurve", "bridgingCurve.fs", "bridgingCurve"), ("projectCurves", "projectCurves.fs", "projectCurves"), ("editCurve", "editCurve.fs", "editCurve"), ("bodyDraft", "bodyDraft.fs", "bodyDraft"), ("routingCurve", "routingCurve.fs", "routingCurve"), ("helix", "helix.fs", "helix"), ("faceBlend", "faceBlend.fs", "faceBlend"), ("mutualTrim", "mutualTrim.fs", "mutualTrim"), ("gusset", "gusset.fs", "gusset"), ("endcap", "endcap.fs", "endcap"), ("decal", "decal.fs", "decal"), ("offsetCurveOnFace", "offsetCurveOnFace.fs", "offsetCurveOnFace")];

/// [`std_name`], or the std feature whose editing logic a dialog runs.
fn logic_std_name(type_id: &str) -> Option<(&'static str, &'static str)> {
    std_name(type_id).or_else(|| LOGIC_ONLY.iter().find(|(t, ..)| *t == type_id).map(|&(_, m, n)| (m, n)))
}

/// Whether std has editing logic for built-in feature `type_id` and the
/// dialog runs it (std runs the feature, or it is in STD_EDITING_LOGIC), or
/// custom feature `type_id` declares one.
pub fn has_editing_logic(type_id: &str) -> bool {
    if let Some(d) = custom_def(type_id) {
        return d.editing.is_some();
    }
    (evaluator(type_id).is_some() || STD_EDITING_LOGIC.contains(&type_id)) && logic_std_name(type_id).and_then(|(module, name)| loaded(module, name)).is_some_and(|f| f.editing.is_some())
}

/// A custom feature type's definition, from the Feature Studios last
/// compiled (the document's, in the app and the agent API).
fn custom_def(type_id: &str) -> Option<super::CustomFeatureDef> {
    let (tab, name) = super::parse_type_id(type_id)?;
    super::fallback_provider().studio(tab)?.feature(name).cloned()
}

/// [`editing_logic`] in `ctx`, the Part Studio just before the feature (a
/// dialog's regeneration context).
#[allow(clippy::too_many_arguments)]
pub fn editing_logic_in(
    ctx: crate::regen::context::Context,
    type_id: &str,
    feature: crate::reference::FeatureId,
    name: &str,
    units: &noshape_units::UnitPrefs,
    old: &crate::params::ParamValues,
    new: &crate::params::ParamValues,
    specified: &[String],
    creating: bool,
) -> Option<crate::params::ParamValues> {
    editing_logic_in_clicked(ctx, type_id, feature, name, units, old, new, specified, creating, "")
}

/// [`editing_logic_in`] after a click on the dialog's button `clicked`
/// (std's `clickedButton`: Routing curve's "processInputs", "resetTriad").
#[allow(clippy::too_many_arguments)]
pub fn editing_logic_in_clicked(
    ctx: crate::regen::context::Context,
    type_id: &str,
    feature: crate::reference::FeatureId,
    name: &str,
    units: &noshape_units::UnitPrefs,
    old: &crate::params::ParamValues,
    new: &crate::params::ParamValues,
    specified: &[String],
    creating: bool,
    clicked: &str,
) -> Option<crate::params::ParamValues> {
    if let Some(d) = custom_def(type_id) {
        return super::run::run_editing_logic(ctx, feature, name, units, d.editing.as_ref()?, &d.params, &d.feature_type.params, old, new, specified, creating, clicked);
    }
    let (module, std) = logic_std_name(type_id)?;
    let f = loaded(module, std)?;
    let editing = f.editing.as_ref()?;
    let (old, new) = (hole_v2_synced(type_id, old), hole_v2_synced(type_id, new));
    super::run::run_editing_logic(ctx, feature, name, units, editing, &f.params, &f.specs, &old, &new, specified, creating, clicked)
}

/// A Hole's hidden V2 sizes as std's `syncHoleDefinitionV3Params` leaves them after every run of its editing logic:
/// the V3 ones. Ours keeps only the V3 parameters, and holeEditLogic copies the V2 ones back when the table changes
/// (a remembered 13.44 mm countersink became the 10 mm default on the Countersink tab: ui-parity
/// hole-countersink-tab).
pub(crate) fn hole_v2_synced(type_id: &str, values: &crate::params::ParamValues) -> crate::params::ParamValues {
    const V2_V3: [(&str, &str); 10] = [
        ("holeDiameterV2", "holeDiameterV3"),
        ("tapDrillDiameterV2", "tapDrillDiameterV3"),
        ("cBoreDiameter", "cBoreDiameterV3"),
        ("cBoreDepth", "cBoreDepthV3"),
        ("cSinkDiameter", "cSinkDiameterV3"),
        ("cSinkAngle", "cSinkAngleV3"),
        ("holeDepthComputed", "holeDepthComputedV3"),
        ("holeDepth", "holeDepthV3"),
        ("tipAngle", "tipAngleV3"),
        ("tappedDepth", "tappedDepthV3"),
    ];
    let mut v = values.clone();
    if type_id == "hole" {
        for (v2, v3) in V2_V3 {
            if let Some(x) = values.get(v3).filter(|_| values.get(v2).is_none()) {
                v.set(v2, x.clone());
            }
        }
    }
    v
}

/// Std's "Manipulator Change Function" of built-in feature `type_id` after
/// its manipulator `key` moved to `manipulator` (a drag, a click on a flip
/// arrow), run in `ctx`, the Part Studio just before the feature, on the
/// dialog's `values`: the parameters it changes. `None`: no such function,
/// or it failed.
#[allow(clippy::too_many_arguments)]
pub fn manipulator_change_in(
    ctx: crate::regen::context::Context,
    type_id: &str,
    feature: crate::reference::FeatureId,
    name: &str,
    units: &noshape_units::UnitPrefs,
    values: &crate::params::ParamValues,
    key: &str,
    manipulator: &crate::display::FeatureManipulator,
) -> Option<crate::params::ParamValues> {
    if let Some(d) = custom_def(type_id) {
        return super::run::run_manipulator_change(ctx, feature, name, units, d.manipulator_change.as_ref()?, &d.params, &d.feature_type.params, values, key, manipulator);
    }
    let (module, std) = logic_std_name(type_id)?;
    let f = loaded(module, std)?;
    let change = f.manipulator_change.as_ref()?;
    super::run::run_manipulator_change(ctx, feature, name, units, change, &f.params, &f.specs, values, key, manipulator)
}

/// The dialog std's precondition declares for built-in feature `type_id`
/// (its parameters only, in std's order): the oracle the built-in schema
/// is compared with (tests/std_oracle.rs).
pub fn std_dialog(type_id: &str) -> Option<Vec<crate::params::ParamSpec>> {
    let (module, name) = std_name(type_id)?;
    let f = loaded(module, name)?;
    Some(super::studio::param_specs(&f.params))
}

/// The dialog of std's feature `name` in `module`, whether or not std runs
/// it here (the oracle's audit of every built-in dialog, tests/std_oracle.rs).
pub fn std_dialog_of(module: &str, name: &'static str) -> Option<Vec<crate::params::ParamSpec>> {
    let f = loaded(module, name)?;
    Some(super::studio::param_specs(&f.params))
}
