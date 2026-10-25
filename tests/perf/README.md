# Viewport performance fixtures

`nist-mtc-crada-assembly.noshape`: the NIST MTC CRADA test assembly (box, cover, plate, polycarbonate
window and fasteners; 36 parts, about 670k triangles when tessellated), imported into a Part Studio from
`nist_mtc_crada_assembly_NX.jt` of NIST's MBE PMI / Manufacturing Test Cases. NIST works are not subject
to copyright in the United States (17 U.S.C. §105) and are in the public domain; source:
https://www.nist.gov/ctl/smart-connected-systems-division/smart-connected-manufacturing-systems-group/mbe-pmi-0

Used by `.github/workflows/perf.yml` (benchmarks: `.github/perf/`).

`parts-1000.noshape`: 1000 pins (Ø4 × 10 mm) as a 40 × 25 Part pattern of new parts, one shared shape drawn with
instancing (`crates/ops/tests/many_parts.rs` `write_parts_1000_fixture`).

`big-plate.noshape`: a 400 × 400 × 10 mm plate with a 20 × 20 grid of Ø8 mm holes, one dense part
(`crates/ops/tests/big_plate_perf.rs` `write_big_plate_fixture`).

`pins-assembly-1000.noshape`: the same pin in an Assembly, 1000 instances on the same 40 × 25 grid (6 mm), the
Assembly as the first tab so the app opens on it (`crates/ops/tests/many_parts.rs` `write_pins_assembly_fixture`).

`fins-400.noshape`: the user's fin plate (qa4's v0.4.8 gate (a)): an 810 × 20 × 10 mm plate, a 1 × 20 × 5 mm wall on
top and one under it, each patterned ×400 every 2 mm with Reapply (`crates/ops/tests/many_parts.rs`
`write_fins_fixture`); `env` = `NOSHAPE_PERF_SCRIPT=delete-undo-last` times deleting Linear pattern 2 and undoing.

Both are perf.yml `fixture`s (no baseline). Display meshes are chord based by default (Onshape's angle-based mesh,
live 459, is gated off until its GPU cost is down: `regen::angle_display_mesh`). To compare both in one run:
`variants` = `angle:NOSHAPE_DISPLAY_MESH=angle`.
