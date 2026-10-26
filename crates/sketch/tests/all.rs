//! Every integration test file of this crate in one test binary (tools/tiers/merge-tests.py): one link
//! instead of one per file. Run one file's tests with `cargo test -p <crate> --test all <file>::`.

mod bench;
mod constraints_more;
mod dwg;
mod fuzz;
mod misc;
mod onshape_ux;
mod regions;
mod shapes;
mod solver_basic;
mod text_and_tools;
mod tools;
