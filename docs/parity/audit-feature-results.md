# Feature results against live: numerical audit (core, from 11-01)

Every feature type and its main options: volume, area and face / edge counts against live's (Onshape). Live's figures
come from the fs oracle (`/root/onshape-research/live/fs-oracle-p1/CASES.md`, results-N.jsonl, its [min, max] bands)
and behaviours.md requests; ours from the T1 / T2 tests named. Status: ✅ matches (within live's band or 1e-6) ·
≈ differs by less than live's own spread · ❌ differs (in the list below) · ⏳ live's figures asked.

## Batches 11-13 against live (core 11-02 16:00, live 1739; oracle_p1::audit_feature_results checks them)

Match: MF1-4, RF1, DF1-2, OS1-2, TH1-2, EN1, FI1, BD1, RB1-2, SW5, RV5, SH6-7, DR5, CH7; BD2 (closed Boundary profiles
refused, live's message) and WR1 (Wrap Add one part, 8 faces) fixed 11-02. Differ: FI2 (live builds a G1 Fill with a
warning, ours fails) [each edge now takes its own face among the row's adjacent faces (it took the first for all); OCCT's plate still throws: G1 to two perpendicular walls at each corner can't be met, live warns and builds a dome], SW6 (5e-5 in volume), LF5
(2.3e-5), LF4 (1.4e-5: Normal to profile ends, as G32's interior columns), FL7 now matches (live 1796: Width
control takes one unbranched chain per part). DR5b and HX1 / HX2 match (live 1797: the oracle's 378.420 mm helix length included the base circle). (SW6's audit also lists our helix wire body, which
live's figures leave out.)

## Coverage

| Feature | Cases | Status | Where |
|---|---|---|---|
| Extrude (blind, through, up to next / face / vertex / part, symmetric, second direction, draft, thin, offset start, scope) | 16 | ✅ | oracle_p1 batch 1 (E1–E15) |
| Revolve (full, angle, symmetric, second direction, thin) | 7 | ✅ (RV5 ✅ 11-02) | oracle_p1 batch 2; batch 13 |
| Fillet (edge, chain, variable, conic / curvature, overflow, full round) | 9 + G-cases | ✅ (mixed-radius ≈, below) | oracle_p1 batch 3; kernel blend::*; live 1686 / G24 / G25 / kern f2 |
| Chamfer (equal, two distances, distance-angle) | 5 | ✅ (CH7 ✅ 11-02) | oracle_p1 batch 4 |
| Hole (simple, counterbore, countersink, tapped, clearance, blind / through / up to next) | 8 | ✅ | oracle_p1 batch 5 |
| Shell, Draft | 4 + 3 | ✅ but G46 (below); SH6 / SH7 / DR5 / DR5b ✅ 11-02 | oracle_p1 batch 6; kernel_robustness |
| Sweep, Loft | 4 + 3 | ✅ (SW5 ✅; SW6 5e-5, LF4 1.4e-5, LF5 2.3e-5 ≈, 11-02) | oracle_p1 batch 7 |
| Thin loft (Trim ends, two-sided, three profiles, flipped) | 7 | ≈ (below) | engine::thin_loft_* (live Loft 8, 1728) |
| Linear / circular / curve pattern, Mirror | 5 + 2 + 1 + 3 | ✅ | oracle_p1 batch 8 |
| Boolean, Split, Transform | 5 + 4 + 5 | ✅ | oracle_p1 batch 9 |
| Plane | 7 | ✅ | oracle_p1 batch 10 |
| Move face, Replace face, Delete face, Offset surface, Thicken, Enclose | 12 | ✅ 11-02 (live 1739) | batch 11 |
| Fill, Boundary, Rib, Wrap, Helix | 9 | ✅ 11-02 but FI2 ❌ (live 1739; BD2 / WR1 fixed, Helix on a sketch circle fixed) | batch 12 |
| Custom feature sweeps along a helix (knurl) | 14 pairs | ≈ (below) | custom_features::knurl_* |
| Mass properties › calculation variance | 6 | ✅ | measure::tests::mass_property_variance_is_lives (live 1731) |
| Sheet metal results | — | skasm's area (docs/handoff/skasm.md) | — |

Counts (11-01 21:30): oracle_p1 91 cases, 91 as live; kernel_robustness's live cases 33 of 34 (G46); batches 11-13
30 cases built (29 OK, FI2 declines), live's figures asked; 1 bug found and fixed (Helix on a sketch circle).

## Batches 11-13: ours, waiting for live's

`oracle_p1::audit_feature_results` (T2, `--ignored --nocapture`) builds them and prints each case's solids (volume mm³,
area mm², faces, edges, count), sheets (area, faces, count) and wire length; live's go into its AUDIT_LIVE as they
come. Hand checks: MF1 / MF2 / RF1 exact boxes; MF3 the wedge 20 × 20 tan 10° × 40; OS2 2π·13·20; HX1 / HX2
5·√((20π)² + 6²); SW5 π·2²·(30 + 5π); SW6 24 × 2π·20 × 3 (a screw sweep of a profile in a plane through the axis:
Pappus); RV5 48000 + π·40²/4·60; EN1 30 × 20 × 20.

| Case | Status | Solids: volume / area / faces / edges (parts) | Sheets: area (faces) | Wire |
|---|---|---|---|---|
| MF1 offset top 5 | OK | 60000 / 9800 / 6 / 12 | | |
| MF2 translate right 5 | OK | 52000 / 9400 / 6 / 12 | | |
| MF3 rotate right 10° | OK | 46589.3842 / 8600.7489 / 6 / 12 | | |
| MF4 hole wall offset 2 | OK | 47434.5133 / 9120.4425 / 7 / 15 (the hole shrinks to Ø6) | | |
| RF1 top → plane z 25 | OK | 60000 / 9800 / 6 / 12 | | |
| DF1 delete hole wall | OK | 48000 / 8800 / 6 / 12 | | |
| DF2 delete fillet face | OK | 48000 / 8800 / 6 / 12 | | |
| OS1 offset top 5 | OK | (BLOCK) | 2400 (1) | |
| OS2 offset CYL side 3 | OK | (CYL) | 1633.6282 (1) | |
| TH1 thicken OS1 2 | OK | 52800 / 14000 / 12 / 24 (2) | | |
| TH2 thicken OS1 mid-plane 4 | OK | 57600 / 14400 / 12 / 24 (2) | | |
| EN1 enclose | OK | 12000 / 3200 / 6 / 12 | | |
| FI1 fill, Position | OK | (shell) | 2400 (1) | |
| FI2 fill, Tangency to the sides | ERROR "Cannot create fill surface with selected boundary conditions. Continuity requirement might be too high." | | | |
| BD1 boundary line / arc | OK | | 896.8500 (1) | |
| BD2 boundary square / Ø16 | OK | | 1298.4862 (4) | |
| RB1 rib, parallel | OK | 12608 / 12976 / 15 / 36 | | |
| RB2 rib, normal | OK | 13728 / 14056 / 15 / 36 | | |
| WR1 wrap emboss 1 (New) | OK | 6335.6854 / 2020.9557 / 9 / 15 (2) | | |
| HX1 / HX2 | OK | | | 315.5884 |
| SW5 | OK | 574.3832 / 599.5159 / 4 / 5 | | |
| SW6 rectangle along a helix | OK | 9047.7834 / 7602.2293 / 6 / 12 | | 378.1829 |
| LF5 square / Ø16 / square | OK | 10888.0469 / 3260.8985 / 6 / 12 | | |
| RV5 top face 90° about its front edge | OK | 123398.2237 / 19883.1853 / 11 / 21 (2) | | |
| SH6 | OK | 11421.8583 / 11804.6902 / 19 / 48 | | |
| SH7 | OK | 11147.4191 / 11543.0009 / 19 / 40 | | |
| DR5 | OK | 50125.6080 / 9103.4825 / 6 / 12 | | |
| CH7 | OK | 47722.8719 / 8689.3265 / 7 / 15 | | |
| FL7 conic ρ 0.5 width 6, 12 edges | OK | 46659.2778 / 8039.0526 / 18 / 48 | | |

Found while writing them: **Helix › Circle failed on a sketch circle** ("Select a circle or arc."; the field takes
sketch edges, as live's): fixed, T1 features_b::helix_on_a_sketch_circle.

## Mismatches, biggest first

| # | Case | Ours | Live | Δ | Cause | State |
|---|---|---|---|---|---|---|
| 1 | Knurl 45/45 (whole part, 148 grooves + mirror) | | | −3.45 mm³ (−3.6e-5) | live's helix sweep: one groove tool exact = area × height 44.730217, ours 44.730284, live 44.733402 (+7.1e-5); 80/30 live −1.4e-5 | not ours: Parasolid's approximation (bool.md 11-01 20:00) |
| 2 | G46 shell 1 mm after fillet + chamfer | 6053.883 mm³ | 6052.697 | +1.19 | live's inner corner patch isn't the exact offset (ours mirror-symmetric, live's not) | kept: ours exact (handoff "From skasm (10-31 05:40)") |
| 3 | Thin loft (d) square → Ø16 → square, Trim ends off | 2584.187 | 2584.533 | −0.346 | faces match live's; live's volume integral suspected (below) | live's .x_t asked |
| 4 | Thin loft (c) Thickness 2 = 0.5 | 1999.930 | 2000.250 | −0.32 | (c)'s faces match live's to 1e-4; live's evVolume moves 0.73 with its accuracy | ≈ |
| 5 | Thin loft flipped (d) | 2334.600 | 2334.863 | −0.263 | as #3 | open |
| 6 | Mixed-radius corner (a), R4 top over R2 verticals | 8548.6645 | 8548.5639 | +0.10 | live's procedural corner (its own −0.04…+0.10 off the exact face) | ≈ |
| 7 | Thin loft (b) | 1368.493 | 1368.571 | −0.078 | as #3 | open |
| 8 | Thin loft flipped (b) | 1228.098 | 1228.170 | −0.072 | as #3 | open |
| 9 | Thin loft (a) | 1370.084 | 1370.105 | −0.021 | as #3 | open |
| 10 | Mixed-radius drafted (c), one call | 44304.915 | 44304.934 | −0.019 | OCCT's 4-sided corner patch | ≈ |

Thin loft (#3, #5, #7-#9), 11-01 21:40: face by face against live 1728's areas (engine
`thin_loft_flipped_inside_is_the_inner_offset_as_live` with NOSHAPE_FACES=1): (b-in) outer 324.62155 / live
324.62147, inner 289.45442 / 289.45426, caps 19.01936 / 19.02057 and 11.79670 / 11.79565 (the total area equal to
1e-5 mm²); (d-in) outer 615.22464 / 615.22452, inner 551.91000 / 551.91313, caps 19.04223 / 19.04461. Ours
integrates tight (mass_property_bounds 2e-5 / 1e-3 mm³). Ruled out as the cause: the offset fit (rows twice as
dense: 10× closer, volume +3e-4), the caps' ruling (by the loft's parameter as Parasolid's: +0.001). The gaps
(6e-5 to 1.3e-4 of the volume) are the knurl gap's size, which was Parasolid's; live asked (21:40) for the four
bodies as .x_t, to integrate here (xt_read reads Parasolid offset surfaces).
