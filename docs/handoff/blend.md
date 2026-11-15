# Blends (agent "blend"): current state

Updated 11-15 20:16 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Landing (queued 20:15, 53737869, 5 commits on main): chamferair, balljump, chordnear (kernel
  `Shape::project_point_near`: OCCT's local extremum kept only inside the face with the offset along the normal),
  ballnear, unifycand (unify candidates once on an unchanged shape; B-spline poles off one plane skip OCCT's planarity
  sampling). Gates: pairs clean (882 = 882), asm of the whole stack identical on all 24 (asm-chord-new.log), g++-13
  ok. Mac release: knife 46/52 both, regen 1576 -> 1192 ms (Chamfer 1 167 -> 121, core's lean-handle Fillet 8 541 ->
  175). After it lands: tell core (Fillet 8); delete the blend4 / blend6 targets on vibe6.
- infento 1dc32b Chamfer 3 (dx/core): live 3169 answered (live2): Distance 1 goes on the SIDE face at both edges,
  merged mirror or not. Branch `blend-ch3` (on main; checked out in blend10): a2074e738's geometry (rim_chamfer_cut by
  shared face, `prism_rim_chamfer_on`, a lone circle's corners mode; 87,057.03 mm3 with live's sides, live 87,057.04)
  + face_order_in ranking a Copy face as the face it copies + T1 features_c::two_offsets_on_a_mirrored_body_go_as_on_
  its_source_as_live (passes, but also without the ranking change: the rectangle doesn't hit the case). Running the
  studio with a ZZ_D1 print of each edge's face keys (Mac, sg/st1dc) to see why the mirrored arc ranks its cap first;
  then the fix, a T1 that fails before it, a pair on vibe5 (slot). Then tell dx and core; recheck Fillet 3 (core's
  dump vibe5 ~/agents/core-keep/fillet3-1dc/fillet-58).

## Open questions / waiting

- The user's dock, tab "B - Cakil Tasi" (qa P0 speed; quiet vibe5: regen 1257 ms, fillets ~590): Rib Dip R 1 168 ms,
  Rib Dip R Dizi 135, Rib Round Dizi 127 each build OCCT's as-is fillet (121 / 59 / 40 ms), which passes the checks;
  then my DOCK-FILLET rule (far-off corners whose loose vertices overlap: features.cpp scaled_blend) throws it away and
  builds at the working scale (34 / 63 / 41): ~220 ms thrown away. A parallel scaled build was considered and dropped
  (crash guards, budget jumps and blend flags are thread-local). Next: sample the as-is build (why 3.5x the scaled
  one) on a mode-700 copy (vibe5 ~/lead-private/dock.noshape; copies in ~/tmp/blend, deleted after).
- isaac-ball studio 3c5c58 Fillet 1 (asm; 42/112/68 vs live 41/106/63; 860 ms vs 16): 14 edges R 4.5. OCCT's as-is and
  scaled builds fail; by_groups (579 ms) + past_end_faces build it. The input matches live face for face. Face diff
  (CLI query_entities after a rollback vs live.jsonl boxes): two vertical rounds (x -413 / -370) come in two faces each,
  split at z 30.68 (live one each, 129.69 / 127.18 mm2), and Extrude 3's plane stays one 834.32 mm2 face where live's
  round across it leaves 618.32 + 216.00. Next: why OCCT fails on the 14 edges together (dump vibe7
  ~/tmp/blend/m3cd/fillet-5.brep/.txt; repro ~/agents/asm-keep/m3c with asm-keep/bin/ops-s30, which reads its
  context.jsonl: my Mac build can't yet).
- Boolean sections (bool's): the knife's remaining Chamfer 1 58 ms / Fillet 2 17 ms are OCCT on pairs that touch
  within the knife's ~21 um tolerances (a disjointness test proves 4 of 96; parked hulls-wip.patch). bool's plane x
  B-spline marcher (71efdd87e) withdrawn (knife Mirror 1 regression).
- cf-snap-hook (dx): Mac release Fillet 6 26 ms (live 8), Fillet 3 14 (7), the document 243 vs live 274; dx's vibe7
  numbers were ~5x. Fillet 6's direct build 15 ms (sections 5.5: cone x torus, axes parallel 50-230 mm apart). Low.

## Next, in order

1. Chamfer 3 (above), then land it.
2. The dock's thrown-away as-is builds (above).
3. isaac-ball 3c5c58 Fillet 1 (above).
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
