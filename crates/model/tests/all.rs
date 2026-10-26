//! Every integration test file of this crate in one test binary (tools/tiers/merge-tests.py): one link
//! instead of one per file. Run one file's tests with `cargo test -p <crate> --test all <file>::`.

mod big_plate;
mod bracket_perf;
mod config;
mod cplane;
mod custom_features;
mod display_tessellation;
mod drag_speed;
mod engine;
mod features_a;
mod features_b;
mod features_c;
mod kernel_robustness;
mod native_assembly;
mod open_undo_fins;
mod oracle_asm;
mod oracle_p1;
mod p2_oracle;
mod pattern_fins;
mod pattern_perf;
mod pattern_second;
mod perf;
mod qa_regressions;
mod regen;
mod seam_display;
mod shared_meshes;
mod sheetmetal_frames;
mod sheetmetal_std;
mod sketch_assoc;
mod sketch_fillet_regen;
mod std_oracle;
