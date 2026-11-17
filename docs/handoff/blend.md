# Blends (agent "blend"): current state

Updated 11-17 01:50 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

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
- In suites (units blend-seq-nk then blend-seq-gv on vibe6; logs suite-r-nk*.log / suite-r-gv*.log, .cmp):
  - `blend-necks` 736f4c2df (blend4, on main 11fbe0901): isaac-ball 3c5c58 9 -> 32/33 (Mac; first diff now Extrude 8,
    52/129/71 vs live 51/128/71). Necks cut at every face (own scale where needed), plane pieces' pcurves stored,
    sewn strip ends dropped, full rounds 5e-5 R apart merged, fillet joins edges after merged_blends too, and core's
    join_loop_pieces joins two arcs of one ellipse (rebuilt as one ellipse, SameParameter tolerance; !legacy).
    Now c47d0c2a7 with core's two suggestions (9-point surface check, keep the arcs if SameParameter's tolerance >
    hair); core OK'd. Strict suite 127/127 same, g13 builds; userdocs on vibe5 (unit blend-ud-nk, ud-nk.log). Land.
  - `blend-gearvar` (blend3, on main): variable fillet edge by name + ends (GearLab 90f78b's shared tooth-root name);
    T1 unit test detail::tests::a_variable_fillets_edge_of_a_shared_name_is_found_by_its_ends; base blend8.
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
- impeller Fillet 3: PARKED by the lead (behind speed and the gearbox fillet). WIP only in parked/imp-f3-conic-balls-wip.patch
  (blend10 is now the speed branch), which also holds the ZZ
  timers, ZZ_NO_BALLS and the zz_probe_lens probe: strip those before landing): the rib under the bore is 5.2 mm wide,
  its lips ~30 deg; live's round is a true R3 rolling ball on the curved faces (contacts y -1.524 on the bore,
  (0.957, -46.774) on the side: ours now matches), the conic between, the two rounds meeting in a ridge. Done so far:
  section_blend retries a conic constant-radius round as rolling balls (ball_section takes the Section: conic apex
  where the contact tangents meet, weight rho/(1-rho)); into_face accepts a curved side by its sag (< step/5);
  Support::Extruded (feet on a swept face moved along its direction, past where it is trimmed); a swept face's
  section side interpolated through 9 feet (an arc was 0.04 mm / 17 deg off at the lip); lies_on classifies with
  periodic recadre. Both tools' sides recognised; the ridge comes out at z -43.902 (live -43.9), the side trimmed at
  x 54.68 (live 54.7). What's left is the ends. Far end: the blends cross the rib's old footprint on the channel's
  offset face (interior, not a face), so the direct build (pieces of existing faces only) can't close there. Near end:
  the tools run on past the lips' start (x 1.5) to -1.5 and cut through the rib's join with the collar at x 0..5 (plane
  x 0, r 6.25 hole, cone, plane x 5): the Boolean fallback (tried) leaves that collar as a 676 mm3 second solid; live
  ends the rounds on the chamfer faces (6.18 -> 12.68 mm2). Needs end trimming by those faces (a capped tool or the
  direct build growing a face into an interior region): a larger build. Impeller corpus runs again on main (55552e704).
- asm, skasm, bool2, studios, live, live2 paused until ~11-17 10:15 UTC: no messages to them, write into their
  handoffs or live/REQUESTS.md (lead 11-16).
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

- Knife Fillet 8 parity (blend10 `blend-f8split` 90deb589a WIP with debug; parked/f8-spring-split-wip.patch):
  - Split rule found: split a round's edge on a B-spline support where the support's normal curvature ACROSS the
    round jumps > 20 % over a C1 knot line (live's 6 splits jump 39-72 %, its 2 non-splits 3-7 %; the along-edge
    curvature jumps at all 8). split_at_support_joints after join_split_lines, pieces marked kept_ends (else
    join_loop_pieces rejoins them), marks carried back through Scaled::unscaled. 2.3 ms (2.0 the face check).
  - Alone it moves Fillet 8 16/34/20 -> 16/40/26 (live 15/37/25): not landable alone. Ours has two round faces
    because the ring is two OCCT edges (sphere seam; shown as one) and OCCT rounds each edge as a face. Live's one
    face needs the halves joined as one B-spline closed on its own seam + side pieces joined across y 0 (~1 day);
    then 15/37/24, the last vertex being Revolve 1's closed-edge vertex (core's). Only knife Fillet 8 in the suite
    has this (knifehandle Fillet 3's +1 face is a planar round face, other cause). Decision asked of the lead.

## Next, in order

1. Land blend-necks (userdocs) and blend-gearvar (suite); then the lead's call on Fillet 8 (above).
2. Knife Fillet 8 speed (67 vs live 12: as-is 7 ms fails, working-scale round 47).
3. Land blend-necks (after core's OK) and blend-gearvar when their suites hold; GearLab then needs OCCT to round the
   tooth root (core checks the loft face: dump vibe6 ~/tmp/blend/gd/fillet-variable-15).
4. Impeller Fillet 3 (parked WIP), Chamfer 3 when the lead decides.

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
