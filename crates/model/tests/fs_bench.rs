//! FeatureScript-driven regenerations timed (T2 bench, `tools/tiers/perf.sh -p noshape-model --test all --
//! fs_bench::fs_regenerations --exact --ignored --nocapture --test-threads=1`): the first one of the process (the
//! standard library compiled), then each warm, best of 3.

use std::time::Instant;

#[test]
#[ignore = "T2 bench: tools/tiers/perf.sh"]
fn fs_regenerations() {
    let t = Instant::now();
    crate::custom_features::compile_and_runtime_errors_become_feature_errors();
    println!("timing fs cold start (a small custom feature, the standard library compiled): {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let cases: [(&str, fn()); 4] = [
        ("std sheet metal: qa4 S1 enclosure", crate::sheetmetal_std::qa4_s1_enclosure_as_live),
        ("custom feature: Y Ribs on a cylinder", crate::custom_features::y_ribs_on_a_cylinder_leave_no_planes),
        ("custom feature: the ops harness", crate::custom_features::ops_and_evaluations_run_in_custom_features),
        ("std features as Onshape's", crate::custom_features::std_features_match_onshape),
    ];
    for (name, f) in cases {
        f();
        let best = (0..3)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .fold(f64::INFINITY, f64::min);
        println!("timing {name}: {best:.1} ms");
    }
}
