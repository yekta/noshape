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

#[test]
fn nist_crada_assembly_zip() {
    // A flat zip like Onshape's (the .SLDASM, then its 8 part files under
    // their original names), outside the repo.
    let zip = std::path::Path::new("/root/kern-native-samples/sldasm-zip/nist_mtc_crada_assembly_rev-D.zip");
    if !zip.exists() {
        return;
    }
    let z = noshape_model::native_assembly::read_assembly_zip(zip).unwrap();
    assert_eq!(z.root, "nist_mtc_crada_assembly_rev-D");
    let units = UnitPrefs::default();
    let mut doc = Document::new();
    let (studio, assembly) = add_native_assembly(&mut doc, &z.root, &z.components, &z.files, true, &units).unwrap();
    let tab = |id| doc.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab(studio).name, "nist_mtc_crada_assembly_rev-D parts");
    assert_eq!(tab(assembly).name, "nist_mtc_crada_assembly_rev-D");
    let order = &doc.settings.tab_order;
    assert!(order.iter().position(|t| *t == assembly) < order.iter().position(|t| *t == studio), "the Assembly tab first");
    // One Import feature; the parts in the reverse of their first use (live
    // G41), named after the component's first instance; each threaded
    // fastener's thread curves first, as a wire of the same name.
    let TabContent::PartStudio(ps) = &tab(studio).content else { panic!() };
    assert_eq!(ps.features.len(), 1);
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    use noshape_model::display::BodyKind;
    let bodies: Vec<(&str, bool)> = r.display.bodies.iter().map(|b| (b.name.as_str(), b.kind == BodyKind::Wire)).collect();
    let nut = "90591A141_ZINC PLATED STEEL HEX NUT-1";
    let s118 = "91274A118_COATED ALLOY STEEL SOCKET HEAD CAP SCREW-1";
    let s112 = "91304A112_BLUE FLAT-HEAD SOCKET CAP SCREW-1";
    let s141 = "91274A141_COATED ALLOY STEEL SOCKET HEAD CAP SCREW-1";
    let (window, cover, plate, boxp) = ("nist_mtc_crada_polycarb-window-1", "nist_mtc_crada_cover_rev-B-1", "nist_mtc_crada_plate_rev-A-1", "nist_mtc_crada_box_rev-D-1");
    let expected = [(nut, true), (nut, false), (s118, true), (s118, false), (window, false), (cover, false), (s112, true), (s112, false), (plate, false), (s141, true), (s141, false), (boxp, false)];
    assert_eq!(bodies, expected);
    // 36 instances of the solids, grouped by part in that order, none fixed,
    // no mates; the BOM (Flattened) rows in first-appearance order.
    let TabContent::Assembly(a) = &tab(assembly).content else { panic!() };
    assert_eq!(a.instances.len(), 36);
    assert!(a.instances.iter().all(|i| !i.fixed && !i.hidden) && a.mates.is_empty());
    let names: Vec<&str> = a.instances.iter().map(|i| i.name.as_str()).collect();
    let mut want = Vec::new();
    for (part, n) in [(nut, 10), (s118, 14), (window, 1), (cover, 1), (s112, 4), (plate, 1), (s141, 4), (boxp, 1)] {
        want.extend((1..=n).map(|k| format!("{part} <{k}>")));
    }
    assert_eq!(names, want);
    let mut ev = noshape_model::assembly::eval::AssemblyEvaluator::new();
    let res = ev.evaluate(&doc, a, &noshape_model::assembly::eval::SolveOptions::default(), None);
    let rows: Vec<(String, usize)> = noshape_model::assembly::analysis::bom(&res).into_iter().map(|r| (r.name, r.quantity)).collect();
    let want_rows: Vec<(String, usize)> = [(nut, 10), (s118, 14), (window, 1), (cover, 1), (s112, 4), (plate, 1), (s141, 4), (boxp, 1)].iter().map(|(n, q)| (n.to_string(), *q)).collect();
    assert_eq!(rows, want_rows);
    // Live G41: the box at the origin, the cover 70 mm up.
    let at = |name: &str| a.instances.iter().find(|i| i.name == name).unwrap().placement.to_dmat4().w_axis.truncate() * 1e3;
    assert!(at(&format!("{boxp} <1>")).length() < 1e-6);
    assert!((at(&format!("{cover} <1>")) - DVec3::new(0.0, 0.0, 70.0)).length() < 1e-6, "{:?}", at(&format!("{cover} <1>")));
    // Placed as the file says: every nut sits on a long cap screw's axis.
    let centre = |o: &noshape_model::assembly::eval::PartOccurrence| o.world_bounds().map(|(lo, hi)| ((lo + hi) * 0.5, hi - lo)).unwrap();
    let of = |prefix: &str| res.occurrences.iter().filter(|o| o.body_name.starts_with(prefix) && o.kind == BodyKind::Solid).map(centre).collect::<Vec<_>>();
    let (nuts, screws) = (of("90591A141"), of("91274A118"));
    assert_eq!((nuts.len(), screws.len()), (10, 14));
    for (c, _) in &nuts {
        let on_axis = screws.iter().any(|(s, size)| {
            let long = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
            (0..3).filter(|&k| k != long).all(|k| (c[k] - s[k]).abs() < 1e-4)
        });
        assert!(on_axis, "nut at {c:?}");
    }
}

#[test]
fn nist_crada_assembly_combined() {
    let zip = std::path::Path::new("/root/kern-native-samples/sldasm-zip/nist_mtc_crada_assembly_rev-D.zip");
    if !zip.exists() {
        return;
    }
    let z = noshape_model::native_assembly::read_assembly_zip(zip).unwrap();
    let units = UnitPrefs::default();
    let mut doc = Document::new();
    // A tab already named like the assembly: Onshape's "(1)" (live G41).
    let assemblies = |d: &Document| d.tabs.iter().filter(|t| matches!(t.content, TabContent::Assembly(_))).count();
    let before = assemblies(&doc);
    let first = noshape_model::native_assembly::add_native_assembly_combined(&mut doc, &z.root, &z.components, &z.files, true, &units).unwrap();
    let second = noshape_model::native_assembly::add_native_assembly_combined(&mut doc, &z.root, &z.components, &z.files, true, &units).unwrap();
    let tab = |id| doc.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab(first).name, "nist_mtc_crada_assembly_rev-D");
    assert_eq!(tab(second).name, "nist_mtc_crada_assembly_rev-D (1)");
    assert_eq!(assemblies(&doc), before, "no Assembly");
    let TabContent::PartStudio(ps) = &tab(first).content else { panic!() };
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    use noshape_model::display::BodyKind;
    let solids: Vec<&noshape_model::regen::context::Body> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| b.as_ref()).collect();
    let wires = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Wire).count();
    // Live G41 (5): 36 solids + 32 thread wires, in the assembly's order
    // (the box first), repeats suffixed "_1", "_2", ….
    assert_eq!((solids.len(), wires), (36, 32));
    assert_eq!(solids[0].name, "nist_mtc_crada_box_rev-D-1");
    let nuts: Vec<&str> = solids.iter().map(|b| b.name.as_str()).filter(|n| n.starts_with("90591A141")).collect();
    let mut want = vec!["90591A141_ZINC PLATED STEEL HEX NUT-1".to_string()];
    want.extend((1..10).map(|k| format!("90591A141_ZINC PLATED STEEL HEX NUT-1_{k}")));
    assert_eq!(nuts, want);
    // At their assembly positions: the cover z 70..95.05 mm.
    let cover = solids.iter().find(|b| b.name == "nist_mtc_crada_cover_rev-B-1").unwrap().shape.bounding_box(true).unwrap();
    assert!((cover.min.z * 1e3 - 70.0).abs() < 1e-3 && (cover.max.z * 1e3 - 95.05).abs() < 1e-2, "{cover:?}");
}

#[test]
fn solid_edge_assembly_zip() {
    // The public ktane hull (Solid Edge): plates, walls and module holders
    // must close up into the box their placements describe.
    let zip = std::path::Path::new("/root/kern-native-samples/se-zip/SingleHull.zip");
    if !zip.exists() {
        return;
    }
    let z = noshape_model::native_assembly::read_assembly_zip(zip).unwrap();
    assert_eq!(z.root, "SingleHull");
    let names: Vec<String> = z.components.iter().map(|c| format!("{}:{}", c.file, c.number)).collect();
    // In the assembly's own order (JAsmRefAttr).
    assert_eq!(names, ["Hull_Top.par:1", "Hull_Left.par:1", "Hull_Left.par:2", "Hull_Top.par:2", "Hull_ModuleHolder_Long.par:1", "Hull_ModuleHolder_Short.par:1", "Hull_ModuleHolder_Short.par:2"]);
    // Placements from "Attachments" (live G43: the module holders at y = 5 mm).
    let long = &z.components[4];
    assert!((long.transform.w_axis.truncate() * 1e3 - DVec3::new(10.0, 5.0, -130.0)).length() < 1e-6, "{:?}", long.transform);
    let units = UnitPrefs::default();
    let mut doc = Document::new();
    let (_, assembly) = add_native_assembly(&mut doc, &z.root, &z.components, &z.files, true, &units).unwrap();
    let TabContent::Assembly(a) = &doc.tabs.iter().find(|t| t.id == assembly).unwrap().content else { panic!() };
    let mut ev = noshape_model::assembly::eval::AssemblyEvaluator::new();
    let res = ev.evaluate(&doc, a, &noshape_model::assembly::eval::SolveOptions::default(), None);
    let (lo, hi) = res.bounds().unwrap();
    // The closed hull: 420 x 75 x 280 mm, z from -270 to 10.
    assert!(((hi - lo) * 1e3 - DVec3::new(420.0, 75.0, 280.0)).length() < 0.1, "{:?}", (hi - lo) * 1e3);
    assert!((lo.z * 1e3 + 270.0).abs() < 0.1 && (hi.z * 1e3 - 10.0).abs() < 0.1);
}

/// An imported assembly's instances: (name, translation in mm).
fn instances_of(doc: &Document, assembly: noshape_model::document::TabId) -> Vec<(String, DVec3)> {
    let TabContent::Assembly(a) = &doc.tabs.iter().find(|t| t.id == assembly).unwrap().content else { panic!() };
    a.instances.iter().map(|i| (i.name.clone(), i.placement.to_dmat4().w_axis.truncate() * 1e3)).collect()
}

#[test]
fn inventor_assembly_zips() {
    use noshape_model::native_assembly::{NativeSystem, NativeTabs, add_native_zip, read_assembly_zip};
    // MIT-licensed Autodesk sets (iam-sets/LICENSES.md); Onshape's imports
    // of the same zips: live G42.
    let dir = std::path::Path::new("/root/kern-native-samples/iam-sets");
    if !dir.exists() {
        return;
    }
    let units = UnitPrefs::default();
    let tab = |doc: &Document, id| doc.tabs.iter().find(|t| t.id == id).unwrap().name.clone();
    // One .iam: any zip name. Assembly "<root>.iam", Part Studio "<zip> parts";
    // the occurrences in the reverse of the browser's order.
    let z = read_assembly_zip(&dir.join("dl-boxasm/BoxAssembly.zip")).unwrap();
    assert_eq!((z.system, z.root_file.as_str(), z.zip.as_str()), (NativeSystem::Inventor, "boxAssy.iam", "BoxAssembly"));
    let mut doc = Document::new();
    let NativeTabs::Assembly { studio, assembly } = add_native_zip(&mut doc, &z, true, &units).unwrap() else { panic!() };
    assert_eq!((tab(&doc, studio), tab(&doc, assembly)), ("BoxAssembly parts".to_string(), "boxAssy.iam".to_string()));
    let want = [("simpleHandle__Solid1 <1>", DVec3::new(914.4, 304.8, 228.6)), ("simpleHandle__Solid1 <2>", DVec3::new(0.0, 304.8, 228.6)), ("box__Solid1 <1>", DVec3::ZERO)];
    let got = instances_of(&doc, assembly);
    assert_eq!(got.len(), want.len());
    for ((n, t), (wn, wt)) in got.iter().zip(want) {
        assert!(n == wn && (*t - wt).length() < 1e-6, "{n} {t} vs {wn} {wt}");
    }
    // Part Studio: the parts in their first use (teeHandle.ipt unused).
    let TabContent::PartStudio(ps) = &doc.tabs.iter().find(|t| t.id == studio).unwrap().content else { panic!() };
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let parts: Vec<&str> = r.context.bodies.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(parts, ["simpleHandle__Solid1", "box__Solid1"]);
    // Wrench: browser reordered against the records (07:2 becomes <1>).
    let z = read_assembly_zip(&dir.join("dl-WrenchForm_2023/WrenchForm_2023.zip")).unwrap();
    let mut doc = Document::new();
    let NativeTabs::Assembly { assembly, .. } = add_native_zip(&mut doc, &z, true, &units).unwrap() else { panic!() };
    let want = [
        ("DS51F1496-08_Forms__Solid1 <1>", [-96.053, -21.466, -7.598]),
        ("DS51F1496-07_Forms__Solid1 <1>", [-9.475, -31.291, -0.461]),
        ("DS51F1496-07_Forms__Solid1 <2>", [-89.551, -6.99, -4.273]),
        ("DS51F1496-06_Forms__Solid1 <1>", [-0.427, -20.382, 2.473]),
        ("DS51F1496-05_Forms__Solid1 <1>", [0.266, 2.315, -8.794]),
        ("DS51F1496-04_Forms__Solid1 <1>", [49.89, 8.409, 12.945]),
        ("DS51F1496-03_Forms__Solid1 <1>", [-49.155, 1.619, 1.475]),
        ("DS51F1496-02_Forms__Solid1 <1>", [-23.74, 19.087, 7.301]),
        ("Wrench_Bar_Complete__Solid1 <1>", [-1.266, -0.405, 6.048]),
    ];
    let got = instances_of(&doc, assembly);
    assert_eq!(got.len(), want.len());
    for ((n, t), (wn, wt)) in got.iter().zip(want) {
        assert!(n == wn && (*t - DVec3::from(wt)).length() < 2e-3, "{n} {t} vs {wn} {wt:?}");
    }
    // A zip with folders, two .iam not named like the zip, no part files:
    // Onshape's errors.
    for (zip, err) in [("dl-WrenchForm_2023/WrenchForm_folder.zip", noshape_model::native_assembly::NO_ROOT), ("dl-SimpleBox2asm/SimpleBox2asm.zip", noshape_model::native_assembly::NO_ROOT), ("dl-WrenchForm_2023/WrenchOnly.zip", noshape_model::native_assembly::NO_GEOMETRY)] {
        assert_eq!(read_assembly_zip(&dir.join(zip)).err().as_deref(), Some(err), "{zip}");
    }
    // One occurrence: only a Part Studio named like the zip.
    let z = read_assembly_zip(&dir.join("dl-SimpleBox2asm/Assembly1.zip")).unwrap();
    let mut doc = Document::new();
    let NativeTabs::Studio(studio) = add_native_zip(&mut doc, &z, true, &units).unwrap() else { panic!() };
    assert_eq!(tab(&doc, studio), "Assembly1");
}

#[test]
fn solid_edge_assembly_names_and_order() {
    use noshape_model::native_assembly::{NativeTabs, add_native_zip, read_assembly_zip};
    let zip = std::path::Path::new("/root/kern-native-samples/se-zip/SingleHull.zip");
    if !zip.exists() {
        return;
    }
    // Live G43: Assembly "SingleHull.asm", Part Studio "SingleHull parts",
    // parts named like their files, the occurrences reversed.
    let z = read_assembly_zip(zip).unwrap();
    let units = UnitPrefs::default();
    let mut doc = Document::new();
    let NativeTabs::Assembly { studio, assembly } = add_native_zip(&mut doc, &z, true, &units).unwrap() else { panic!() };
    let tab = |id| doc.tabs.iter().find(|t| t.id == id).unwrap().name.clone();
    assert_eq!((tab(studio), tab(assembly)), ("SingleHull parts".to_string(), "SingleHull.asm".to_string()));
    let want = [
        ("Hull_ModuleHolder_Short.par <1>", [280.0, 5.0, -260.0]),
        ("Hull_ModuleHolder_Short.par <2>", [140.0, 5.0, -260.0]),
        ("Hull_ModuleHolder_Long.par <1>", [10.0, 5.0, -130.0]),
        ("Hull_Top.par <1>", [0.0, 75.0, -260.0]),
        ("Hull_Left.par <1>", [410.0, 0.0, 10.0]),
        ("Hull_Left.par <2>", [10.0, 0.0, -270.0]),
        ("Hull_Top.par <2>", [0.0, 0.0, 0.0]),
    ];
    let got = instances_of(&doc, assembly);
    assert_eq!(got.len(), want.len());
    for ((n, t), (wn, wt)) in got.iter().zip(want) {
        assert!(n == wn && (*t - DVec3::from(wt)).length() < 1e-6, "{n} {t} vs {wn} {wt:?}");
    }
    let TabContent::PartStudio(ps) = &doc.tabs.iter().find(|t| t.id == studio).unwrap().content else { panic!() };
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let parts: Vec<&str> = r.context.bodies.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(parts, ["Hull_ModuleHolder_Short.par", "Hull_ModuleHolder_Long.par", "Hull_Top.par", "Hull_Left.par"]);
}

/// Live G42: the Wrench's 8 parts are solids with Onshape's boxes. Ignored
/// until the ACIS reader builds `helix_spl_line` surfaces (the thread flanks
/// of parts 02 and 04 have no stored B-spline data: 02 comes back open) and
/// Inventor's hidden surface bodies are dropped (03 and 04 carry some).
#[test]
#[ignore]
fn inventor_wrench_parts_are_onshapes_solids() {
    use noshape_model::native_assembly::{NativeTabs, add_native_zip, read_assembly_zip};
    let dir = std::path::Path::new("/root/kern-native-samples/iam-sets");
    if !dir.exists() {
        return;
    }
    let units = UnitPrefs::default();
    let z = read_assembly_zip(&dir.join("dl-WrenchForm_2023/WrenchForm_2023.zip")).unwrap();
    let mut doc = Document::new();
    let NativeTabs::Assembly { studio, .. } = add_native_zip(&mut doc, &z, true, &units).unwrap() else { panic!() };
    // The parts at their own origins, Onshape's boxes (02 and 04 have helical
    // threads: ACIS helix intcurves).
    let TabContent::PartStudio(ps) = &doc.tabs.iter().find(|t| t.id == studio).unwrap().content else { panic!() };
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.context.bodies.len(), 8, "{:?}", r.context.bodies.iter().map(|b| (b.kind, b.name.clone(), b.shape.count(k::ShapeType::Face), b.shape.count(k::ShapeType::Edge))).collect::<Vec<_>>());
    for (name, lo, hi) in [("DS51F1496-02_Forms__Solid1", [-20.0, -20.0, -9.76], [20.0, 20.0, 9.76]), ("DS51F1496-04_Forms__Solid1", [-135.51, -42.0, -12.75], [39.08, 28.48, 12.75])] {
        let b = r.context.bodies.iter().find(|b| b.name == name).unwrap();
        assert!(b.shape.is_valid(), "{name}");
        let bb = b.shape.bounding_box(true).unwrap();
        assert!((bb.min * 1e3 - DVec3::from(lo)).length() < 0.02 && (bb.max * 1e3 - DVec3::from(hi)).length() < 0.02, "{name}: {:?} {:?}", bb.min * 1e3, bb.max * 1e3);
    }
}
