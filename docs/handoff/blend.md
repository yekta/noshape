# Blends (agent "blend"): current state

Updated 11-16 04:03 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Landed b106b424 (20:33): chamferair, balljump, chordnear, ballnear, unifycand. Core told; vibe6 gate targets gone.
- Dock tab B scale-first: commit 20c8533d4 in blend6 (on main 84f94ad8a; g++-13 OK): short_step_beside fires only
  where exactly one rounded edge ends at a step under r / 4 (Halka Round, two rounds meeting at a 10 um line, excluded:
  its as-is round is kept and scaled-first cost it +20 ms). Log-mode suite (old trigger): 98 docs, the trigger fired on
  2 fillets, both "as-is not kept"; never on a kept one. Dock tab B with scale-first: every fillet's part identical
  (faces/edges/vertices, volume, area) to log mode; Rib Round Dizi ~270 -> ~160 ms, Rib Round 1 ~150 -> ~130 (vibe6
  at load 15: candidates). Rib Dip R 1 / Dizi don't fire (their as-is round thrown away too: a wider trigger later).
  qa's userdocs.py on vibe5 (green 84f94ad8a = base vs my CLI): hat15, hat15b, hat8, dock and the 47 ticket docs all
  SAME (log userdocs-sf.log). Still queued on vibe6 (unit blend-seq-sf, bin/seq-sf.sh; behind asm's s49 in the slot):
  model pair blend2 (main) vs blend6 (suite-sf-*: every suite doc, corner-break's 42 faces), then the assemblies
  (asm-sf-{base,new}.log). Land if all identical.

- c2-impeller-loft feature 26 "Chamfer Stevenrohr Halterung": commit 12b25c47e (blend3, on main 6acd980f6): the rim
  chamfer cut's end edge (OCCT's B-spline, 1.7e-3 rad / 6 um off the side line over 3.5 mm) joins the line as one line
  on its extension (join_edges_at's max_turn: the far vertex moved onto it, the pcurve kept in the line's period; the
  unify's concatenation threw "Courbes non jointives"). 33/88/56 -> live's 33/86/54; kernel T1
  blend::a_straight_section_turned_off_a_line_joins_it_on_the_lines_extension. Pair blend-base (main) vs blend3 +
  g++-13 queued on vibe5 (unit blend-imp-pair, suite-imp-*, flag imp-pair.done; behind bool2's gate). Core and dx
  told. join_edges_at is in boolean.cpp but only blends call it (bool's gate not needed: no Boolean path changes).
- side-cuts-tab Corner break 1: core's (queued by core 03:2x).

## Open questions / waiting

- infento 1dc32b Chamfer 3 (live 3169: 23 mm on the cylinder at both R65 arcs, by volume 87,057.04). Ruled out: a
  merged face ranking as the later Add's (live G60: both variants on the top, as our rule), qAdjacent / qOwnedByBody /
  shell order (the two arcs, congruent under Transform 12's rotation, come out opposite). x_t (live q-x38, read with
  our reader + ZZ_XT_FINS debug, blend3 uncommitted): at both arcs the edge's first (forward) fin is the cylinder's;
  on G37's block the first fin matches G30 only 4/6 ((a) front, (e) right fail), our class rule 6/6. Fits all: class
  order when both faces are one feature's, else the forward fin (left face) (G30 (i) too). Needs Parasolid's edge
  direction, which our OCCT edges needn't share. Decision asked of the lead. Parked: parked/ch3-alias-rule.patch.
- c2-impeller-loft Fillet 3 (feature 27, now the first difference with 12b25c47e): CONIC R 3 rho 0.5 on the two
  joined 59.4 mm lines at y = +-2.6104, z = -43.106; each is the lip of a lens-shaped wall: face 0 the 5.2 mm bottom
  strip of a 9.47 mm bore (concave cylinder, axis z = -34), face 1 / 3 Extrude's surface below it, 9 deg apart.
  frame_at's 45 deg probes all classify outside ("could not orient a face at the edge", 10 ms). With the face's own
  normal as fallback (parked/f3-wedge-frame-wip.patch) the conic cut leaves 2 solids, 2.1 s; live removes 280 mm3,
  33/86/54 -> 34/95/62. Fillet Stevenrohr zum Halter (29) follows from it (its edges are Fillet 3's). Probe
  parked/zz_probe_f3.rs, dumps vibe5 ~/tmp/blend/im3 (fillet-16 = the part after the chamfer). Also: the chamfer
  takes 684 ms (live 20).
- isaac-ball 3c5c58 Fillet 1 (asm; 42/112/68 vs live 41/106/63): 14 line edges, R 4.5; face 21 has a 9 mm strip:
  full rounds across it (live one half cylinder each, face 21 split 618.32 + 216.00). checked_blend's hair-smaller
  retry leaves a 9e-9 m neck in face 21. Branch `blend-necks` d6de0d70f (blend4): necks cut (all, at the part's scale
  when the working-scale cut isn't valid there), pcurves stored on new plane pieces, full rounds 5e-5 R apart.
  73 + 80: one half cylinder 127.2345 mm2, face split, as live. 76 + 77 (needs the 1e-5 retry): the cut comes out
  invalid again once scaled (a sewn plane piece), so the neck stays. All 14: 42 faces. Probe zz_probe_m3c, dump vibe6
  ~/tmp/blend/m3c-fillet.brep. Next: find why a sewn piece breaks under BRepBuilderAPI_Transform.
- GearLab 90f78b (asm-isaac-ball; core's opLoft fix): two causes. Ours: a variable fillet run found its edges by name
  and the tooth root's two share one (same edge twice); fixed on branch `blend-gearvar` 8055969d5 (+ a fillet-variable
  op dump). Then OCCT can't round either edge even at a constant 0.2 mm (90.24 deg cone / loft B-spline): asked core
  to check the new loft face (dump vibe6 ~/tmp/blend/gd/fillet-variable-15). Land the name fix with a T1 later.
- Boolean sections (bool's): the knife's Chamfer 1 58 ms / Fillet 2 17 ms are OCCT on pairs that touch within the
  knife's ~21 um tolerances (a disjointness test proves 4 of 96; parked hulls-wip.patch).
- cf-snap-hook (dx): Mac release Fillet 6 26 ms (live 8), Fillet 3 14 (7); the document 243 vs live 274. Low.

## Next, in order

1. Dock scale-first (in flight).
2. Impeller Fillet 3 (above); then Chamfer 3 when the lead decides.
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
