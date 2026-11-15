# Blends (agent "blend"): current state

Updated 11-15 19:25 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Stack of five speedups, branch `blend-chordnear` in blend10 (old base 3bfc17b16), the same five rebased on main in
  `blend-hulls` / `blend-zz-bool` up to 2f676b943:
  - chamferair 31473b9cd: a chamfer as sections runs its line a tenth past each face (knife Chamfer 1 167 -> 150).
  - balljump 278641b42: rolling balls that jump, or a section far off the others' size, decline before any loft
    (core's lean-handle Fillet 8 541 -> 231).
  - chordnear 1e92ec0e5: a chamfer's chord feet by Newton from the nearest station's foot (kernel
    `Shape::project_point_near`: OCCT's local extremum kept only inside the face with the offset along the normal,
    else the full projection; without the normal check the knife-chamfer T1 strip came 0.0476 mm, live 0.0499).
  - ballnear 3bff56db3: Support::Face feet the same way (knife Fillet 8 on the revolved ring 231 -> 206).
  - unifycand d04f5c0ae: unify's candidate search not repeated on an unchanged shape; a B-spline whose poles lie off
    one plane isn't sampled by GeomLib_IsPlanarSurface (Fillet 8 206 -> 175).
  - Measured, Mac release, main vs main + stack: knife 46/52 both, regen 1576 -> 1192 ms.
  - Gates: g++-13 ok; Mac crate tests ok (model chamfer/fillet/round/blend/shell, kernel). Pairs clean on the old
    base (chair, jump, chord: 882 = 882). The asm of the whole stack: unit `blend-chord-gates` on vibe6, blend6 at
    d04f5c0ae, log /root/blend-scratch/asm-chord-new.log, flag chord-asm.done. The earlier chair / jump asms died
    (earlyoom; a half-built target), this one covers them.
- cf-snap-hook (dx, 19:00): dx's vibe7 numbers (Fillet 6 145 ms, Fillet 3 69) are ~4.5x the Mac release (32 / 14;
  live 8 / 7; the document 243 ms vs live 274). Fillet 6's direct build: sections of cone x torus 20 pairs 32 ms CPU
  (45 lines; axes parallel but 50-230 mm apart: no circle shortcut), cone x cylinder 8 ms; in parallel.

## Queued to land

- The stack above, when chord-asm.done shows nothing worse than asm-combo-rest / asm-runlaw-inf: land from blend10
  `blend-chordnear` (land.sh rebases; rust_boolean.cpp untouched). Then tell core (Fillet 8 on their lean handle).

## Open questions / waiting

- infento 1dc32b Chamfer 3 (dx/core): geometry in branch `blend-ch3-wip` a2074e738 (rim_chamfer_cut by shared face,
  `prism_rim_chamfer_on`, a lone circle's corners mode): 87,057.04 mm3 as live when the 23 mm goes down both sides;
  our face-order side rule puts it on the bottom cap at the mirrored arc. Waiting on live REQUESTS 3169 (mirrored
  body's chamfer side). Then fix the rule, gate, land, tell dx and core; then recheck Fillet 3 (core's dump vibe5
  ~/agents/core-keep/fillet3-1dc/fillet-58).
- Boolean sections (bool's): the knife's remaining Chamfer 1 58 ms / Fillet 2 17 ms are OCCT on pairs that touch
  within the knife's ~21 um tolerances (measured: a disjointness test proves 4 of 96; parked hulls-wip.patch). bool's
  plane x B-spline marcher (71efdd87e) withdrawn (knife Mirror 1 regression), bool chases it (decfe3013034332d).

## Next, in order

1. cf-snap-hook Fillet 6 / 3 sections (above); then Fillet 5 / 4.
2. cf-spur-gear's opFillet blend 65-120 ms (dx).
3. Knife Fillet 2 (100 ms, live 10): OCCT's split of the run-on tool by the extended walls 25 ms (the grind wall alone
   17: a planes-first two-step split gives the same kept piece, saves ~5 ms only), met-face distances 6 ms,
   extend_face_over 10, the fuse 25. Fillet 3 (87, live 11): OCCT's try 28 ms thrown away (only 5 fillets in the
   suite take that path: no predicate), blend_direct 27, lofts 21.
4. Knife parity after core's Revolve 1: Fillet 8's spring-line split at the support's G2 knot lines (live 3093:
   6 vertices on our side surface's C1 joints u 0.61249 .. 1.38751, not at u 1.0). Parked as one case so far.

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
- Pairs / asm on vibe6 (lead), through `~/bin/slot` there (my scripts call it): bin/host-pair.sh <tag> <host> <base wt>
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
