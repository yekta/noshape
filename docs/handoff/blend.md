# Blends (agent "blend"): current state

Updated 11-15 23:19 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

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

- infento 1dc32b Chamfer 3: live 3169 says Distance 1 on the cylinder at both arcs; live answered REQUESTS 22:43 (23:14,
  live/blend/q-ch3order/out.json): the arcs' faces carry Extrude 39 / Extrude 40 ids (Rm.. / Ro..), i.e. the ADD
  extrudes (each extrudes a Pedal face, RkC6 / RkC+, up to a face) merged their own side (the cylinder) and end cap
  (the plane) into Extrude 38's / Transform 12's faces. Neither qAdjacent, qOwnedByBody nor qCreatedBy order explains
  it (the two arcs, congruent under Transform 12's rotation, give opposite orders). Working idea: rank a face by the
  latest feature among its name and its merge aliases (name_unified's), then that feature's class (cylinder = side).
  Branch `blend-ch3` (+ `Bodies::aliases_of`, ZZ_D1 alias print, uncommitted in blend4): running 1dc32b on vibe6
  (log ch3-alias.log) to see the aliases. Not landable yet.
- isaac-ball 3c5c58 Fillet 1 (asm; 42/112/68 vs live 41/106/63): 14 line edges between planes, R 4.5; face 21 is
  9 mm tall between 73 + 80 and 76 + 77: full rounds across a strip of a bigger face (live: one half cylinder each,
  face 21 split 618.32 + 216.00). Cause: checked_blend's hair-smaller retry is valid with a 9e-9 m neck left in face
  21, and without_necks ran only when invalid at part scale. Branch `blend-necks` 97f669374 (blend4): always cut a
  neck where that is valid at part scale. Probe (kernel test zz_probe_m3c, dump vibe6 ~/tmp/blend/m3c-fillet.brep):
  73 + 80 now one half cylinder (127.2345 mm2, face 21 split); 76 + 77's 1e-5 retry cut is valid at working scale but
  not at part scale (a plane piece UnorientableShape / NoCurveOnSurface after the scale back; ShapeFix or cutting at
  the part's scale fixes it), so it falls to by_groups: one exact R 4.5 face (129.7389, live 129.69) but 250 ms. All
  14 together: 43 faces, the merge not reached. Next: cut at the part's own scale; then all 14; pairs.
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
