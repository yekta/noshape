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
}

/// Loaded std features by name (`None`: not found).
type Loaded = HashMap<&'static str, Option<Arc<StdFeature>>>;

static FEATURES: LazyLock<Mutex<Loaded>> = LazyLock::new(Default::default);

fn load(module: &str, name: &str) -> Option<Arc<StdFeature>> {
    let m = noshape_featurescript::std_module(module).ok()?;
    let decl = m.decls.iter().find(|d| &*d.name == name)?;
    let DeclKind::Const(cell) = &decl.kind else { return None };
    let item = m.ast.items.iter().find(|it| matches!(&it.kind, TopKind::Const { name: n, .. } if &**n == name))?;
    let TopKind::Const { init, .. } = &item.kind else { return None };
    let lambda = defined_lambda(init, "defineSheetMetalFeature").or_else(|| defined_lambda(init, "defineFeature"))?;
    let mut params = Vec::new();
    if let Some(p) = &lambda.precondition {
        collect_params(&m, p, &mut params);
    }
    let mut specs = crate::features::get(name).map(|t| t.params.clone()).unwrap_or_default();
    let extra: Vec<_> = super::studio::param_specs(&params).into_iter().filter(|s| !specs.iter().any(|b| b.id == s.id)).collect();
    specs.extend(extra);
    Some(Arc::new(StdFeature { cell: cell.clone(), params, specs }))
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

macro_rules! std_features {
    ($($f:ident => $module:literal, $name:literal;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            evaluate(ev, $module, $name)
        })*

        /// The std evaluator of a built-in feature type, if std runs it.
        pub fn evaluator(type_id: &str) -> Option<crate::regen::features::EvalFn> {
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
    pcb_start => "pcbStart.fs", "pcbStart";
    pcb_end => "pcbEnd.fs", "pcbEnd";
}
