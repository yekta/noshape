//! Every integration test file of this crate in one test binary (tools/tiers/merge-tests.py): one link
//! instead of one per file. Run one file's tests with `cargo test -p <crate> --test all <file>::`.

mod edge_looks;
mod pattern_bench;
mod silhouettes;
mod sketch_perf;
