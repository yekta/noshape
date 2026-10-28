//! Part Studio analysis shared by the Mass properties panel and the agent
//! API.

use noshape_model::display::{ConnectorDisplay, StudioDisplay};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::BodyId;
use noshape_model::regen::context::Context;
use noshape_model::regen::measure::{self, MassProperties};

/// Mass properties as the panel shows them.
pub struct StudioMass {
    pub props: MassProperties,
    /// The mate connector the inertia is taken about.
    pub reference: Option<String>,
}

/// Mass properties of `bodies` (empty: every part) with the parts'
/// densities; a mass override replaces the computed mass when every part
/// in the computation has one (Onshape), and with a `reference` mate
/// connector the inertia is about its origin and axes.
pub fn studio_mass_properties(ctx: &Context, display: &StudioDisplay, ps: &PartStudio, bodies: &[BodyId], reference: Option<&ConnectorDisplay>) -> Option<StudioMass> {
    let props = &ps.part_properties;
    let all: Vec<BodyId> = if bodies.is_empty() { display.bodies.iter().map(|b| b.id.clone()).collect() } else { bodies.to_vec() };
    let overrides: Option<f64> = all.iter().map(|b| props.get(b).and_then(|p| p.mass_override)).sum();
    let mut mp = measure::mass_properties(ctx, bodies, &|b| props.get(b).and_then(|p| p.density))?;
    if let Some(m) = overrides {
        if let Some(old) = mp.mass.filter(|o| *o > 0.0) {
            mp.inertia *= m / old;
        }
        mp.mass = Some(m);
    }
    // The reference field names its connector also without a material (live ui-mass/07; MASS-MC-FIELD: the field
    // stayed empty); the inertia moves to its frame once there is a mass.
    let name = reference.map(|c| c.name.clone());
    if let (Some(c), Some(m)) = (reference, mp.mass) {
        let (o, x, z) = (c.origin, c.x, c.z);
        let y = z.cross(x);
        // Parallel axis to the connector origin, then its axes.
        let d = mp.center_of_mass - o;
        let shift = glam::DMat3::from_diagonal(glam::DVec3::splat(d.length_squared())) - glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z);
        let r = glam::DMat3::from_cols(x, y, z);
        mp.inertia = r.transpose() * (mp.inertia + shift * m) * r;
    }
    Some(StudioMass { props: mp, reference: name })
}
