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

/// noshape's own fillets and chamfers for the studio regenerating on this thread
/// ([`crate::partstudio::PartStudio::legacy_blends`]: OCCT's for studios saved before them), unless
/// NOSHAPE_BLEND_RUST says otherwise; the thread's mode before comes back when dropped (a Derived studio inside
/// another).
pub struct BlendKernel(Option<Option<bool>>);

impl BlendKernel {
    pub fn set(ours: bool) -> Self {
        if std::env::var_os("NOSHAPE_BLEND_RUST").is_some() {
            return Self(None);
        }
        Self(Some(noshape_kernel::set_rust_blend(Some(ours))))
    }
}

impl Drop for BlendKernel {
    fn drop(&mut self) {
        if let Some(before) = self.0 {
            noshape_kernel::set_rust_blend(before);
        }
    }
}

/// The kernel's topology clean-ups off for a studio saved before them ([`crate::partstudio::PartStudio::legacy_topology`])
/// while it regenerates on this thread; the setting before comes back when dropped.
pub struct LegacyTopology(bool);

impl LegacyTopology {
    pub fn set(on: bool) -> Self {
        Self(noshape_kernel::set_legacy_topology(on))
    }
}

impl Drop for LegacyTopology {
    fn drop(&mut self) {
        noshape_kernel::set_legacy_topology(self.0);
    }
}

/// The kernel's blends finished as before for a studio saved before ([`crate::partstudio::PartStudio::legacy_blend_finish`])
/// while it regenerates on this thread; the setting before comes back when dropped.
pub struct LegacyBlendFinish(bool);

impl LegacyBlendFinish {
    pub fn set(on: bool) -> Self {
        Self(noshape_kernel::set_legacy_blend_finish(on))
    }
}

impl Drop for LegacyBlendFinish {
    fn drop(&mut self) {
        noshape_kernel::set_legacy_blend_finish(self.0);
    }
}

/// The kernel's fillets leave a hole's seams on the rims they trim, as before, for a studio saved before
/// ([`crate::partstudio::PartStudio::legacy_rim_seams`]) while it regenerates on this thread.
pub struct LegacyRimSeams(bool);

impl LegacyRimSeams {
    pub fn set(on: bool) -> Self {
        Self(noshape_kernel::set_legacy_rim_seams(on))
    }
}

impl Drop for LegacyRimSeams {
    fn drop(&mut self) {
        noshape_kernel::set_legacy_rim_seams(self.0);
    }
}

/// noshape's blends leave a full round's or a ridge's two rounds to OCCT, as before, for a studio saved before
/// ([`crate::partstudio::PartStudio::legacy_blend_pairs`]) while it regenerates on this thread.
pub struct LegacyBlendPairs(bool);

impl LegacyBlendPairs {
    pub fn set(on: bool) -> Self {
        Self(noshape_kernel::set_legacy_blend_pairs(on))
    }
}

impl Drop for LegacyBlendPairs {
    fn drop(&mut self) {
        noshape_kernel::set_legacy_blend_pairs(self.0);
    }
}

thread_local! {
    static LEGACY_STUB_JOINTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a stub at a joint of collinear sketch lines keeps them two edges on this thread
/// ([`crate::partstudio::PartStudio::legacy_stub_joints`]).
pub fn legacy_stub_joints() -> bool {
    LEGACY_STUB_JOINTS.with(|c| c.get())
}

/// [`legacy_stub_joints`] as a studio saved before asks while it regenerates on this thread; the setting before comes
/// back when dropped.
pub struct LegacyStubJoints(bool);

impl LegacyStubJoints {
    pub fn set(on: bool) -> Self {
        Self(LEGACY_STUB_JOINTS.with(|c| c.replace(on)))
    }
}

impl Drop for LegacyStubJoints {
    fn drop(&mut self) {
        LEGACY_STUB_JOINTS.with(|c| c.set(self.0));
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
    ($($f:ident => $ty:literal, $native:path, $module:literal, $name:literal, $when:path;)*) => {
        $(fn $f(ev: &mut EvalCtx) -> FResult<()> {
            if $when(ev) { evaluate_std(ev, $module, $name) } else { $native(ev) }
        })*

        /// Core features std runs when they add to or cut an active sheet
        /// metal part (type, evaluator).
        const SHEET_METAL_AWARE: &[(&str, EvalFn)] = &[$(($ty, $f),)*];
    };
}

sheet_metal_aware! {
    sm_extrude => "extrude", crate::regen::features::extrude::eval, "extrude.fs", "extrude", adds_to_active_sheet_metal;
    sm_revolve => "revolve", crate::regen::features::revolve::eval, "revolve.fs", "revolve", adds_to_active_sheet_metal;
    sm_sweep => "sweep", crate::regen::features::solid_a::sweep, "sweep.fs", "sweep", adds_to_active_sheet_metal;
    sm_loft => "loft", crate::regen::features::solid_a::loft, "loft.fs", "loft", adds_to_active_sheet_metal;
    sm_linear_pattern => "linearPattern", crate::regen::features::pattern::linear_pattern, "linearPattern.fs", "linearPattern", patterns_active_sheet_metal;
    sm_circular_pattern => "circularPattern", crate::regen::features::pattern::circular_pattern, "circularPattern.fs", "circularPattern", patterns_active_sheet_metal;
    sm_curve_pattern => "curvePattern", crate::regen::features::pattern::curve_pattern, "curvePattern.fs", "curvePattern", patterns_active_sheet_metal;
    sm_mirror => "mirror", crate::regen::features::pattern::mirror, "mirror.fs", "mirror", patterns_active_sheet_metal;
    sm_fillet => "fillet", crate::regen::features::detail::fillet, "fillet.fs", "fillet", blends_active_sheet_metal;
    sm_chamfer => "chamfer", crate::regen::features::detail::chamfer, "chamfer.fs", "chamfer", blends_active_sheet_metal;
}

/// A core feature std runs only beside active sheet metal (otherwise its native evaluator, as before: reusable).
pub fn sheet_metal_aware(type_id: &str) -> bool {
    SHEET_METAL_AWARE.iter().any(|(t, _)| *t == type_id) && !STD_ROUTE.with_borrow(|r| r.iter().any(|t| t == type_id))
}

/// Sheet metal runs by std (the setting; a legacy studio still runs natively).
pub fn std_sheet_metal_on() -> bool {
    STD_SHEET_METAL.load(std::sync::atomic::Ordering::Relaxed)
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

/// A part pattern (or mirror) of an active sheet metal part, or whose Add's merge scope holds one (c2-sm-louver's
/// Linear pattern 1 of the Side): std's sheet metal pattern; its merge scope leaves active sheet metal out (an Add into
/// one is live's error), where the native pattern's "Merge with all" took every solid.
fn patterns_active_sheet_metal(ev: &EvalCtx) -> bool {
    let active = |i: usize| ev.ctx.sheet_metal.get(&ev.ctx.bodies[i].id).is_some_and(|d| d.active);
    if !ev.ctx.sheet_metal.values().any(|d| d.active) || ev.enum_("patternType") != "PART" {
        return false;
    }
    ev.resolve_bodies(ev.query("entities")).into_iter().any(active)
        || (!matches!(ev.enum_("operationType"), "NEW" | "") && crate::regen::features::extrude::scope(ev).is_some_and(|s| s.into_iter().any(active)))
}

/// A fillet or chamfer of an active sheet metal part's edges or faces: std's sheet metal aware blend (a corner break
/// of the model, "For active sheet metal parts use Corner break.": c2-sm-louver's Fillet 1).
fn blends_active_sheet_metal(ev: &EvalCtx) -> bool {
    if !ev.ctx.sheet_metal.values().any(|d| d.active) {
        return false;
    }
    let refs = ev.query("entities");
    let active = |i: usize| ev.ctx.sheet_metal.get(&ev.ctx.bodies[i].id).is_some_and(|d| d.active);
    ev.resolve_edges(refs).into_iter().chain(ev.resolve_faces(refs)).any(|(b, _)| active(b))
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

        /// The built-in feature types std runs (sheet metal, Tag, PCB).
        pub const STD_TYPES: &[&str] = &[$($name,)*];

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
