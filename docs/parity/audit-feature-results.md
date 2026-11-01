# Feature results against live: numerical audit (core, from 11-01)

Every feature type and its main options: volume, area and face / edge counts against live's (Onshape). Live's figures
come from the fs oracle (`/root/onshape-research/live/fs-oracle-p1/CASES.md`, results-N.jsonl, its [min, max] bands)
and behaviours.md requests; ours from the T1 / T2 tests named. Status: ✅ matches (within live's band or 1e-6) ·
≈ differs by less than live's own spread · ❌ differs (in the list below) · ⏳ live's figures asked.

## Coverage

| Feature | Cases | Status | Where |
|---|---|---|---|
| Extrude (blind, through, up to next / face / vertex / part, symmetric, second direction, draft, thin, offset start, scope) | 16 | ✅ | oracle_p1 batch 1 (E1–E15) |
| Revolve (full, angle, symmetric, second direction, thin) | 7 | ✅ (RV5 ⏳) | oracle_p1 batch 2; batch 13 |
| Fillet (edge, chain, variable, conic / curvature, overflow, full round) | 9 + G-cases | ✅ (mixed-radius ≈, below) | oracle_p1 batch 3; kernel blend::*; live 1686 / G24 / G25 / kern f2 |
| Chamfer (equal, two distances, distance-angle) | 5 | ✅ (CH7 ⏳) | oracle_p1 batch 4 |
| Hole (simple, counterbore, countersink, tapped, clearance, blind / through / up to next) | 8 | ✅ | oracle_p1 batch 5 |
| Shell, Draft | 4 + 3 | ✅ but G46 (below); SH6 / SH7 / DR5 ⏳ | oracle_p1 batch 6; kernel_robustness |
| Sweep, Loft | 4 + 3 | ✅ (SW5 / SW6 / LF4 / LF5 ⏳) | oracle_p1 batch 7 |
| Thin loft (Trim ends, two-sided, three profiles, flipped) | 7 | ≈ (below) | engine::thin_loft_* (live Loft 8, 1728) |
| Linear / circular / curve pattern, Mirror | 5 + 2 + 1 + 3 | ✅ | oracle_p1 batch 8 |
| Boolean, Split, Transform | 5 + 4 + 5 | ✅ | oracle_p1 batch 9 |
| Plane | 7 | ✅ | oracle_p1 batch 10 |
| Move face, Replace face, Delete face, Offset surface, Thicken, Enclose | 12 | ⏳ | batch 11 (REQUESTS core 11-01 20:30) |
| Fill, Boundary, Rib, Wrap, Helix | 9 | ⏳ | batch 12 |
| Custom feature sweeps along a helix (knurl) | 14 pairs | ≈ (below) | custom_features::knurl_* |
| Mass properties › calculation variance | 6 | ✅ | measure::tests::mass_property_variance_is_lives (live 1731) |
| Sheet metal results | — | skasm's area (docs/handoff/skasm.md) | — |

Counts (11-01 20:40): oracle_p1 91 cases, 91 as live; kernel_robustness's live cases 33 of 34 (G46).

## Mismatches, biggest first

| # | Case | Ours | Live | Δ | Cause | State |
|---|---|---|---|---|---|---|
| 1 | Knurl 45/45 (whole part, 148 grooves + mirror) | | | −3.45 mm³ (−3.6e-5) | live's helix sweep: one groove tool exact = area × height 44.730217, ours 44.730284, live 44.733402 (+7.1e-5); 80/30 live −1.4e-5 | not ours: Parasolid's approximation (bool.md 11-01 20:00) |
| 2 | G46 shell 1 mm after fillet + chamfer | 6053.883 mm³ | 6052.697 | +1.19 | live's inner corner patch isn't the exact offset (ours mirror-symmetric, live's not) | kept: ours exact (handoff "From skasm (10-31 05:40)") |
| 3 | Thin loft (d) square → Ø16 → square, Trim ends off | 2584.187 | 2584.533 | −0.346 | the faces' continuation past the creases (Parasolid's lies between the polynomial and the tangent one) | open: three continuations tried |
| 4 | Thin loft (c) Thickness 2 = 0.5 | 1999.930 | 2000.250 | −0.32 | (c)'s faces match live's to 1e-4; live's evVolume moves 0.73 with its accuracy | ≈ |
| 5 | Thin loft flipped (d) | 2334.600 | 2334.863 | −0.263 | as #3 | open |
| 6 | Mixed-radius corner (a), R4 top over R2 verticals | 8548.6645 | 8548.5639 | +0.10 | live's procedural corner (its own −0.04…+0.10 off the exact face) | ≈ |
| 7 | Thin loft (b) | 1368.493 | 1368.571 | −0.078 | as #3 | open |
| 8 | Thin loft flipped (b) | 1228.098 | 1228.170 | −0.072 | as #3 | open |
| 9 | Thin loft (a) | 1370.084 | 1370.105 | −0.021 | as #3 | open |
| 10 | Mixed-radius drafted (c), one call | 44304.915 | 44304.934 | −0.019 | OCCT's 4-sided corner patch | ≈ |
