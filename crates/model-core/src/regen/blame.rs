//! A failed blend's blame, on request: which of its picks fail alone. Never during regeneration (each pick reruns the
//! feature: c2-impeller-loft's failed Fillet took 52 s, not 1.1); the dialog asks for it off the UI thread.

use super::{RegenOptions, Regenerator};
use crate::configuration::ConfigValues;
use crate::display::FeatureStatus;
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::{FeatureId, Reference};
use noshape_units::UnitPrefs;
use std::time::{Duration, Instant};

/// At most this many picks are tried one by one; a failed blend with more blames none.
pub const MAX_BLAME_PICKS: usize = 24;

/// The picks of a failed Fillet or Chamfer that fail on their own: live blames those (errors-1123: one edge too big,
/// its entry red) and none when only the set fails (mistakes-1125: a block's 12 edges at R12, each alone builds).
/// `reg` should hold the last run's results (a snapshot), so only the blend reruns, once per pick. None when the
/// feature isn't a failed blend of up to [`MAX_BLAME_PICKS`] picks, or `cap` ran out first.
pub fn failing_alone(reg: &mut Regenerator, studio: &PartStudio, config: &ConfigValues, units: &UnitPrefs, opts: &RegenOptions, feature: FeatureId, cap: Duration) -> Option<Vec<Reference>> {
    let start = Instant::now();
    let at = studio.features.iter().position(|f| f.id == feature)?;
    let f = &studio.features[at];
    if !matches!(f.type_id.as_str(), "fillet" | "chamfer") {
        return None;
    }
    let picks: Vec<Reference> = f.params.query("entities").iter().filter(|r| f.type_id != "fillet" || matches!(r, Reference::Edge(_))).cloned().collect();
    if picks.len() <= 1 || picks.len() > MAX_BLAME_PICKS {
        return (picks.len() <= 1).then_some(picks);
    }
    let opts = RegenOptions { upto: Some(at + 1), ..opts.clone() };
    let mut out = Vec::new();
    for r in picks {
        if start.elapsed() > cap {
            return None;
        }
        let mut ps = studio.clone();
        ps.features[at].params.set("entities", ParamValue::Query(vec![r.clone()]));
        let res = reg.regenerate(&ps, config, units, &opts);
        let failed = res.display.features.iter().find(|s| s.id == feature).is_some_and(|s| s.status == FeatureStatus::Error);
        if failed {
            out.push(r);
        }
    }
    Some(out)
}
