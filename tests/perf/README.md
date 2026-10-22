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

Both are perf.yml `fixture`s (no baseline). Display meshes are chord based by default (Onshape's angle-based mesh,
live 459, is gated off until its GPU cost is down: `regen::angle_display_mesh`). To compare both in one run:
`variants` = `angle:NOSHAPE_DISPLAY_MESH=angle`.
