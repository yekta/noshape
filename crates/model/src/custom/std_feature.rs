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
}

/// Loaded std features by name (`None`: not found).
type Loaded = HashMap<&'static str, Option<Arc<StdFeature>>>;

static FEATURES: LazyLock<Mutex<Loaded>> = LazyLock::new(Default::default);

fn load(module: &str, name: &str) -> Option<Arc<StdFeature>> {
    // Natives bind when std compiles: register them first.
    super::host::register();
    let m = noshape_featurescript::std_module(module).ok()?;
    let decl = m.decls.iter().find(|d| &*d.name == name)?;
    let DeclKind::Const(cell) = &decl.kind else { return None };
    let item = m.ast.items.iter().find(|it| matches!(&it.kind, TopKind::Const { name: n, .. } if &**n == name))?;
    let TopKind::Const { init, .. } = &item.kind else { return None };
    let editing = item
        .annotation
        .as_ref()
        .map(|a| noshape_featurescript::annot::eval(&m, a))
        .and_then(|ann| ann.str("Editing Logic Function").map(str::to_string))
        .and_then(|f| m.functions(&f));
    let lambda = defined_lambda(init, "defineSheetMetalFeature").or_else(|| defined_lambda(init, "defineFeature"))?;
    let mut params = Vec::new();
    if let Some(p) = &lambda.precondition {
        collect_params(&m, p, &mut params);
    }
    // The built-in feature std's feature stands for (Transform: fTransform).
    let ty = CORE.iter().find(|c| c.2 == name).map_or(name, |c| c.0);
    let mut specs = crate::features::get(ty).map(|t| t.params.clone()).unwrap_or_default();
    let extra: Vec<_> = super::studio::param_specs(&params).into_iter().filter(|s| !specs.iter().any(|b| b.id == s.id)).collect();
    specs.extend(extra);
    Some(Arc::new(StdFeature { cell: cell.clone(), params, specs, editing }))
}

/// Evaluate the current feature with std's `name` from `module` (e.g.
/// "sheetMetalFlange.fs", "sheetMetalFlange").
pub fn evaluate(ev: &mut EvalCtx, module: &'static str, name: &'static str) -> FResult<()> {
    super::host::register();
    let f = FEATURES.lock().entry(name).or_insert_with(|| load(module, name)).clone();
    let Some(f) = f else { return err(format!("The standard library has no feature {name}")) };
    super::run::run_feature(ev, &f.cell, &f.params, &f.specs, name.to_string(), None)
}

/// Built-in sheet metal features run by std (switched on by
/// [`set_std_sheet_metal`] or `NOSHAPE_STD_SHEET_METAL=1` until the std
/// route passes every native sheet metal test; Flex PCB always runs by std).
static STD_SHEET_METAL: LazyLock<std::sync::atomic::AtomicBool> = LazyLock::new(|| std::sync::atomic::AtomicBool::new(std::env::var("NOSHAPE_STD_SHEET_METAL").is_ok_and(|v| v == "1")));

pub fn set_std_sheet_metal(on: bool) {
    STD_SHEET_METAL.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Core features (Extrude, Fillet, ...) run natively; [`set_std_route`]
/// makes std run them instead, the oracle the native ones are tested
/// against (tests/std_oracle.rs).
static STD_ROUTE: LazyLock<parking_lot::RwLock<Vec<String>>> = LazyLock::new(Default::default);

/// Run these core feature types by std (empty: all native again).
pub fn set_std_route(types: &[&str]) {
    *STD_ROUTE.write() = types.iter().map(|t| t.to_string()).collect();
}

macro_rules! core_features {
    ($($f:ident => $ty:literal, $module:literal, $name:literal;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            evaluate(ev, $module, $name)
        })*

        /// Core features std can run: feature type, std module and feature, evaluator.
        const CORE: &[(&str, &str, &str, crate::regen::features::EvalFn)] = &[$(($ty, $module, $name, $f),)*];
    };
}

core_features! {
    core_extrude => "extrude", "extrude.fs", "extrude";
    core_fillet => "fillet", "fillet.fs", "fillet";
    core_hole => "hole", "hole.fs", "hole";
    core_loft => "loft", "loft.fs", "loft";
    core_transform => "transform", "transformCopy.fs", "fTransform";
    core_revolve => "revolve", "revolve.fs", "revolve";
    core_chamfer => "chamfer", "chamfer.fs", "chamfer";
    core_shell => "shell", "shell.fs", "shell";
    core_sweep => "sweep", "sweep.fs", "sweep";
}

macro_rules! std_features {
    ($($f:ident => $module:literal, $name:literal;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            evaluate(ev, $module, $name)
        })*

        /// The std module and feature of a built-in feature type.
        fn std_name(type_id: &str) -> Option<(&'static str, &'static str)> {
            if let Some(&(_, module, name, _)) = CORE.iter().find(|(t, ..)| *t == type_id) {
                return Some((module, name));
            }
            Some(match type_id {
                $($name => ($module, $name),)*
                _ => return None,
            })
        }

        /// The std evaluator of a built-in feature type, if std runs it.
        pub fn evaluator(type_id: &str) -> Option<crate::regen::features::EvalFn> {
            if let Some(&(.., f)) = CORE.iter().find(|(t, ..)| *t == type_id)
                && STD_ROUTE.read().iter().any(|t| t == type_id)
            {
                return Some(f);
            }
            if !STD_SHEET_METAL.load(std::sync::atomic::Ordering::Relaxed) && !matches!(type_id, "pcbStart" | "pcbEnd") {
                return None;
            }
            Some(match type_id {
                $($name => $f,)*
                _ => return None,
            })
        }
    };
}

std_features! {
    sheet_metal_start => "sheetMetalStart.fs", "sheetMetalStart";
    sheet_metal_end => "sheetMetalEnd.fs", "sheetMetalEnd";
    flange => "sheetMetalFlange.fs", "sheetMetalFlange";
    hem => "sheetMetalHem.fs", "sheetMetalHem";
    tab => "sheetMetalTab.fs", "sheetMetalTab";
    bend => "sheetMetalBend.fs", "sheetMetalBend";
    jog => "sheetMetalJog.fs", "sheetMetalJog";
    formed => "sheetMetalFormed.fs", "sheetMetalFormed";
    loft => "sheetMetalLoft.fs", "sheetMetalLoft";
    make_joint => "sheetMetalMakeJoint.fs", "sheetMetalMakeJoint";
    joint => "sheetMetalJoint.fs", "sheetMetalJoint";
    corner => "sheetMetalCorner.fs", "sheetMetalCorner";
    bend_relief => "sheetMetalBendRelief.fs", "sheetMetalBendRelief";
    corner_break => "sheetMetalCornerBreak.fs", "sheetMetalCornerBreak";
    rip => "sheetMetalRip.fs", "sheetMetalRip";
    // Tag (std tag.fs): its form / frame attributes are what std Form and Frame import.
    tag => "tag.fs", "tag";
    pcb_start => "pcbStart.fs", "pcbStart";
    pcb_end => "pcbEnd.fs", "pcbEnd";
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
) -> Option<crate::params::ParamValues> {
    let feature = studio.features.get(index)?;
    let before = crate::regen::RegenOptions { upto: Some(index), ..opts.clone() };
    let ctx = crate::regen::Regenerator::new().regenerate(studio, config, units, &before).context;
    editing_logic_in((*ctx).clone(), &feature.type_id, feature.id, &feature.name, units, old, new, specified)
}

/// Core features whose dialogs run std's editing logic although they
/// regenerate natively (std Extrude: Add and the merge scope from the trial
/// tool's collisions, flips for Up to and Remove, the second direction).
const STD_EDITING_LOGIC: &[&str] = &["extrude"];

/// Whether std has editing logic for built-in feature `type_id` and the
/// dialog runs it (std runs the feature, or it is in STD_EDITING_LOGIC).
pub fn has_editing_logic(type_id: &str) -> bool {
    (evaluator(type_id).is_some() || STD_EDITING_LOGIC.contains(&type_id)) && std_name(type_id).and_then(|(module, name)| FEATURES.lock().entry(name).or_insert_with(|| load(module, name)).clone()).is_some_and(|f| f.editing.is_some())
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
) -> Option<crate::params::ParamValues> {
    let (module, std) = std_name(type_id)?;
    let f = FEATURES.lock().entry(std).or_insert_with(|| load(module, std)).clone()?;
    let editing = f.editing.as_ref()?;
    super::run::run_editing_logic(ctx, feature, name, units, editing, &f.params, &f.specs, old, new, specified)
}

/// The dialog std's precondition declares for built-in feature `type_id`
/// (its parameters only, in std's order): the oracle the built-in schema
/// is compared with (tests/std_oracle.rs).
pub fn std_dialog(type_id: &str) -> Option<Vec<crate::params::ParamSpec>> {
    let (module, name) = std_name(type_id)?;
    let f = FEATURES.lock().entry(name).or_insert_with(|| load(module, name)).clone()?;
    Some(super::studio::param_specs(&f.params))
}
