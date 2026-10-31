//! Every integration test file of this crate in one test binary (tools/tiers/merge-tests.py): one link
//! instead of one per file. Run one file's tests with `cargo test -p <crate> --test all <file>::`.

mod assembly_perf;
mod big_plate_perf;
mod coverage;
mod crop_fixtures;
mod derived_mirror;
mod drawing_drawn_by;
mod dx_perf;
mod e2e;
mod hidden_edges;
mod install;
mod live;
mod many_parts;
mod mate_flip;
mod mesh_pixels;
mod regen_profile;
mod smoke;
mod symmetric_mirror;
