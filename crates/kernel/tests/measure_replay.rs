//! Measure's minimum distance between two parts (`part_distance`) against BRepExtrema's whole-shape one (`distance`)
//! on a corpus of part pairs (NOSHAPE_MEASURE_CORPUS: BREP compounds of two parts, e.g. the parity suite's with
//! NOSHAPE_MEASURE_CORPUS set: each Part Studio's nearest, middle and farthest pair): the same distance within 1e-7 m,
//! each point on its part, the points that far apart; both times. NOSHAPE_MEASURE_OLD_MAX=<n>: pairs whose faces'
//! product passes n are measured the new way alone (BRepExtrema's whole-shape distance takes hours on a plate with
//! 1735 holes), their points checked.

use noshape_kernel::*;

#[test]
#[ignore = "T2: the measure corpus (NOSHAPE_MEASURE_CORPUS)"]
fn measure_corpus_replay() {
    let Some(dir) = std::env::var_os("NOSHAPE_MEASURE_CORPUS") else { return };
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "brep")).collect();
    names.sort();
    let (mut bad, mut ms_old, mut ms_new) = (Vec::new(), 0.0, 0.0);
    for path in &names {
        let Ok(pair) = Shape::from_brep_bytes(&std::fs::read(path).unwrap()) else { continue };
        let parts = pair.children();
        let [a, b] = parts.as_slice() else { continue };
        let big = std::env::var("NOSHAPE_MEASURE_OLD_MAX").ok().and_then(|v| v.parse::<usize>().ok()).is_some_and(|m| a.faces().len() * b.faces().len() > m);
        let t = std::time::Instant::now();
        let old = if big { part_distance(a, b) } else { distance(a, b) };
        let t_old = if big { f64::NAN } else { t.elapsed().as_secs_f64() * 1e3 };
        let t = std::time::Instant::now();
        let new = part_distance(a, b);
        let t_new = t.elapsed().as_secs_f64() * 1e3;
        let name = path.file_stem().unwrap().to_string_lossy();
        let (old, new) = match (old, new) {
            (Ok(o), Ok(n)) => (o, n),
            // (Both refused (a face without a surface): the same answer.)
            (Err(_), Err(_)) => {
                println!("{name}: both failed");
                continue;
            }
            (o, n) => {
                bad.push(format!("{name}: BRepExtrema {:?}, ours {:?}", o.map(|d| d.distance).map_err(|e| e.to_string()), n.map(|d| d.distance).map_err(|e| e.to_string())));
                continue;
            }
        };
        if !big {
            ms_old += t_old;
            ms_new += t_new;
        }
        let on = |p: glam::DVec3, s: &Shape| make_vertex(p).and_then(|v| distance(&v, s)).map_or(f64::INFINITY, |d| d.distance);
        let (da, db) = (on(new.point_a, a), on(new.point_b, b));
        let apart = (new.point_b - new.point_a).length();
        println!("{name}: {:.9} mm (BRepExtrema {}), {t_new:.1} ms (BRepExtrema {t_old:.1} ms)", new.distance * 1e3, if big { "not run".to_string() } else { format!("{:.9}", old.distance * 1e3) });
        if (new.distance - old.distance).abs() > 1e-7 || da > 1e-7 || db > 1e-7 || (apart - new.distance).abs() > 1e-7 {
            bad.push(format!("{name}: {} m against {} m, points {da:.1e} / {db:.1e} off their parts, {apart} apart", new.distance, old.distance));
        }
    }
    println!("{} pairs: part_distance {ms_new:.0} ms, BRepExtrema {ms_old:.0} ms", names.len());
    assert!(bad.is_empty(), "{} of {} differ:\n{}", bad.len(), names.len(), bad.join("\n"));
}
