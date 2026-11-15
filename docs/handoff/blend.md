# Blends (agent "blend"): current state

Updated 11-15 22:58 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Landed b106b424 (20:33): chamferair, balljump, chordnear, ballnear, unifycand. Core told; vibe6 gate targets gone.
- Dock tab B scale-first (lead 22:25: try it; land only if identical end to end: every suite document's topology and
  status, hats / dock, qa's 47 tickets, corner-break's 42 faces, all 24 assemblies; else keep the double build and
  park the diff). Branch `blend-scalefirst` 18b585d5b (on main; blend6 has it): op_fillet's `short_step_beside` (an
  edge left out, under r / 4 long, at a rounded edge's vertex) skips the as-is build in scaled_blend (identical where
  the as-is round wouldn't have been kept; not at working scale 1). ZZ_SF_LOG=1 keeps the old flow and prints
  "zz sf <what> predicted P kept K" per fillet: running the model suite so on vibe6 (unit blend-sf-log6, log
  sf-log.log, flag sf-log.done). Any "predicted 1 kept 1" = a result change: narrow the predictor or park. Then the
  dock in log mode (CLI from blend6 on vibe6, a mode-700 copy, deleted after), then the full check above.
  Dock numbers: Mac release, tab B 518 ms; Rib Dip R 1 as-is 36 + scaled 23 ms, Rib Dip R Dizi 35 + 35; quiet vibe5:
  ~220 of 1257 ms thrown away.

## Open questions / waiting

- infento 1dc32b Chamfer 3: live 3169 says Distance 1 on the side at both arcs. Our class rule (G30: first side,
  start cap, end cap, other sides) puts it on the start cap: the R65 arc's side isn't its loop's first side by any
  entity order we have (ours: entity 28 in a loop with 10, 11, 17, 24, 31, 33; live's sketch order: the arc 11th, the
  loop has the rectangle's left/right sides 3 / 4 and lines 7-10 before it). One arc's side is a Mirror copy, the
  other's cap is. Asked live (REQUESTS 22:43) for qAdjacent / qOwnedByBody / qCreatedBy face orders at bar 100.
  Branch `blend-ch3` (main + a2074e738 geometry + a Copy face ranked as its source + first side of the face's own loop
  + T1 two_offsets_on_a_mirrored_body... that passes either way; commit e960186b6 has ZZ_D1 debug). Not landable yet.
- isaac-ball 3c5c58 Fillet 1 (asm; 42/112/68 vs live 41/106/63): all 14 edges are lines between planes, R 4.5, and
  face 21 (Extrude 3's plane) is 9 mm tall between edges 73 + 80 (x -370) and 76 + 77 (x -413): full rounds across a
  strip of a bigger face. OCCT builds each edge and each such pair (radius shrunk to 4.4999955: two quarter cylinders
  9e-9 m apart, a sliver of face 21 left between), but not all 14 (fails; by_groups 579 ms + past_end_faces).
  merge_full_rounds only joins rounds that share an edge (a consumed whole face, live W1). Live: one half cylinder
  R 4.5 (127.18 / 129.69 mm2) and face 21 split in two (618.32 + 216.00). Needs: the sliver removed (face split, halves
  merged), and the 14-edge build. Probe: parked/zz_probe_m3c.rs (kernel test reading the dump; dump vibe6
  ~/tmp/blend/m3c-fillet.brep, edges 52 54 56 63 65 67 73 76 77 80 85 87 92 96, R 0.0045).
- Boolean sections (bool's): the knife's Chamfer 1 58 ms / Fillet 2 17 ms are OCCT on pairs that touch within the
  knife's ~21 um tolerances (a disjointness test proves 4 of 96; parked hulls-wip.patch).
- cf-snap-hook (dx): Mac release Fillet 6 26 ms (live 8), Fillet 3 14 (7); the document 243 vs live 274. Low.

## Next, in order

1. Dock scale-first (in flight).
2. Chamfer 3 when live answers.
3. isaac-ball 3c5c58 Fillet 1: the full round across a strip (above).
4. cf-spur-gear's opFillet blend 65-120 ms (dx); GearLab 90f78b opFillet "Failed to fillet selections." after core's
   opLoft fix (asm; no dump yet).
5. Knife Fillet 2 (100 ms, live 10): OCCT's split of the run-on tool by the extended walls 25 ms (the grind wall 17 of
   it; a planes-first two-step split saves ~5 ms only), met-face distances 6, extend_face_over 10, the fuse 25. Fillet
   3 (87, live 11): OCCT's try 28 ms thrown away (only 5 fillets in the suite take that path: no predicate).
6. Knife parity after core's Revolve 1: Fillet 8's spring-line split at the support's G2 knot lines (live 3093).

## Parked (with where)

- Known sections API: /root/blend-scratch/parked/knownsec-wip.patch (bool agreed to park; tangent support pairs aren't
  the cost).
- Debug patches: parked/zz-ahead-dbg.patch (per-pair occt_ahead time), knot-report-dbg.patch (round-edge crossings),
  hulls-wip.patch (pole-hull disjointness + true distances), chordfoot-wip.patch (superseded by chordnear),
  impf3-wip.patch (impeller Fillet 3: 1.8 mm-loose vertices; the conic loft builds, the cut is invalid).
- L-joint Fillet 2 (asm, no mate impact): live's corners sphere + torus + B-spline where OCCT fills one B-spline;
  needs a corner builder (data blend/q-ljoint).
- RING-BLEND-SPEED (rings-loft Fillet 1 / Chamfer 1, cut by tools on the part's faces), RABC (setback fill patch),
  the dock's "outer arc + step" subset: see archive.
- asm suite blend failures f0eedd Fillet 3/4, Chamfer 2, a3f7a1 Chamfer 3, 74d3d6 Fillet 5/10: upstream differences
  (Loft 1 core's; Import 1); f0eedd's build standalone.

## How to run

- Builds and crate tests: the Mac only, from blend10 (`NOSHAPE_RB_HOST=mac tools/tiers/rb.sh bash -c '...'`; env
  inside the quotes). One Mac slot, shared.
- Kernel C++ with g++-13 before landing: /root/blend-scratch/bin/g13.sh <worktree> <log> (vibe5).
- Pairs / asm on vibe5 (lead 19:50), through `~/bin/slot` there (my scripts call it): bin/host-pair.sh <tag> <host> <base wt>
  <new wt> (logs suite-<tag>-{base,new}.log, .cmp, .walls), bin/asm-one.sh <wt> <log> <host>. Long runs as systemd-run
  units with flags /root/blend-scratch/<tag>.done.
- Release timing on the Mac: `NOSHAPE_KERNEL_PROFILE=1 NOSHAPE_PARITY_ONLY=<doc>` parity_models --release; macOS
  `sample` via bin/sample-mac.sh (DOC=, REL=1, SECS=) and /root/skasm-scratch/bin/sample_{children,parents}.py.
- An assembly studio as a model: copy studios/<id>.json, .sketches.json, .live.jsonl, .refs.jsonl to
  <dir>/st-<id>/{features,sketches,live,refs}.json(l), NOSHAPE_PARITY_MODELS=<dir> (Mac: ~/noshape-host/agents/
  blend10/sg/st).
- Debug env: NOSHAPE_BLEND_DEBUG (sections, rolling balls), NOSHAPE_BLEND_DIRECT_DEBUG (direct build, per-section
  times), NOSHAPE_FREE_DEBUG (bool's per-pair outcomes).
- Worktrees: blend10 (Mac builds), blend4 / blend6 / blend8 (vibe6 gate copies; blend8's target deleted), blend9
  (handoff), blend-ch3-wip branch.
