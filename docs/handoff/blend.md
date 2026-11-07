# Blends (agent "blend")

Area (lead 11-07): fillets, chamfers and face blends: noshape's own blends (crates/brep/src/blend*, rust_blend.cpp,
ticket BLEND-RUST), OCCT's blend route and its patches, and the blend features in crates/model-core
(regen/features/detail.rs `fillet` / `chamfer` / `variable_fillet`, blend_section.rs, faceBlend). core keeps the rest
and reviews blend changes on request. History before 11-07: docs/handoff/core.md ("noshape's own blends",
"Mixed-radius corners", the 11-07 states).
Worktree /root/projects/noshape-wt/blend, builds on vibe5 (`NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh ...`), scratch
/root/blend-scratch. Any change in crates/brep passes bool's gate first (handoff/bool.md: every corpus and parity56 at
0 differences from OCCT).

Parity runs: `NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh env NOSHAPE_PARITY_MODELS=/home/ngyekta/parity/models
NOSHAPE_PARITY_ONLY=<docs> cargo test --release -p noshape-ops --test all parity::parity_models -- --ignored --nocapture`.

## List (lead 11-07 18:00)
1. cf-surface-pattern's Fillet 1 (side rounds against the slanted top extended).
2. c2-tpd-fillet-variable's variable Fillet 1: exact from its radius law, and fast (1.7 s, live 44 ms).
3. RING-BLEND-SPEED: c2-rings-loft's Fillet 1 370-410 ms vs live 23 ms (OCCT's cut of the two rounds).
4. Face blend tangent hold lines, once live answers REQUESTS core 17:27.
5. Blend differences in the parity suites / SCORE.md, most features first (cf-snap-hook's Fillet 4 is skasm's).

## State

**11-07 18:50 UTC: cf-surface-pattern's Fillet 1 as live (34 -> 36 of 39).**
- Live 2227: the round is the ellipse's alone (JtS top x nose cylinder JnW); a picked face's edges aren't propagated
  (live 2202: "with a face picked, the result doesn't depend on propagation"). Past each end of the ellipse the round
  runs straight on between the side plane (the cylinder's tangent plane there) and the top's plane (cylinders R 2.54,
  JuW / Jua) until the sweep below trims it, 1.617 mm up the side edge.
- `blend_section::run_on_rim_blend`: for a selection that stops where its plane x cylinder arcs run on tangentially
  into edges left out: a wedge (the slab under the plane with the arc and its two end tangent planes), its three top
  edges rounded by OCCT, the wedge minus its round cut from the body. G7's rule ("Adjoining edge not filleted." for a
  curved edge left open) now only where the tangent neighbour shares one of the edge's faces (G7's arc and lines share
  the top; Part 5's ellipse and side edges share none).
- 10/22/14, live's. T1 model features_c::a_rounded_arc_runs_straight_on_past_its_ends_as_live (Part 5 as imported,
  data/surface-pattern-part5.brep: face picked with propagation, and the edge alone without).
- The document's time isn't the fillet's (44 ms, live 11): Surface Pattern 3 7.7 s (live 537 ms), the next difference
  (11 parts, live 6): custom feature, not a blend.
