//! Which built-in features the standard library's own FeatureScript runs instead of their native evaluators (Flex
//! PCB always; sheet metal when switched on; core features on the std route, the oracle the native ones are tested
//! against). The FeatureScript host (noshape-custom, `custom::std_feature`) runs them, through
//! [`super::custom_features::evaluate_std`].

use crate::regen::custom_features::evaluate_std;
use crate::regen::eval::{EvalCtx, FResult};
use crate::regen::features::EvalFn;
use std::sync::LazyLock;

/// Built-in sheet metal features run by std, Onshape's own sheet metal
/// FeatureScript (default; `NOSHAPE_STD_SHEET_METAL=0` or
/// [`set_std_sheet_metal`]`(false)` runs the native evaluators; Flex PCB always
/// runs by std). Without the FeatureScript host registered (a bare
/// model-core regeneration) the native evaluators run.
static STD_SHEET_METAL: LazyLock<std::sync::atomic::AtomicBool> = LazyLock::new(|| std::sync::atomic::AtomicBool::new(std::env::var("NOSHAPE_STD_SHEET_METAL").map_or(true, |v| v != "0")));

pub fn set_std_sheet_metal(on: bool) {
    STD_SHEET_METAL.store(on, std::sync::atomic::Ordering::Relaxed);
}

thread_local! {
    /// The studio regenerating on this thread has sheet metal saved before std
    /// ran it ([`crate::partstudio::PartStudio::legacy_sheet_metal`]): its sheet
    /// metal features run natively.
    static LEGACY_SHEET_METAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Set by a regeneration for its studio (see `LEGACY_SHEET_METAL`) until
/// dropped: a Derived studio regenerating inside another gives the outer one
/// its setting back.
pub struct LegacySheetMetal(bool);

impl LegacySheetMetal {
    pub fn set(on: bool) -> Self {
        Self(LEGACY_SHEET_METAL.replace(on))
    }
}

impl Drop for LegacySheetMetal {
    fn drop(&mut self) {
        LEGACY_SHEET_METAL.set(self.0);
    }
}

thread_local! {
    /// Core features (Extrude, Fillet, ...) run natively; [`set_std_route`]
    /// makes std run them instead, the oracle the native ones are tested
    /// against (tests/std_oracle.rs). Per thread (a regeneration evaluates its
    /// features on the calling thread): process-wide, std_oracle's route
    /// leaked into the tests running beside it (oracle_p1's extrudes came
    /// out as std's "Select face or sketch region to extrude.").
    static STD_ROUTE: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run these core feature types by std on this thread (empty: all native again).
pub fn set_std_route(types: &[&str]) {
    STD_ROUTE.with_borrow_mut(|r| *r = types.iter().map(|t| t.to_string()).collect());
}

macro_rules! core_features {
    ($($f:ident => $ty:literal, $module:literal, $name:literal;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            evaluate_std(ev, $module, $name)
        })*

        /// Core features std can run: feature type, std module and feature, evaluator.
        const CORE: &[(&str, &str, &str, EvalFn)] = &[$(($ty, $module, $name, $f),)*];
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
    core_linear_pattern => "linearPattern", "linearPattern.fs", "linearPattern";
    core_circular_pattern => "circularPattern", "circularPattern.fs", "circularPattern";
    core_mirror => "mirror", "mirror.fs", "mirror";
    core_boolean => "booleanBodies", "boolean.fs", "booleanBodies";
    core_plane => "cPlane", "cplane.fs", "cPlane";
    core_draft => "draft", "draft.fs", "draft";
    core_split => "splitPart", "splitpart.fs", "splitPart";
}

macro_rules! sheet_metal_aware {
    ($($f:ident => $ty:literal, $native:path, $module:literal, $name:literal;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            if adds_to_active_sheet_metal(ev) { evaluate_std(ev, $module, $name) } else { $native(ev) }
        })*

        /// Core features std runs when they add to or cut an active sheet
        /// metal part (type, evaluator).
        const SHEET_METAL_AWARE: &[(&str, EvalFn)] = &[$(($ty, $f),)*];
    };
}

sheet_metal_aware! {
    sm_extrude => "extrude", crate::regen::features::extrude::eval, "extrude.fs", "extrude";
    sm_revolve => "revolve", crate::regen::features::revolve::eval, "revolve.fs", "revolve";
    sm_sweep => "sweep", crate::regen::features::solid_a::sweep, "sweep.fs", "sweep";
    sm_loft => "loft", crate::regen::features::solid_a::loft, "loft.fs", "loft";
}

/// Sheet metal runs by std in this regeneration (on, not a legacy studio, the
/// FeatureScript host registered).
pub fn std_sheet_metal() -> bool {
    STD_SHEET_METAL.load(std::sync::atomic::Ordering::Relaxed) && !LEGACY_SHEET_METAL.get() && crate::regen::custom_features::host().is_some()
}

/// The feature's boolean (Add / Remove / Intersect) reaches an active sheet
/// metal part: std's sheetMetalAwareBoolean then cuts the model's
/// definition sheet, so the cut lasts through the model's later features (a
/// native cut was rebuilt away by the next Flange), and adding to one is
/// live's error.
fn adds_to_active_sheet_metal(ev: &EvalCtx) -> bool {
    if !ev.ctx.sheet_metal.values().any(|d| d.active) || matches!(ev.enum_("operationType"), "NEW" | "") {
        return false;
    }
    crate::regen::features::extrude::scope(ev).is_some_and(|s| s.iter().any(|&i| ev.ctx.sheet_metal.get(&ev.ctx.bodies[i].id).is_some_and(|d| d.active)))
}

/// The built-in feature type std's feature `name` stands for (Transform: fTransform), else `name`.
pub fn core_type(name: &str) -> &str {
    CORE.iter().find(|c| c.2 == name).map_or(name, |c| c.0)
}

macro_rules! std_features {
    ($($f:ident => $module:literal, $name:literal;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            evaluate_std(ev, $module, $name)
        })*

        /// The std module and feature of a built-in feature type.
        pub fn std_name(type_id: &str) -> Option<(&'static str, &'static str)> {
            if let Some(&(_, module, name, _)) = CORE.iter().find(|(t, ..)| *t == type_id) {
                return Some((module, name));
            }
            Some(match type_id {
                $($name => ($module, $name),)*
                _ => return None,
            })
        }

        /// The std evaluator of a built-in feature type, if std runs it.
        pub fn evaluator(type_id: &str) -> Option<EvalFn> {
            if let Some(&(.., f)) = CORE.iter().find(|(t, ..)| *t == type_id)
                && STD_ROUTE.with_borrow(|r| r.iter().any(|t| t == type_id))
            {
                return Some(f);
            }
            if !matches!(type_id, "pcbStart" | "pcbEnd")
                && (!STD_SHEET_METAL.load(std::sync::atomic::Ordering::Relaxed) || LEGACY_SHEET_METAL.get() || crate::regen::custom_features::host().is_none())
            {
                return None;
            }
            if let Some(&(_, f)) = SHEET_METAL_AWARE.iter().find(|(t, _)| *t == type_id) {
                return Some(f);
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
