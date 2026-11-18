# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-18 03:55 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## Landed 11-18
- b6f5e810: snap onto another sketch's point / model vertex = one Coincident, no use glyph; joint row's flat edges.
- 0fda4de5: double-clicked face's sketch drawn through the part; Extend nearer end; offset arrow drag.
- 9c015658 (03:39): Rip -> Bend unfold failure (host `Unfolded::clashes`: a second bend path placing a wall elsewhere ->
  no flat, `cannot_unfold`, WARNING SHEET_METAL_COULD_NOT_UNFOLD, bend kept, yellow toast; a consistent loop such as
  qa4 S1's split plate still unfolds); Corner / Bend relief boxes (`SM_CORNER`); offset arrow zoom step;
  NOSHAPE_SNAP_TRACE; " radian" label; copy/paste test. SKSNAP-REACH closed not a bug by qa.

## In flight
- Sticky snap target (live getInference lastInferenceTarget): `SketchEditor::snap_target` + `Inference::targets`;
  scene test `the_line_snapped_to_last_stays_over_a_point_by_it`; feel row. live REQUESTS 11-18 03:46 asks for the
  real-pointer check (a line slid into its own end).
- Landed 4b4e4915 (05:04): sticky target, held-point solver fix.
- Queued: an opened sketch's draft takes the regeneration's solve (hat15b Sketch 1 was 1.1 mm off its stored place:
  hover / snaps / drags on the stored lines); `take_solved_draft`, `Sketch::adopt_geometry`.
- Solver (regeneration's constructive pass): a circle / ellipse Coincident with a Fixed point / model vertex
  translates with its size kept, a free point comes onto the curve (live coincident.json); was carried centre-onto-
  point (r 0). `carry_onto_placed` carries only point-point Coincidents; `kept_first_in` keeps a Fixed member and the
  curve over a lone point; `hold_size` holds an ellipse's radii. Fuzz 801/801 unchanged. Test onshape_ux
  `a_curve_onto_a_held_point_moves_whole_with_its_size_kept`.
- Open live REQUESTS 11-18 02:30: the unfold row's colour; Corner taking a rip side face (then the corner disc).

## Next, in order
0. USER P0 (lead 11-17): interaction feel. sm-feel-1117 left: Corner / Bend relief taking a rip's side face +
   the corner disc (std's findCornerDefinitionVertex throws SHEET_METAL_RIP_NO_CORNER for our rip face; native corner
   search needs both bends on one parent). Rigs on vibex6 :391: ~/tmp/skasm/sk (sks.sh start|shot|stop, SNAP=<json>;
   dbl.json = two-blocks double-click), keep binary ~/agents/skasm-keep/noshape (bin/appbuild.sh). Land
   NOSHAPE_LAND_QUEUE_HOST=vibe2. psui owns the pointer-offset report (latency, not an offset).
1. config-parts: linear-rail 26/26, links 35/35 (live fetched Derived 1's source), standoffs 32/32, sprockets 24/24
   (live's volume band carried while its volume stays: carry_volume_bands); pulleys 12/22 (Circular pattern 1, a face
   pattern ADD, fails in our op on main too: core's since 11:45); nuts 14/17 (Sweep 1, bool's). Standoffs regenerates
   in 12.4 s against live's 0.2 s: bool2's thread Booleans.
2. Surface workshop: pipe stops at core's Loft (guides with more than two profiles); knifehandle P4 Pierce taken back
   (its edge doesn't end on the plane), first difference Sweep 2 area (core); britatap Split topology (core);
   manualblend Sketch 5 Concentric 128 mm (upstream JNE), Sketch 6 / 7 INTERSECTED geometry not found. A circle or
   ellipse Coincident with something fixed: done 11-18 (Pierce of a 3D point not checked).
3. core's sketch cases below (links 35/35 now; wave-washer: live REQUESTS 11-18 04:1x asks what bodies JHk / JH4
   are). 4. Remaining dropped constraints: suite on vibex7 with ops-b (built 11-18 04:1x). tablet-stand 16/16 now.

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
58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (moved 11-17 13:3x).

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
- Model suite 2026-11-16 00:01 vibex6 (suite-2354.log): 80/111 documents, 1019/1292 features (24 documents new to the
  set). Sketch constraints left to the solved geometry: 586 over all, 74 over the 87 of 2026-11-15 (16:10: 83).
- Sheet metal speed, quiet vibex6 19:00, ops binary of 17:43 (ours vs live ms): corner-break 1637-1782 / 1972,
  shaped-flange 809 / 1026, overcut-dogbone 210 / 270, hem 65 / 95. All four are faster than live; the suite's slower
  figures were host load.
- Exchange: model-plus STEP / IGES import in live as 58/151/94 (6f5b151e snap_coplanar); dx "live imports ours" 43/48.
- Sketch fuzz vs live: 801 of 801 counted (29 live defects, 29 known settles, of 830); probes 20 11/11.

## Hosts and rules (mine; DECISIONS.md has the general ones)
- Builds and crate tests on the Mac: `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh bash -c '…'` (one slot; rb.sh keeps file
  times: touch an edited file inside the command when a result looks cached). No suites, pairs or app runs there.
- Suites and A/B pairs on vibex6 (or vibex7) through `~/bin/slot <cmd>` there. ~/tmp on vibe hosts is pruned after
  6-12 h: lasting binaries in ~/agents/skasm-keep (cli-*, ops-a / ops-b).
- Any change touching projection / Use: userdocs check (hats hat8 / hat15 / hat15b, dock) on vibe5 against 0.4.52
  (~/qa-rel/0.4.52/noshape-cli) before landing: candidate CLI built on vibex6 (buildcli.sh), copied with `scp -3` via
  this host (vibex6 can't reach vibe5). The private documents only as mode-700 copies in ~/tmp/skasm (or userdocs.py's
  ~/tmp/qa/ud-<pid>), deleted after with their outputs; never in the repo.
- Live captures: append to /root/onshape-research/live/REQUESTS.md (timestamps from `date -u`).

## How to run my tools (/root/skasm-scratch/bin)
- fullsuite.sh <log>: the whole model suite through rb.sh on $HOST (default vibex6); wrap the remote command in
  `~/bin/slot`. dropped.py <log>: count the constraints a suite log leaves to the solved geometry.
- ut3t.sh <doc>: one model-suite document with NOSHAPE_PARITY_TRACE (translator TRACE lines, untranslated, DIFFs).
  ut3p.sh <docs,comma> <models dir>: the same from another models dir (a fresh copy), binary kept as skasm-keep/ops-c.
- smab.sh: release ops test binaries for skasm-base (A, /root/projects/noshape-wt/skasm-base: reset it to my
  branch's base first) and skasm (B) on vibex6 -> ~/agents/skasm-keep/ops-{a,b}; vibex6 ~/tmp/skasm/smrun.sh <runs>
  <docs,comma> interleaves them (A/B by the match lines' times).
- buildcli.sh <name>: release noshape-cli -> ~/agents/skasm-keep/<name>.
- Profiling: NOSHAPE_KERNEL_PROFILE=cpu, NOSHAPE_FS_PROFILE=1, NOSHAPE_RECORD_BOOLEANS=<dir> (one .brep + .txt per
  distinct Boolean); Mac `sample` + sample_under.py <sample> <root symbol>.
- Fuzz: fz/cmpprobe.sh <set> <case> (initial / ours / live); probe.sh runs a probes/zz_*.rs test copied into
  crates/ops/tests (zz_sketch_speed, zz_skdump, zz_sksolve; never committed).
- stepadj.py: coplanar adjacent planes in a STEP file. cfg.sh: one assembly studio alone in the model suite
  (archive: "config-assembly's sheet metal studio 08590a").
- Parity translator changes asm made for the assembly corpus: archive section "Parity translator changes by asm".
