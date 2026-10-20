//! Native assembly import as Onshape does it (live G38/G38f). Samples
//! outside the repo; skipped when absent.

use glam::DVec3;
use noshape_kernel as k;
use noshape_model::configuration::ConfigValues;
use noshape_model::native_assembly::{PartFile, add_native_assembly};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::{Document, TabContent};
use noshape_units::UnitPrefs;

#[test]
fn shaft_with_plate_assembly() {
    let dir = std::path::Path::new("/root/kern-native-samples");
    let asm_file = dir.join("sldasm/psas_Shaft_With_Plate_Asm.SLDASM");
    if !asm_file.exists() {
        return;
    }
    let comps = k::read_native_assembly(&asm_file).unwrap();
    // The zip Onshape imported: Shaft.SLDPRT, then Canard_Shaft_Plate.SLDPRT
    // (the samples carry a "psas_" prefix).
    let files: Vec<PartFile> = ["Shaft.SLDPRT", "Canard_Shaft_Plate.SLDPRT"]
        .iter()
        .map(|f| PartFile { file: f.to_string(), configuration: None, bodies: k::read_native_parts(&dir.join(format!("sldprt/psas_{f}")), k::LengthUnit::Meter).unwrap() })
        .collect();
    let units = UnitPrefs::default();
    let mut doc = Document::new();
    let (studio, assembly) = add_native_assembly(&mut doc, "Shaft_With_Plate_Asm", &comps, &files, true, &units).unwrap();
    let tab = |id| doc.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab(studio).name, "Shaft_With_Plate_Asm parts");
    assert_eq!(tab(assembly).name, "Shaft_With_Plate_Asm");
    let order = &doc.settings.tab_order;
    assert!(order.iter().position(|t| *t == assembly) < order.iter().position(|t| *t == studio), "the Assembly tab first");
    // The parts, each at its own origin, named after the components.
    let TabContent::PartStudio(ps) = &tab(studio).content else { panic!() };
    assert_eq!(ps.features.len(), 1);
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let names: Vec<_> = r.display.bodies.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["Shaft-1", "Canard_Shaft_Plate-1"]);
    for (b, vol, rgb) in [(0, 1276.6004136609363, [203, 210, 239]), (1, 241.17251765399996, [255, 103, 249])] {
        let v = r.context.bodies[b].shape.volume().unwrap() * 1e9;
        assert!((v - vol).abs() < 1e-4, "{}: {v}", names[b]);
        let c = r.display.bodies[b].color;
        let srgb = |l: f32| ((if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }) * 255.0).round() as u8;
        assert_eq!([srgb(c[0]), srgb(c[1]), srgb(c[2])], rgb, "{}", names[b]);
    }
    let shaft_box = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((shaft_box.max.z - 0.042926).abs() < 1e-6, "{shaft_box:?}");
    // The instances where the file places them, not fixed, no mates.
    let TabContent::Assembly(a) = &tab(assembly).content else { panic!() };
    let inst: Vec<_> = a.instances.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(inst, ["Shaft-1 <1>", "Canard_Shaft_Plate-1 <1>"]);
    assert!(a.instances.iter().all(|i| !i.fixed) && a.mates.is_empty());
    let shaft = a.instances[0].placement.to_dmat4();
    assert!((shaft.w_axis.truncate() * 1e3 - DVec3::new(0.1072, -44.3876, -1.5658)).length() < 1e-4);
    assert!((shaft.transform_vector3(DVec3::Y) - DVec3::new(0.0, 0.0, -1.0)).length() < 1e-9 && (shaft.transform_vector3(DVec3::Z) - DVec3::Y).length() < 1e-9);
    let plate = a.instances[1].placement.to_dmat4();
    assert!((plate.w_axis.truncate() * 1e3 - DVec3::new(0.1072, 2.2214, -1.5658)).length() < 1e-4);
}
