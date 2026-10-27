# Viewport performance fixtures

`nist-mtc-crada-assembly.noshape`: the NIST MTC CRADA test assembly (box, cover, plate, polycarbonate
window and fasteners; 36 parts, about 670k triangles when tessellated), imported into a Part Studio from
`nist_mtc_crada_assembly_NX.jt` of NIST's MBE PMI / Manufacturing Test Cases. NIST works are not subject
to copyright in the United States (17 U.S.C. §105) and are in the public domain; source:
https://www.nist.gov/ctl/smart-connected-systems-division/smart-connected-manufacturing-systems-group/mbe-pmi-0

Used by `.github/workflows/perf.yml` (benchmarks: `.github/perf/`).

`nist-mtc-crada-assembly.step` / `.x_t`: the same assembly as Onshape exports it (STEP AP242 and Parasolid 38.1 text,
from its import of NIST's SOLIDWORKS rev-D zip; live behaviours "dx speed: drawing creation and NIST import"): the
files live timed importing (PLAN 2.7, `crates/ops/tests/dx_perf.rs` `timed_nist_import`).

`parts-1000.noshape`: 1000 pins (Ø4 × 10 mm) as a 40 × 25 Part pattern of new parts, one shared shape drawn with
instancing (`crates/ops/tests/many_parts.rs` `write_parts_1000_fixture`).

`big-plate.noshape`: a 400 × 400 × 10 mm plate with a 20 × 20 grid of Ø8 mm holes, one dense part
(`crates/ops/tests/big_plate_perf.rs` `write_big_plate_fixture`).

`pins-assembly-1000.noshape`: the same pin in an Assembly, 1000 instances on the same 40 × 25 grid (6 mm), the
Assembly as the first tab so the app opens on it (`crates/ops/tests/many_parts.rs` `write_pins_assembly_fixture`).

`fins-400.noshape`: the user's fin plate (qa4's v0.4.8 gate (a)): an 810 × 20 × 10 mm plate, a 1 × 20 × 5 mm wall on
top and one under it, each patterned ×400 every 2 mm with Reapply (`crates/ops/tests/many_parts.rs`
`write_fins_fixture`); `env` = `NOSHAPE_PERF_SCRIPT=delete-undo-last` times deleting Linear pattern 2 and undoing.

These are perf.yml `fixture`s (no baseline; the NIST assembly is the default and has one). Display meshes are
Onshape's angle-based ones (live 459; `regen::angle_display_mesh`); `NOSHAPE_DISPLAY_MESH=chord` gives the older
chord-based mesh. To compare both in one run: `variants` = `chord:NOSHAPE_DISPLAY_MESH=chord`.

`live-30-features.noshape`: live's 30-feature part (qa4 round 5 REGEN-SPEED, behaviours "qa4 round 5 v2 s1 / s2"): a
100 × 60 × 10 mm plate, 6 bosses on a plane 10 mm up, a 2-point Hole, corner fillets, boss chamfers and fillets, a
Feature pattern of Boss 6 and two edge chamfers; 60779.484 mm³, 48 faces. Built by qa4's `r5/v3s2.py` through the CLI.
Onshape's server regenerates it in 171 ms (bosses ≤ 9 ms each, Hole 18 ms); `crates/ops/tests/regen_profile.rs`
profiles it per feature in release (perf.yml `model` job).
