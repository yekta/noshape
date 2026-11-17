# Blends (agent "blend"): current state

Updated 11-17 08:10 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Speed is judged on the Mac (release, quiet, median of 3) against live (DECISIONS 11-16); vibe numbers are hints.
  Done: robot-relations a4e419 (Mac 65 vs live 78), cf-part-draft Spur Gear (76-81 vs 227).
- Landed 11-16: 684e77e2 (samples / ball tools ahead), 534c6e30 (lies_on skip, loft orientation by rays, run-on trims),
  ce61a597 (scale-first), 35c63214 (Fillet 3's direct build beside OCCT's try, loft probe two-sided per core's review,
  sliver check by 1% areas). Mac x3: knife Fillet 2 95 -> 85, Chamfer 1 117 -> 112, Fillet 3 63 -> 47, Fillet 5 23 ->
  21, Fillet 6 20 -> 7 (live 7); knife ~1020 ms (live 622). Lead split the knife's non-blend time: bool (Split 4,
  Mirror 1, Chamfer 1 / Fillet 2 Booleans, Revolve 1, Extrudes 2/3/8/9), core (Extrude 10, Lofts 1/3); both told.
- Landed 11-17 00:42: 0aad993d (367cf3ae) conic contact edges checked by the faces around them (Fillet 3 47 -> 45).
- cf-spur-gear (11-17, Mac release, main 5b23c028f): every Spur Gear is under live (117/56/34/98/77 vs 955/95/75/283/
  343); its opFillet is 0.3-0.4 ms, the time is the Rust op_boolean (112/46/26/78/53 ms). Sent to bool. Nothing for blend.
- Landed 11-17: blend-necks ba7a8d755 (isaac-ball 9 -> 32/33) broke rust-zero-solid-f197's Mirror 3 (4 slivers: the
  retry's necks cut on a result already valid changed Chamfer 3); reverted (b596da51b), re-landed 07:05 as 74d05ad5e:
  a valid retry keeps its necks unless cutting them lets a full round merge. T1 blend::a_valid_chamfers_necks_stay_and_
  its_mirror_adds_whole (data blend/f197-chamfer3-input.brep). Before landing: userdocs + f197 + Hole docs + every
  ~/qa/tickets doc + qa's compat v0443 docs SAME (bin/ud-nk3.sh, the release-candidate set), g13 OK. GearLab
  (b2e0cbd71) landed.
- qa P0 speed (11-17, vibe7 A/B v0.4.48 -> v0.4.53): knife Chamfer 2 10.7 -> 67.9, Fillet 5 18.8 -> 95, Fillet 3 136
  -> 188 ms; impeller Stevenrohr zum Halter 0.6 -> 2610 ms (fixed by 29c1841df, after .53; Mac main 2 ms). v0.4.53
  predates the 11-16 speed lands; to do: Mac A/B v0.4.48 vs main on those three (bin/ab-mac.sh; commit WIP first).
- Knife blends left (Mac): Fillet 8 65 (live 12: OCCT's working-scale round 45 after the as-is fails 7; split-line
  join 5.8 needed), Fillet 3 45 (direct build 26: classify.tool 7 = ray intersectors on lofted tools; lofts; conic
  rebuild), Chamfer 2 35, Fillet 5 21. Probes: parked/zz-*.patch; Mac A/B: bin/ab-mac.sh <base> <cand> <doc> <regex>.
- Backlog (lead: parked, >1 day): Fillet 5 to live's 9 ms needs a contact-curve build: blend faces made straight from
  the ball sections and contact curves (no solid tool), supports trimmed along the contact curves in 2D (no lies_on,
  no IntSS on supports), the shell from known adjacency, only new faces checked; est. Mac ~8-10 ms, 3-4 days incl.
  every rolling-ball fillet identical on the suite.
- Landed 11-16: d7f9264c9 impeller chamfer end edge joined (impeller 26 -> 27/48 on main; strict suite 127/127, userdocs
  SAME). Core told.
- Scale-first blend6 4b2f94289 (base 3178c98b5): release suite strict-identical 127/127, userdocs ALL-SAME; assemblies
  (vibe6, asm-sf-base.log / asm-sf.log, unit blend-seq-sf3) running; land when identical.
- Live's knife times (11-16): Fillet 1-8 8/9/10/6/9/27/7/12 ms, studio 622. Lead: Fillet 5 at or below live is P0.
- impeller Fillet 3 (P0, blend10 `blend-impf3`, WIP uncommitted on 8e269426d): builds as one solid 71,097.14 mm3 (live
  71,096.12), 34 faces as live, edges/vertices 93/60 vs 95/62 (the front circle's 3 arcs joined by the unify: a fix
  run in flight), Fillet 3 13 s (live 0.1 s: speed next). Pieces: closed_run_on takes all conic tools at once; an end
  whose first met face doesn't cut the tool is a corner: walls = that face + its neighbours bordering a support
  (7, 17, 4 = the chamfers and the x 5 face), tool less the cells behind every wall (one Boolean, so the round face
  stays whole); new kernel op merge_on_surfaces (boolean.cpp): pieces on copies of an extended face's surface put on
  one and merged (core's unify refuses: its 1e-9 volume check vs 1e-7 integral noise; ours checks 1e-6 by
  volume_props), vertices left between pieces of one curve kept (KeepEndsMark, live keeps them). Debug left: ZZ_*
  prints (blend_section.rs, boolean.cpp ZZ_USD, closed_run_on), probes in kernel tests/blend.rs zz_*. Then
  Stevenrohr zum Halter ("Adjoining edge not filleted.", 2 ms), then tell core. Live's chamfer extension reaches
  x 0.097 at the deep contact, ours 1.5 (OCCT's B-spline extension vs Parasolid's): areas 1.6 mm2 apart, counts same.
- Agent ids (11-17 09:25, fresh threads): asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm
  18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
  58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2
  3b9b1e66-33a0-4dfe-9e0a-eee216e354b2, bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4, core 6692ae35-758a-4442-81c8-29f67be2e21a.
- Queued from asm (after the current items): asm-gearbox-std 6a912b / a470167 Fillet 2 (EDGE fillet of 2 faces' edges,
  tangent, 0.125 in: ours 38/92/56, live 38/88/52, -12.1 mm3; repro vibe6/7 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/mgb
  NOSHAPE_PARITY_ONLY=m6a912b); asm-scissor-lift fed22d Fillet 3 (Top - Side 2 45/110/64 vs 48/116/64; Motor Mount
  69/128/50 vs 66/121/46), then Fillets 4, 5, 15 fail (repro vibe7 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/ms
  ~/agents/dx-keep/ops-gB3; on main + cf49196a it differs at feature 6 Mirror 1 already: parked until dx's translator
  work lands). Gearbox: live's corner topology asked (REQUESTS 14:02); ours at each of 4 corners an OCCT 48.7 mm2
  B-spline patch + a 4-sided 12.72 mm2 torus.

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
  parked/zz_probe_f3.rs, dumps vibe5 ~/tmp/blend/im3 (fillet-16 = the part after the chamfer). Live (live.jsonl
  26 -> 27): removes the bore strip Jee, both lips' Extrude faces trimmed (840 -> 604 mm2, x to 54.7), the chamfer's
  two 6.18 mm2 tool faces -> 12.68 mm2; two conic faces JfC / JfG (177 mm2, y 0..+-1.02, z -43.9..-46.77) meet in a
  ridge JfZ along y = 0 at z -43.9 (below the bore's bottom -43.47): each round overflows the strip, touches its wall
  far down and is clipped by the other. Needs a two-round overflow build (Parasolid's), not section_blend per edge.
  core waits on 27-29 (Extrude impellerraum on depends on them). The chamfer takes 0.6-1 s (live 20): 4 Booleans of
  rim_chamfer_cut's tool after OCCT's chamfer fails twice and section_blend once.
- Boolean sections (bool's): the knife's Chamfer 1 58 ms / Fillet 2 17 ms are OCCT on pairs that touch within the
  knife's ~21 um tolerances (a disjointness test proves 4 of 96; parked hulls-wip.patch).
- cf-snap-hook (dx): Mac release Fillet 6 26 ms (live 8), Fillet 3 14 (7); the document 243 vs live 274. Low.

- PARKED (lead 11-17 04:10, impeller P0 first): knife Fillet 8 join, blend10 `blend-f8split` c7fe2dc61 (rebased on
  main; parked/f8-ring-join-wip.patch). Fillet 8 15/37/24 vs live 15/37/25 (the vertex: Revolve 1's closed-edge vertex,
  core's). Pieces: join_ring_rounds (round faces of edge pieces shown as one edge: rational B-splines end to end, the
  other cross edge the seam, then loop_pieces_joined, a wrapper of core's join_loop_pieces), split_at_support_joints
  (across-curvature jump > 20 % at C1 knot lines), a new KeepCutsMark (kept as kept_ends by joins / unify, not shown
  where a seam runs through), marks carried both ways through Scaled, join_split_lines never between two faces the
  blend left (it had joined the handle's far-end lines). To do: remove the 3 join_rings timers, cut its ~10 ms
  (whole-solid checks / shown_topology), T1 test, strict suite + userdocs + g13, core reviews common.cpp / boolean.cpp.
  Then Fillet 8's speed (67 vs live 12).

## Next, in order

1. P0 (lead 11-17): impeller Fillet 3 (counts, speed, strip debug, suite/userdocs/g13, land) and "Fillet Stevenrohr
   zum Halter"; tell core when either builds (core told 08:10 of the WIP state).
2. qa's P0 speed A/B (knife Chamfer 2 / Fillet 3 / Fillet 5).
3. Unpark the knife Fillet 8 join, then its speed.

## Parked (with where)

- Known sections API: /root/blend-scratch/parked/knownsec-wip.patch (bool agreed to park; tangent support pairs aren't
  the cost).
- Debug patches: parked/f8-spring-split-wip.patch (split + ZZ_KNOTS report), parked/zz-ahead-dbg.patch (per-pair occt_ahead time), knot-report-dbg.patch (round-edge crossings),
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
