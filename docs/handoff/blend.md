# Blends (agent "blend"): current state

Updated 11-16 20:13 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Speed (P0): `blend-samples` LANDED as 684e77e2 (11-16 16:14; SamplesScope, ball tools ahead of OCCT's try, lies_on
  per tool in parallel, conic_contacts prefilter). A/B vibe7 (load 5-8, a hint), main -> branch: knife Fillet 5 89 -> 57
  ms (live 9), Fillet 3 202 -> 156, Fillet 8 243 -> 178; a4e419 Fillet 1 179 -> 132 (live 16), regeneration 291 -> 223
  (live 78). Mac release: Fillet 5 41 -> 23, a4e419 83 -> 65.
- Speed is judged on the Mac (release, quiet, median of 3) against live (DECISIONS 11-16); vibe numbers are hints.
  robot-relations a4e419 is done (Mac 65 vs live 78). cf-part-draft Spur Gear done (Mac 76-81 vs live 227).
- QUEUED a9cd8f47 (11-16 20:12; blend5, 3 commits): lies_on skips points found on an earlier face; a natural solid
  loft oriented by two rays off its faces (signed volume only when they disagree; core reviews: lead told core);
  trimmed_run_on's extension projects in parallel and finds met faces by a run-on vertex first. Strict suite 127/127
  (suite-r-tk.cmp, vibe7), userdocs ALL-SAME, g++-13 OK. Mac A/B x3 (bin/ab-mac.sh): knife Fillet 2 95 -> 85, Chamfer 1
  117 -> 112, Fillet 5 23 -> 21, knife 1049 -> 1038 (live 622). Loft check (every natural solid loft: rays vs volume,
  ZZLOFT lines) queued on vibe6 (unit blend-loftcheck, blend7 -> suite-loftcheck.log): report agree / unsure counts.
- Next: branch `blend-ahead` (blend9, 92f53ad4f on a9cd8f47): the rolling balls' direct build made in the
  speculative thread beside OCCT's try (with_cancel(stop)); Mac A/B: knife Fillet 3 63 -> 47 ms, others unchanged.
  Risk: a non-split multi-edge fillet waits for the cancelled build's current phase. Suite pair queued on vibe6 (unit
  blend-seq-ah: blend5 vs blend9 -> suite-r-ah.cmp; compare per-feature Fillet / Chamfer times too), then userdocs, land.
- Knife blends left (Mac): Fillet 2 85 (live 9): bool's Boolean sections (split 25 + fuse 26: plane x grind
  B-spline pairs 15-18 ms each); handed to bool by the lead (after its screws P0). Chamfer 1 ~112 (live: asked in
  REQUESTS 17:01): Boolean sections 52 of its 63 ms (lofted tool x B-spline pairs, 20-50 ms with no section; bool's);
  its tool sides cross the knife's faces (0.3-2.8 rad), so no direct build. Fillet 8 64 (live 12): OCCT's round at the
  working scale 45 after the as-is one fails 7. Probes: parked/zz-fillet2-chamfer1-probes.patch.
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
