# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-17 11:38 UTC (restarted 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## Landed today
- 0bb94eede (11:23) flat-view colour fix (a FeatureScript feature's colours go to the bodies it leaves); psui told.

## Unlanded (worktree /root/projects/noshape-wt/skasm, branch agent/skasm)
- Committed: d3e6bcef5 (commit A of 11-16), b0bee2b62 (sheet metal chips on re-edit), a26f18913 (Bezier poles,
  BEZIER_DEGREE, EQUAL_CURVATURE), 036070cc4 (INTERSECTED, ids-only spline edges), 52447d4eb (Pierce takes a curve,
  Concentric model point, anchored solves kept), e5f43fb96 (Pierce 3-pick refusal), ca5c2d9f0 (configured documents
  drawn at sketches.json's captured solve), handoff commits.
- Uncommitted (parity.rs + tests): a pattern direction naming a mate connector takes the connector (linear-rail
  24 -> 26/26); a configured Variable's value goes to its typed key (sprockets); a feature list drops ids of features
  deleted from the document, and a hidden parameter's failed query isn't a problem (pulleys' Circular pattern 1 and
  Extrude 2); a solve onto where live's edge (ending on the plane) pierces the sketch is kept (pipe's Loft profile 3);
  tests stale_references_live_ignores_name_nothing, a_configured_variable_value_is_configured,
  a_solve_onto_lives_pierce_point_is_kept. Our Pierce of a circle grows its radius (5 -> 6.5 mm in that test); live
  keeps the radius and shifts the circle: a solver item.
- Single-doc effects vs main: config-parts 86 -> 131/156 (+2 linear-rail now); manualblend 9 -> 11/19; cpcurves
  7 -> 8/24; tablet-stand 16/16.

## Running
- skasm-pair-1117a: vibe6 slot pairrun.sh (ops-a = main 143f22c12, ops-b 09:48) -> ~/agents/skasm-keep/pair-1117a-
  {a,b}.log; a started 11:17.
- skasm-mac-1117p: Mac parity:: tests -> /root/skasm-scratch/mac-1117p.log (needs a rerun with the hidden-parameter rule).
- skasm-pu-1117c, then skasm-tr-1117a (pulleys, pipe traces) -> /root/skasm-scratch/tr-<doc>.log, vibe6
  ~/agents/skasm/trace-<doc>.log.

## Next, in order
0. USER P0 (lead 11-17 12:30): sketch interaction parity, ahead of config-parts. psui owns the viewport pick / hover
   emit (planes under the open sketch in the GPU pick); mine is the sketch side. Done, in test: hover re-resolved on
   each PointerMove (`sketch_hover`, `ui.sketch_gpu_hover`; GUI test a_line_lights_over_the_plane_the_gpu_pick_keeps;
   the user's frames lead-scratch/vid/hover/t01..t05). Then, with live2's capture (coming) and behaviours.md: constraint
   creation by selection (two lines -> the toolbar's options), inference and snapping while drawing, drags of points,
   lines and dimensions, dimension placement and edit, constraint-icon hover. Each fix with an interaction-feel test
   (docs/parity/interaction-feel.md "Sketch").
1. Stack 9d4a1a74 queued 12:2x (landing: tell bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, standoffs Revolve 1 fixed by
   ca5c2d9f0, 13 -> 31/32).
2. config-parts' rest: pulleys Circular pattern 1 handed to core 11:45 (face pattern ADD fails in our op, main too);
   sprockets 16/24 (one volume, 1.6e-6: live's band asked, REQUESTS 11:28 (a)); standoffs Hole 2 message (rule asked
   (b)); links Derived 1 (source studio not in the corpus, asked (c)); nuts Sweep 1 (bool's).
3. Surface workshop: pipe now stops at core's Loft (guides with more than two profiles); knifehandle P4 Pierce taken
   back (its edge doesn't end on the plane), first difference Sweep 2 area (core); britatap Split topology (core);
   manualblend Sketch 5 Concentric 128 mm (upstream JNE), Sketch 6 / 7 INTERSECTED geometry not found. Pierce of a
   circle grows its radius, live keeps it (asked REQUESTS (d)).
4. core's sketch cases below. 5. tablet-stand Flange 2. 6. Remaining dropped constraints (see archive).

## core's sketch cases (2026-11-16), queue item 3
- c2-sm-rocker-enclosure-sm Extrude 2 (now its first difference once core's split-face attributes fix lands, 8/14):
  Sketch 4's four circles are PROJECTED (Use) constraints whose live external edges (refs.jsonl
  AjhQtHt4MbMf.{0..3}.project: x 45–60 mm, y ±78, z −91 / −21) aren't on our part. Our sketch comes out with two
  circles (both rows map onto one), so Extrude 2 names one region twice ([0, 19] twice) and cuts 2 holes of live's 4
  (ours 26/52/24, live 28/56/24). Probe: `NOSHAPE_PROBE_SKETCH="Sketch 4" NOSHAPE_PROBE_PARAM=entities` in
  parity::bodies_after_feature_probe (core's probe patch).
- c2-loft-guide-curves-wave-washer Loft 1 ("Could not create loft", 6/7): Sketch 4's two rectangles (Right plane)
  stay at their stored x 77.47–88.9 mm; live solves them onto the ring at 13.97–25.4 (live's Loft 1 faces all lie in
  r 13.97–25.4). Each has COINCIDENT bottom.start to an external vertex by deterministic id only (queryString ""):
  JHk (0, 25.4, 3.81) mm, body JHb, and JH4 (0, −25.4, 3.81), body JHv, both in refs.jsonl. No vertex of ours sits
  there (our Routing curve is one closed B-spline edge, its one vertex at (17.96, 17.96, 3.81)), and parity.rs's
  VERTEX resolution then falls back to the nearest vertex anywhere (`pick(&r, "vertices")`). The constraint seems to
  have no effect (the rectangles stay as stored). Live's point probably wants a fixed point at live's position when
  nothing of ours is there. Loft 1 itself (four profiles round a closed path) is core's once the profiles are right.
- c2-config-parts-links Extrude 1 (10/35; parts 3.2 mm across, live 5): Sketch 1 is stored at Pitch 03B (p 5 mm)
  and live solves it at 05B (p 8 mm, sketches.json). Our dimensions do reach live's points except the two waist arcs
  (h5M8JHVkSHwo, U0aWy9P0xf2C: tangent to both circles, through the waist point at (4, ±2.6625) mm). Live keeps the
  large concave arcs (centre (4, ∓8.57), r 5.91, as stored); our solve jumps to the small convex ones (centre
  (4, ∓1.82), r 0.84). Off live by 6.75 mm instead of 3.35 before, so the translator takes the edits back and the
  stored 5 mm sketch stays. A solve that keeps the branch (step large dimension changes, or keep each arc's side of
  its chord as stored) would fix it. Seen with a temporary eprintln in off_live_points (per-point live vs ours).
- The other c2-config-parts documents stop at a sketch the translator takes back too, where the stored geometry is
  another configuration's (core's 97b778e7d, queued: they're now built at sketches.json's capturedConfiguration, the
  defaults live's lines are at): pulleys Sketch 2 (after the solve only qW2yvEmz0tbj.end, the free end of a
  construction mirror line, sits 0.2 mm off live's: 1.09 -> 0.20 mm, but `closer` wants <= 1e-6), linear-rail
  Sketch 2 (1.92 -> 0.30 mm), standoffs Sketch 3 (2.38 -> 3.29 mm), sprockets Sketch 3. A `closer` that ignores free
  construction points (or compares only points the regions use) would likely keep pulleys' and linear-rail's.
- c2-surf-workshop-cpcurves Main pipe (7/24): Sketch 4 (on Plane 1, a curve-point plane at live vertex JPg
  (158.574, -97.976, 0) mm) has its circle centre COINCIDENT to JPg. The stored circle is 18.8 mm off it (stale: the
  Layout moved), and live re-solves it onto JPg. With no sketches.json for that document, the translator takes the
  move back ("moved the geometry by up to 18.798 mm ... taken back"), so our tube runs 18.8 mm off live's. A solve
  that lands an external COINCIDENT exactly on live's recorded refs point could count as `closer`.
- c2-surf-workshop-manualblend Sweep 2 (11/19): Sketch 5's circle (the profile, on Plane 3) has its CONCENTRIC
  rejected ("constraints left to the solved geometry: 1 CONCENTRIC rejected"), so it stays where stored, ~100 mm
  from the path (our Curve 2 matches live's); our swept surface lands at x 67–95 mm, live's at −61…−26.
- c2-sm-rocker-enclosure-body Extrude 6 (15/28): Sketch 6's solve is taken back ("COINCIDENT on
  R4oICkWq6xtC.cCircle.center, oKC7s6ob7uli.0.center by 1.974 mm"; also "CIRCULAR_PATTERN: the instances aren't
  copies of the seeds"), so our hexagon-and-lines stays as stored: one region of 107.106 mm² where live has two
  (84.163 + 22.943, refs.jsonl) at the solved place; the cut removes 115 mm³ more than live's and has one face more.


## Agents (lead's roster 11-17 10:30; only these ids)
psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (since 11-17 ~10:35), core 6692ae35-758a-4442-81c8-29f67be2e21a, blend
0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4, dx f93a1292-5bb7-406f-9a25-59767e01878e,
qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm (me)
18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 3b9b1e66-33a0-4dfe-9e0a-eee216e354b2.

## Waiting on others
- psui: told 20:3x the tab row is a pick difference (live hit Flange 1's wall).
- dx: cf-spur-gear's per-host Sketch 2 failure = the near-singular sketch 62893b1ed takes back (told 21:3x).
- Not mine (handed): knife / impeller geometry (core); fan-blade Boolean 2, gamma / wire-clip Boolean 2, shaped-flange
  Booleans, surface-pattern sphere cuts (bool); setback fillet (blend); hex-bookcase mesh Uses (dx).

## Open questions (none blocking)
- s236 (fuzz probes 11): live solves three linked quads at once; per-case placement rules don't converge. Decision
  for the lead if fuzz work resumes there. s228 has its rule (grow_slot); s226 is 3 µm off (live's anchor drifts).
- Jog at its reduced angle: our flat lays two bend strips 0.73 mm over each other (features_c
  jog_preserve_material_off_and_errors' second case; live's doesn't): the jog's definition has two middle faces 0.79
  mm apart, so coplanar_rip doesn't join them.
- Sheet metal "Collision in sheet metal flat pattern." INFO for zero-gap contact at a tear (config-assembly 08590a's
  Flange 1 … Mirror 1): needs a contact rule, not an area check. (tablet-stand's was the curved wall: fixed in A.)

## Current numbers
- Model suite 2026-11-16 00:01 vibe6 (suite-2354.log): 80/111 documents, 1019/1292 features (24 documents new to the
  set). Sketch constraints left to the solved geometry: 586 over all, 74 over the 87 of 2026-11-15 (16:10: 83).
- Sheet metal speed, quiet vibe6 19:00, ops binary of 17:43 (ours vs live ms): corner-break 1637-1782 / 1972,
  shaped-flange 809 / 1026, overcut-dogbone 210 / 270, hem 65 / 95. All four are faster than live; the suite's slower
  figures were host load.
- Exchange: model-plus STEP / IGES import in live as 58/151/94 (6f5b151e snap_coplanar); dx "live imports ours" 43/48.
- Sketch fuzz vs live: 801 of 801 counted (29 live defects, 29 known settles, of 830); probes 20 11/11.

## Hosts and rules (mine; DECISIONS.md has the general ones)
- Builds and crate tests on the Mac: `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh bash -c '…'` (one slot; rb.sh keeps file
  times: touch an edited file inside the command when a result looks cached). No suites, pairs or app runs there.
- Suites and A/B pairs on vibe6 (or vibe7) through `~/bin/slot <cmd>` there. ~/tmp on vibe hosts is pruned after
  6-12 h: lasting binaries in ~/agents/skasm-keep (cli-*, ops-a / ops-b).
- Any change touching projection / Use: userdocs check (hats hat8 / hat15 / hat15b, dock) on vibe5 against 0.4.52
  (~/qa-rel/0.4.52/noshape-cli) before landing: candidate CLI built on vibe6 (buildcli.sh), copied with `scp -3` via
  this host (vibe6 can't reach vibe5). The private documents only as mode-700 copies in ~/tmp/skasm (or userdocs.py's
  ~/tmp/qa/ud-<pid>), deleted after with their outputs; never in the repo.
- Live captures: append to /root/onshape-research/live/REQUESTS.md (timestamps from `date -u`).

## How to run my tools (/root/skasm-scratch/bin)
- fullsuite.sh <log>: the whole model suite through rb.sh on $HOST (default vibe6); wrap the remote command in
  `~/bin/slot`. dropped.py <log>: count the constraints a suite log leaves to the solved geometry.
- ut3t.sh <doc>: one model-suite document with NOSHAPE_PARITY_TRACE (translator TRACE lines, untranslated, DIFFs).
  ut3p.sh <docs,comma> <models dir>: the same from another models dir (a fresh copy), binary kept as skasm-keep/ops-c.
- smab.sh: release ops test binaries for skasm-base (A, /root/projects/noshape-wt/skasm-base: reset it to my
  branch's base first) and skasm (B) on vibe6 -> ~/agents/skasm-keep/ops-{a,b}; vibe6 ~/tmp/skasm/smrun.sh <runs>
  <docs,comma> interleaves them (A/B by the match lines' times).
- buildcli.sh <name>: release noshape-cli -> ~/agents/skasm-keep/<name>.
- Profiling: NOSHAPE_KERNEL_PROFILE=cpu, NOSHAPE_FS_PROFILE=1, NOSHAPE_RECORD_BOOLEANS=<dir> (one .brep + .txt per
  distinct Boolean); Mac `sample` + sample_under.py <sample> <root symbol>.
- Fuzz: fz/cmpprobe.sh <set> <case> (initial / ours / live); probe.sh runs a probes/zz_*.rs test copied into
  crates/ops/tests (zz_sketch_speed, zz_skdump, zz_sksolve; never committed).
- stepadj.py: coplanar adjacent planes in a STEP file. cfg.sh: one assembly studio alone in the model suite
  (archive: "config-assembly's sheet metal studio 08590a").
- Parity translator changes asm made for the assembly corpus: archive section "Parity translator changes by asm".
