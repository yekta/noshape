//! Demo documents with an assembly (they solve it); the rest are noshape-model-core's (re-exported here).

pub use noshape_model_core::demo::*;
use noshape_model_core::configuration::ConfigValues;
use noshape_model_core::document::{Document, TabContent};
use noshape_model_core::reference::Reference;

pub fn demo_document() -> Document {
    let mut d = plate_document();
    add_demo_assembly(&mut d);
    add_demo_feature_studio(&mut d);
    d
}

/// Fill the document's first Assembly tab with a demo assembly: two plates
/// (the second fastened on top of the first, turned 90°) and a pin in a
/// revolute mate through the center holes. Adds a "Pin" Part Studio, which
/// is configured (Length: Standard / Long; the instance uses the default).
pub fn add_demo_assembly(doc: &mut Document) {
    use crate::assembly::eval::AssemblyEvaluator;
    use crate::assembly::*;
    use crate::reference::Inference;
    let Some(plate_tab) = doc.tabs.iter().find(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id) else { return };
    let mut pin_ps = pin_studio();
    configure_pin_length(&mut pin_ps);
    let pin_tab = doc.add_tab("Pin", TabContent::PartStudio(pin_ps));
    let mut ev = AssemblyEvaluator::new();
    let cfg = ConfigValues::default();
    let (Some(plate), Some(pin)) = (ev.studio(doc, plate_tab, &cfg), ev.studio(doc, pin_tab, &cfg)) else { return };
    let (Some(pb), Some(qb)) = (plate.context.bodies.first(), pin.context.bodies.first()) else { return };
    // Planar cap faces of the extrusion (not fillet blends whose names mention the caps).
    let face_named = |b: &crate::regen::context::Body, needle: &str| {
        b.faces
            .iter()
            .find(|f| match &***f {
                crate::reference::TopoName::Face { role: crate::reference::Role::CapEnd(_), .. } => needle == "CapEnd",
                crate::reference::TopoName::Face { role: crate::reference::Role::CapStart(_), .. } => needle == "CapStart",
                _ => false,
            })
            .map(|f| Reference::Face((**f).clone()))
    };
    let cylinder = |b: &crate::regen::context::Body, radius: f64| {
        b.faces.iter().enumerate().find_map(|(i, f)| {
            let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?;
            match face.face_surface().ok()? {
                noshape_kernel::Surface::Cylinder { radius: r, .. } if (r - radius).abs() < 1e-6 => Some(Reference::Face((**f).clone())),
                _ => None,
            }
        })
    };
    let (Some(top), Some(bottom), Some(hole), Some(pin_face)) = (face_named(pb, "CapEnd"), face_named(pb, "CapStart"), cylinder(pb, 0.012), cylinder(qb, 0.012)) else { return };
    let mc = |instance: u64, on: Reference, inference: Inference| MateConnectorRef::on(InstanceId(instance), vec![], Reference::Implicit { on: Box::new(on), inference });
    let inst = |id: u64, name: &str, studio, body: crate::reference::BodyId, at: [f64; 3], fixed| Instance {
        id: InstanceId(id),
        name: name.into(),
        source: InstanceSource::Part { studio, body, config: ConfigValues::default() },
        placement: Placement { translation: at, rotation: [0.0, 0.0, 0.0, 1.0] },
        fixed,
        hidden: false,
        suppressed: false,
        lock: SubassemblyLock::Flexible,
        nested: Vec::new(),
        best_tessellation: false,
        reference_designator: None,
        unsuppressed_if: None,
    };
    let mate = |id: u64, name: &str, t: MateType, a, b| Mate::new(MateId(id), name.into(), t, a, b);
    let mut fastened = mate(4, "Fastened 1", MateType::Fastened, mc(1, top, Inference::Centroid), mc(2, bottom, Inference::Centroid));
    fastened.reorient_secondary = 1;
    let asm = Assembly {
        instances: vec![
            inst(1, "Part 1 <1>", plate_tab, pb.id.clone(), [0.0; 3], true),
            inst(2, "Part 1 <2>", plate_tab, pb.id.clone(), [0.0, 0.09, 0.0], false),
            inst(3, "Part 1 <1>", pin_tab, qb.id.clone(), [0.08, 0.0, 0.0], false),
        ],
        mates: vec![fastened, mate(5, "Revolute 1", MateType::Revolute, mc(1, hole, Inference::MidAxis), mc(3, pin_face, Inference::MidAxis))],
        ..Default::default()
    };
    let mut asm = asm;
    asm.instances[2].name = "Pin <1>".into();
    // Store solved positions so the document opens assembled.
    let res = ev.evaluate(doc, &asm, &eval::SolveOptions::default(), None);
    for (id, p) in &res.solved.placements {
        if let Some(i) = asm.instance_mut(*id) {
            i.placement = *p;
        }
    }
    if let Some(t) = doc.tabs.iter_mut().find(|t| matches!(t.content, TabContent::Assembly(_))) {
        t.content = TabContent::Assembly(Box::new(asm));
    }
}
