//! Running a FeatureScript custom feature inside a Part Studio regeneration.
//!
//! The feature value (`defineFeature(...)`) is called with a `Context`
//! wrapping the regeneration context, the feature's id (`makeId("F1a2b")`)
//! and the definition built from the stored parameters. `defineFeature`'s
//! own machinery reports errors as the feature's status and rolls the
//! context back (`@abortFeature`); the status becomes the feature error.

use super::host::{self, FsContext, query::query_of_refs};
use super::studio::{ParamDecl, ParamKindFs};
use crate::params::{NumKind, ParamValue, ParamValues, ReferenceKind, fill_defaults};
use crate::reference::FeatureId;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, err};
use noshape_featurescript::interp::{Interp, Notice};
use noshape_featurescript::ir::GlobalCell;
use noshape_featurescript::value::{Map, Value, type_info};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

/// Regeneration time limit for one feature.
const TIME_LIMIT: Duration = Duration::from_secs(60);
/// Stack for the evaluator (deeply recursive FeatureScript).
const STACK_SIZE: usize = 256 << 20;

/// Output of the last evaluation of a custom feature (notices, `println`),
/// for the FeatureScript notices panel's Monitor.
#[derive(Clone, Debug, Default)]
pub struct FeatureOutput {
    /// Feature type id (`custom:<tab>:<name>`).
    pub type_id: String,
    /// The Part Studio feature's name ("FS probe 1"), the last stack line.
    pub feature_name: String,
    pub notices: Vec<Notice>,
    pub output: String,
    /// Time per (module, line) while profiling ([`set_profiling`]).
    pub line_times: Vec<(Arc<str>, u32, std::time::Duration)>,
}

static PROFILING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Feature Studio "Profile <Part Studio>": time every FeatureScript line of
/// the custom features run from now on.
pub fn set_profiling(on: bool) {
    PROFILING.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn profiling() -> bool {
    PROFILING.load(std::sync::atomic::Ordering::Relaxed)
}

static OUTPUTS: LazyLock<Mutex<HashMap<FeatureId, FeatureOutput>>> = LazyLock::new(Default::default);

static OUTPUT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Changes whenever a custom feature's output is recorded (views showing
/// [`monitor`] refresh on it).
pub fn output_generation() -> u64 {
    OUTPUT_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Notices and printed output of the last run of each custom feature.
pub fn outputs() -> HashMap<FeatureId, FeatureOutput> {
    OUTPUTS.lock().clone()
}

/// Notices and printed output of the last run of one Part Studio feature.
pub fn output_of(feature: FeatureId) -> Option<FeatureOutput> {
    OUTPUTS.lock().get(&feature).cloned()
}

/// Build the definition map for `params`.
fn definition(cx: &Arc<FsContext>, decls: &[ParamDecl], values: &ParamValues, ev: &EvalCtx, provider: Option<&dyn super::CustomFeatureProvider>) -> FResult<Value> {
    let mut m = Map::new();
    for d in decls {
        let v = match &d.kind {
            ParamKindFs::Num { kind, .. } => {
                let x = ev.eval_num_in(values, &d.id, *kind)?;
                match kind {
                    NumKind::Length => host::length_value(x),
                    NumKind::Angle => host::angle_value(x),
                    NumKind::Integer => Value::num(x.round()),
                    NumKind::Real => Value::num(x),
                }
            }
            ParamKindFs::Bool { .. } => Value::bool(values.bool(&d.id)),
            ParamKindFs::Text { .. } => Value::str(values.text(&d.id)),
            ParamKindFs::Enum { tag, default, .. } => {
                let s = values.enum_(&d.id);
                let info = type_info(*tag);
                let o = info.ordinal(s).or_else(|| info.ordinal(default)).unwrap_or(0);
                Value::Enum(*tag, o)
            }
            ParamKindFs::Query { .. } => {
                let refs = values.query(&d.id);
                query_of_refs(cx, refs)
            }
            ParamKindFs::FeatureList { .. } => host::feature_list_value(values.query(&d.id)),
            ParamKindFs::Array { items, .. } => {
                let mut arr = Vec::new();
                for it in values.array(&d.id) {
                    arr.push(definition(cx, items, it, ev, provider)?);
                }
                Value::array(arr)
            }
            // A LookupTablePath: { level key : chosen option } along the path.
            ParamKindFs::Lookup { table } => {
                let path = crate::features::hole_tables::complete_path(table, values.lookup(&d.id));
                let mut m = Map::new();
                for k in 0..path.len() {
                    if let Some(level) = crate::features::hole_tables::dynamic_level(table, &path[..k]) {
                        m.insert(Value::str(level.key.as_str()), Value::str(path[k].as_str()));
                    }
                }
                Value::map(m).with_tag(host::std_tag("lookupTablePath.fs", "LookupTablePath"))
            }
            ParamKindFs::Reference { kind: ReferenceKind::PartStudio } => super::reference::part_studio_value(values.text(&d.id), ev.derived.as_deref()),
            ParamKindFs::Reference { kind } => {
                let tab = values.text(&d.id);
                // Other documents and versions: `ext:` keys, read through the Derived provider.
                let file = if tab.is_empty() {
                    None
                } else if tab.starts_with("ext:") || tab.contains('/') {
                    ev.derived.as_ref().and_then(|d| d.file(tab))
                } else {
                    provider.and_then(|p| p.file(tab))
                };
                super::reference::reference_value(*kind, file.as_ref())
            }
            // A group title is not a parameter.
            ParamKindFs::Group { .. } => Value::Undefined,
            ParamKindFs::Anything => match values.get(&d.id) {
                Some(ParamValue::Text(s)) => Value::str(s.as_str()),
                _ => Value::Undefined,
            },
        };
        if !v.is_undefined() {
            m.insert(Value::str(d.id.as_str()), v);
        }
    }
    Ok(Value::map(m))
}

/// The first visible top-level number parameter outside its bounds.
fn out_of_bounds(ev: &EvalCtx, specs: &[crate::params::ParamSpec], values: &ParamValues) -> Option<&'static str> {
    specs.iter().filter(|s| s.is_visible(values)).find_map(|s| match &s.kind {
        crate::params::ParamKind::Num(n) => {
            let v = ev.eval_num_in(values, s.id, n.kind).ok()?;
            crate::regen::eval::bounds_violation(v, n, ev.units).map(|_| s.id)
        }
        _ => None,
    })
}

/// Evaluator for every `custom:<tab>:<name>` feature type.
pub fn evaluate(ev: &mut EvalCtx) -> FResult<()> {
    let Some(type_id) = ev.custom_type.clone().or_else(|| ev.features.iter().find(|f| f.id == ev.feature).map(|f| f.type_id.clone())) else {
        return err("Custom feature type is unknown");
    };
    let Some((tab, name)) = super::parse_type_id(&type_id) else { return err("Invalid custom feature type") };
    let provider = ev.custom.clone().unwrap_or_else(super::fallback_provider);
    let Some(studio) = provider.studio(tab) else {
        return err(format!("The Feature Studio defining \"{name}\" no longer exists"));
    };
    if let Some(d) = studio.first_error() {
        return err(format!("Feature Studio has errors: {d}"));
    }
    let Some(def) = studio.feature(name).cloned() else {
        return err(format!("The Feature Studio no longer defines \"{name}\""));
    };

    run_feature(ev, &def.cell, &def.params, &def.feature_type.params, type_id, Some(&*provider))
}

/// Run a feature lambda (`cell`: `(context, id, definition)`) on the Part
/// Studio context, its definition built from `params` (the precondition's
/// parameters) and the feature's values (`specs` fill the defaults); used for
/// custom features and for built-in features run by the standard library.
pub(crate) fn run_feature(ev: &mut EvalCtx, cell: &Arc<GlobalCell>, params: &[ParamDecl], specs: &[crate::params::ParamSpec], type_id: String, provider: Option<&dyn super::CustomFeatureProvider>) -> FResult<()> {
    let mut values: ParamValues = ev.params.clone();
    fill_defaults(specs, &mut values, ev.units);
    // Live (FS probe at Size 0.5 mm): "A parameter value is not valid.",
    // the field marked.
    if let Some(id) = out_of_bounds(ev, specs, &values) {
        return Err(FeatureError { faulty: vec![id], ..FeatureError::new("A parameter value is not valid.") });
    }

    // Printed object numbers restart per feature run (the Context is "BuiltIn: 0").
    noshape_featurescript::value::reset_shown_numbers();
    let cx = FsContext::new(std::mem::take(&mut ev.ctx), ev.feature, ev.name.to_string(), *ev.units, ev.outer_vars.clone(), ev.xform);
    let definition = match definition(&cx, params, &values, ev, provider) {
        Ok(d) => d,
        Err(e) => {
            ev.ctx = cx.take_context();
            return Err(e);
        }
    };
    let feature_id = vec![Arc::<str>::from(ev.feature.to_string())];
    let ctx_value = cx.value();
    let cell = cell.clone();
    let counters = noshape_featurescript::value::shown_counters();
    let derived = ev.derived.clone();

    // Run on a thread with a large stack (FeatureScript recursion).
    let run = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(s, || {
                noshape_featurescript::value::set_shown_counters(counters);
                host::set_derived(derived);
                let mut i = Interp::new();
                i.set_deadline(Some(Instant::now() + TIME_LIMIT));
                if profiling() {
                    i.profile = Some(Default::default());
                }
                let f = i.global(&cell);
                let r = f.and_then(|f| i.call_value(&f, vec![ctx_value, host::id_value(&feature_id), definition]));
                let lines = i.profile.take().map(|p| p.times.into_iter().map(|((m, l), t)| (m, l, t)).collect()).unwrap_or_default();
                (r.map(|_| ()).map_err(|e| e.message()), std::mem::take(&mut i.notices), std::mem::take(&mut i.output), lines)
            })
            .map(|h| h.join())
    });
    let (result, notices, output, line_times) = match run {
        Ok(Ok(r)) => r,
        _ => (Err("Feature evaluation crashed".to_string()), Vec::new(), String::new(), Vec::new()),
    };
    OUTPUT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    OUTPUTS.lock().insert(ev.feature, FeatureOutput { type_id: type_id.clone(), feature_name: ev.name.to_string(), notices, output, line_times });

    ev.ctx = cx.take_context();
    let st = cx.state.lock();
    let top = vec![Arc::<str>::from(ev.feature.to_string())];
    let status = st.status.get(&top).cloned();
    drop(st);
    if let Err(m) = result {
        return err(m);
    }
    if let Some(s) = status {
        let kind = s.get("statusType").as_str().unwrap_or_default();
        let text = match (s.get("statusEnum").as_str(), s.get("statusMsg").as_str()) {
            (Some(e), Some(m)) if &*e == "CUSTOM_ERROR" => m.to_string(),
            (Some(e), _) => host::error_text(&e),
            (None, Some(m)) => m.to_string(),
            _ => host::error_text("REGEN_ERROR"),
        };
        // std's faulty parameters mark those fields red (regenError's array).
        let faulty: Vec<&'static str> = match s.get("faultyParameters") {
            Value::Array(a, _) => a.iter().filter_map(|v| v.as_str()).filter_map(|p| specs.iter().find(|x| *x.id == *p).map(|x| x.id)).collect(),
            _ => Vec::new(),
        };
        match &*kind {
            "ERROR" => return Err(FeatureError { faulty, ..FeatureError::new(text) }),
            "WARNING" => ev.warnings.push(text),
            // reportFeatureInfo (std Finish sheet metal model): an Info status.
            "INFO" => ev.infos.push(text),
            _ => {}
        }
    }
    Ok(())
}

/// Run an "Editing Logic Function" (std's `sheetMetalTabEditingLogic`, a
/// custom feature's) as Onshape does while a dialog is edited:
/// `f(context, id, oldDefinition, definition, specifiedParameters,
/// hiddenBodies)` in `ctx`, the Part Studio just before the feature.
/// `specified`: the parameters the user has set; `creating`: the feature is
/// new (`isCreating` of seven-parameter functions). Returns the parameters the
/// function changed (queries as references, numbers as literals in
/// `units`); `None` if it failed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_editing_logic(
    ctx: crate::regen::context::Context,
    feature: FeatureId,
    name: &str,
    units: &noshape_units::UnitPrefs,
    f: &Arc<noshape_featurescript::ir::OverloadSet>,
    decls: &[ParamDecl],
    specs: &[crate::params::ParamSpec],
    old: &ParamValues,
    new: &ParamValues,
    specified: &[String],
    creating: bool,
) -> Option<ParamValues> {
    host::register();
    let (mut olds, mut news) = (old.clone(), new.clone());
    fill_defaults(specs, &mut olds, units);
    fill_defaults(specs, &mut news, units);
    let hidden: Vec<crate::reference::Reference> = ctx.fs_hidden_bodies.iter().map(|b| crate::reference::Reference::Body(b.id.clone())).collect();
    let outer = std::collections::BTreeMap::new();
    let ev = EvalCtx::new(feature, name, &news, units, Default::default(), &outer);
    let cx = FsContext::new(ctx, feature, name.to_string(), *units, outer.clone(), None);
    let old_def = definition(&cx, decls, &olds, &ev, None).ok()?;
    let new_def = definition(&cx, decls, &news, &ev, None).ok()?;
    let mut flags = Map::new();
    for d in decls {
        flags.insert(Value::str(d.id.as_str()), Value::bool(specified.contains(&d.id)));
    }
    let mut args = vec![cx.value(), host::id_value(&[Arc::<str>::from(feature.to_string())]), old_def, new_def.clone(), Value::map(flags), query_of_refs(&cx, &hidden)];
    // Seven parameters: `isCreating` after the definition (std filletEditLogic).
    if f.fns.iter().any(|d| d.arity() == 7) {
        args.insert(4, Value::bool(creating));
    }
    let func = Value::Function(noshape_featurescript::value::Shared::new(noshape_featurescript::value::Func::Overloads(f.clone())));
    let units = *units;
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(s, || {
                let mut i = Interp::new();
                i.set_deadline(Some(Instant::now() + TIME_LIMIT));
                let out = i.call_value(&func, args).ok()?;
                let mut changed = ParamValues::default();
                for d in decls {
                    let (v, was) = (out.get(d.id.as_str()), new_def.get(d.id.as_str()));
                    if v.is_undefined() || v == was {
                        continue;
                    }
                    if let Some(pv) = editing_value(&mut i, &cx, d, v, &units) {
                        changed.set(&d.id, pv);
                    }
                }
                Some(changed)
            })
            .ok()?
            .join()
            .ok()?
    })
}

/// A definition value an editing logic function returned, as the
/// parameter value of `d` (arrays item by item); `None`: not representable.
fn editing_value(i: &mut Interp, cx: &Arc<FsContext>, d: &ParamDecl, v: &Value, units: &noshape_units::UnitPrefs) -> Option<ParamValue> {
    Some(match &d.kind {
        ParamKindFs::Query { .. } => ParamValue::Query(host::query::evaluate_refs(i, cx, v).ok()?),
        ParamKindFs::FeatureList { .. } => ParamValue::Query(host::feature_list_refs(v)),
        ParamKindFs::Bool { .. } => ParamValue::Bool(v.as_bool()?),
        ParamKindFs::Num { kind, .. } => {
            let q = match kind {
                NumKind::Length => noshape_units::Quantity::new(host::length_of(v)?, noshape_units::Dim::LENGTH),
                NumKind::Angle => noshape_units::Quantity::new(host::angle_of(v)?, noshape_units::Dim::ANGLE),
                NumKind::Integer | NumKind::Real => noshape_units::Quantity::new(v.as_number()?, noshape_units::Dim::UNITLESS),
            };
            ParamValue::Expr(noshape_units::ExprValue::from_quantity(q, units))
        }
        ParamKindFs::Enum { tag, .. } => match v {
            Value::Enum(t, o) if t == tag => ParamValue::Enum(type_info(*tag).members.get(*o as usize)?.to_string()),
            _ => return None,
        },
        // std filletEditLogic fills a variable fillet's first vertex.
        ParamKindFs::Array { items, .. } => {
            let mut out = Vec::new();
            for it in v.as_array()?.iter() {
                let mut p = ParamValues::default();
                for d in items {
                    let x = it.get(d.id.as_str());
                    if !x.is_undefined()
                        && let Some(pv) = editing_value(i, cx, d, x, units)
                    {
                        p.set(&d.id, pv);
                    }
                }
                out.push(p);
            }
            ParamValue::Array(out)
        }
        _ => return None,
    })
}

/// One entry of the FeatureScript notices panel's Monitor (live
/// dom/fs-probe-output.txt): the message, then stack rows of
/// (`line:col`, `module (function)`), the last row being the Part Studio
/// and feature (`fs probe test (FS probe 1)`, no position).
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorEntry {
    pub message: String,
    pub severity: super::Severity,
    /// (tab, line, column) of the first row, to jump to.
    pub at: Option<(crate::document::TabId, usize, usize)>,
    pub rows: Vec<(String, String)>,
}

/// A runtime notice as a Monitor entry: the message, its stack rows
/// (`line:col`, `module (function)`), innermost first, then `last` (the
/// Part Studio and feature, no position).
fn notice_entry(doc: &crate::document::Document, n: &noshape_featurescript::interp::Notice, last: String) -> MonitorEntry {
    use crate::document::TabId;
    let module_tab = |m: &str| m.strip_prefix("tab:").and_then(|t| u64::from_str_radix(t, 16).ok()).map(TabId);
    let module_name = |m: &str| match module_tab(m) {
        Some(t) => doc.tab(t).map(|t| t.name.clone()).unwrap_or_else(|| m.to_string()),
        None => m.to_string(),
    };
    let place = |module: &str, function: &str| if function.is_empty() { module_name(module) } else { format!("{} ({function})", module_name(module)) };
    let mut rows = Vec::new();
    let innermost = n.trace.last().map(|f| f.function.to_string()).unwrap_or_default();
    rows.push((format!("{}:{}", n.pos.line, n.pos.col), place(&n.module, &innermost)));
    // Call sites, innermost first; the host's call (no module) ends the stack.
    for k in (0..n.trace.len()).rev() {
        let fr = &n.trace[k];
        if fr.module.is_empty() {
            break;
        }
        let caller = if k > 0 { n.trace[k - 1].function.to_string() } else { String::new() };
        rows.push((format!("{}:{}", fr.pos.line, fr.pos.col), place(&fr.module, &caller)));
    }
    rows.push((String::new(), last));
    let at = module_tab(&n.module).map(|t| (t, n.pos.line as usize, n.pos.col as usize));
    let severity = match n.kind {
        noshape_featurescript::NoticeKind::Error => super::Severity::Error,
        noshape_featurescript::NoticeKind::Warning => super::Severity::Warning,
        noshape_featurescript::NoticeKind::Info => super::Severity::Info,
    };
    MonitorEntry { message: n.message.clone(), severity, at, rows }
}

/// Profile of a Part Studio's last regeneration: time per Feature Studio
/// line, summed over its custom features (live "Profile": bars beside the
/// lines that ran).
pub fn profile(doc: &crate::document::Document, part_studio: crate::document::TabId) -> HashMap<crate::document::TabId, std::collections::BTreeMap<u32, std::time::Duration>> {
    use crate::document::{TabContent, TabId};
    let mut out: HashMap<TabId, std::collections::BTreeMap<u32, std::time::Duration>> = HashMap::new();
    let Some(TabContent::PartStudio(ps)) = doc.tab(part_studio).map(|t| &t.content) else { return out };
    for f in ps.features.iter().filter(|f| !f.suppressed && super::is_custom(&f.type_id)) {
        let Some(o) = output_of(f.id) else { continue };
        for (module, line, t) in &o.line_times {
            let Some(tab) = module.strip_prefix("tab:").and_then(|t| u64::from_str_radix(t, 16).ok()).map(TabId) else { continue };
            *out.entry(tab).or_default().entry(*line).or_default() += *t;
        }
    }
    out
}

/// The notices of a custom table run as Monitor entries, ending with
/// `last` ("pvz 2parts (main)", live 652).
pub fn notice_entries(doc: &crate::document::Document, notices: &[noshape_featurescript::interp::Notice], last: &str) -> Vec<MonitorEntry> {
    notices.iter().map(|n| notice_entry(doc, n, last.to_string())).collect()
}

/// Monitor contents of a Part Studio: the compile warnings / infos of the
/// Feature Studios its custom features come from (not parse warnings) and the
/// features' runtime notices, ordered like Onshape's (live fs-errors3.txt):
/// notices in Feature Studios first, then those in the standard library, each
/// by line, a compile notice before runtime ones on its line; then the
/// printed text.
pub fn monitor(doc: &crate::document::Document, part_studio: crate::document::TabId) -> (Vec<MonitorEntry>, String) {
    use crate::document::{TabContent, TabId};
    let Some(tab) = doc.tab(part_studio) else { return (Vec::new(), String::new()) };
    let TabContent::PartStudio(ps) = &tab.content else { return (Vec::new(), String::new()) };
    let module_tab = |m: &str| m.strip_prefix("tab:").and_then(|t| u64::from_str_radix(t, 16).ok()).map(TabId);
    let module_name = |m: &str| match module_tab(m) {
        Some(t) => doc.tab(t).map(|t| t.name.clone()).unwrap_or_else(|| m.to_string()),
        None => m.to_string(),
    };
    // (in the standard library, line, runtime) for the ordering.
    let mut entries: Vec<((bool, u32, bool), MonitorEntry)> = Vec::new();
    let mut text = String::new();
    let mut studios_done = Vec::new();
    let scripts = super::DocumentScripts::from_document(doc);
    for f in ps.features.iter().filter(|f| !f.suppressed && super::is_custom(&f.type_id)) {
        let Some((studio_tab, _)) = super::parse_type_id(&f.type_id) else { continue };
        if !studios_done.contains(&studio_tab)
            && let Some(studio) = super::CustomFeatureProvider::studio(&*scripts, studio_tab)
        {
            studios_done.push(studio_tab);
            for d in studio.diagnostics.iter().filter(|d| d.severity != super::Severity::Error && !d.parse) {
                let pos = if d.line > 0 { format!("{}:{}", d.line, d.column) } else { String::new() };
                entries.push(((false, d.line as u32, false), MonitorEntry { message: d.message.clone(), severity: d.severity, at: (d.line > 0).then_some((studio.tab, d.line, d.column)), rows: vec![(pos, module_name(&format!("tab:{studio_tab}")))] }));
            }
        }
        let Some(out) = output_of(f.id) else { continue };
        for n in out.notices.iter() {
            entries.push(((n.module.starts_with("onshape/std/"), n.pos.line, true), notice_entry(doc, n, format!("{} ({})", tab.name, out.feature_name))));
        }
        text.push_str(&out.output);
    }
    entries.sort_by_key(|(k, _)| *k);
    (entries.into_iter().map(|(_, e)| e).collect(), text)
}
