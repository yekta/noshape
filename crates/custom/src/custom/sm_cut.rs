//! Cutting active sheet metal parts as std's Hole does (hole.fs
//! handleSheetMetalCutAndAttribution): the tools that cut planar walls are
//! registered on them (registerSheetMetalBooleanTools: the walls'
//! `cuttingToolBodyIds`, which the sheet metal rebuild cuts, so a later
//! sheet metal feature keeps the cut), the others subtracted by std's
//! sheet metal aware booleanBodies. Run as a small internal FeatureScript
//! feature on the native Hole's context (a Counterbore / Countersink Hole:
//! SM-CBORE-THEN-FLANGE).

use crate::document::TabId;
use crate::params::{ParamValue, ParamValues};
use crate::reference::{BodyId, Reference};
use crate::regen::eval::{EvalCtx, FResult, err};
use std::sync::LazyLock;

const SRC: &str = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");
import(path : "onshape/std/registerSheetMetalBooleanTools.fs", version : "3095.0");

annotation { "Feature Type Name" : "Sheet metal cut" }
export const smCut = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Targets", "Filter" : EntityType.BODY }
        definition.targets is Query;
        annotation { "Name" : "Tools", "Filter" : EntityType.BODY }
        definition.tools is Query;
    }
    {
        callSubfeatureAndProcessStatus(id, registerSheetMetalBooleanTools, context, id + "register", {
                    "targets" : definition.targets,
                    "subtractiveTools" : definition.tools,
                    "doUpdateSMGeometry" : true
                });
        if (!isQueryEmpty(context, definition.tools))
        {
            callSubfeatureAndProcessStatus(id, booleanBodies, context, id + "boolean", {
                        "targets" : definition.targets,
                        "tools" : definition.tools,
                        "operationType" : BooleanOperationType.SUBTRACTION
                    });
        }
    });
"#;

static STUDIO: LazyLock<std::sync::Arc<crate::regen::custom_features::CompiledStudio>> = LazyLock::new(|| super::studio::compile(TabId(u64::MAX - 0x5c), SRC));

/// Cut `parts` with the `tools` bodies (consumed), in `ev`'s context.
pub fn cut(ev: &mut EvalCtx, parts: &[BodyId], tools: &[BodyId]) -> FResult<()> {
    let Some(def) = STUDIO.feature("smCut").cloned() else {
        return err(format!("The sheet metal cut did not compile: {:?}", STUDIO.first_error()));
    };
    let mut p = ParamValues::default();
    p.set("targets", ParamValue::Query(parts.iter().cloned().map(Reference::Body).collect()));
    p.set("tools", ParamValue::Query(tools.iter().cloned().map(Reference::Body).collect()));
    let mut sub = EvalCtx::new(ev.feature, ev.name, &p, ev.units, std::mem::take(&mut ev.ctx), ev.outer_vars);
    sub.cancel = ev.cancel.clone();
    let res = super::run::run_feature(&mut sub, &def.cell, &def.params, &def.feature_type.params, "smCut".into(), None);
    ev.ctx = sub.ctx;
    ev.warnings.extend(sub.warnings);
    ev.infos.extend(sub.infos);
    res
}
