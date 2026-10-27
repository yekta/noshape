//! Every integration test file of this crate in one test binary (tools/tiers/merge-tests.py): one link
//! instead of one per file. Run one file's tests with `cargo test -p <crate> --test all <file>::`.

mod bbox;
mod blend;
mod bool_replay;
mod boolean_groups;
#[path = "core.rs"]
mod kernel_core;
mod crash_inputs;
mod display_mesh_guard;
mod draft_curve;
mod export;
mod extrude_sense;
mod features;
mod hlr;
mod hole_orientation;
mod instances;
mod mesh_import;
mod perf;
mod planar_mesh;
mod projection;
mod seam;
mod shell;
mod surfacing;
mod thread;
