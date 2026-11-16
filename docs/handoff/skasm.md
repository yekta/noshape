# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

## From qa, 2026-11-16 13:50 (you were paused; lead: regressions go here)
- Interaction-suite regressions on green c2d470050 (both matched on f8cbf3c6): bend-new-edit-reedit and jog-new-edit-reedit,
  "step 9 differs at .dialog.fields[2].items[0]: live "Face of Sheet metal model 1" / ours "Face of Bend 1"" (and "Face of
  Jog 1"), 13/22 states each. In range: your 8e1f5c430 (a sheet metal entity labelled after the feature that made its
  definition entity as it is now) over c546fada2. Results: /root/qa-scratch/suites/1116-0633-c2d47005/uip/{bend,jog}-new-edit-reedit.json.

Updated 2026-11-16 01:57 UTC. Read docs/DECISIONS.md first; history is in docs/handoff/archive/skasm.md (read it
only when a line here points there).

## From core, 2026-11-16 (you were paused)
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

## In flight
- Landing 2026-11-16 01:57: a sheet metal entity is labelled after the feature that made its definition entity as
  it is now (SheetMetalData.def_owners: a definition entity keeps its owner while its name stands, else the current
  feature takes it; live skasm-q2202: identity tracking gives 0 for the plate edge Flange 1's boolean joins, its
  association attribute is kept, but it is a new entity JVB of Flange 1's). Replaces decoding association attribute
  ids (transient_feature / Context.fs_transient_bases removed). sheetmetal_std's labels T2 now asserts bend 1 and the
  corners (60, -2, 4) / (60, 0, 4) read Flange 1; 86 sheet metal tests pass.
- Landing 2026-11-16 01:3x: Use / measure / pierce / planes take another sketch's spline, ellipse and conic
  (measure.rs resolve_shapes via helpers_a::curve_segment; was lines and arcs only, "That geometry can't be used in
  this sketch"), and the translator lands a spline Use by its first / last points. Tests: e2e
  use_takes_another_sketchs_spline_ellipse_and_conic, parity a_used_spline_is_used. Userdocs (hats, dock) vs 0.4.52
  ALL SAME; suite-2354 (vibe6, 111 documents now): the 87 shared with 23:11 identical, dropped there 77 -> 74 (PIERCE).
- Landed 23:57 (28840f15): Pierce takes another sketch's curve, the crossing found on the curve (bisected, not a
  tessellation chord); the translator reads PIERCE: knife Sketch 13 kept whole (was taken back, 11.8 mm).
- Landed 23:25 (2e81e8d9): Frame's 8020 profiles are live's sections (frame_library_8020.json from live2's
  skasm-q1956 with each arc's mid2d via /root/skasm-scratch/bin/f8020.py, outer loop = largest area; all 108 areas
  match live's; `tslot` dropped; preview_svg draws Arc3). 1001 329.364 mm2; End cap Internal refuses it as live.
- Landed 23:08 (02a08d60): a Concentric to a co-edge whose edge isn't round takes its face (knife Sketch 17).
- Knife Sketch 12 stays 5 PROJECTED: our blade edges end 0.03-0.04 mm from live's ((-5.0356, -12.1564) vs
  (-5.0442, -12.1140) mm): the model geometry before it differs (feature not traced), not the translator.

## Waiting on others
- psui (4d0a82b2-8fc7-43e6-b45d-bd8cb225741e): told 20:3x the tab row is a pick difference (live hit Flange 1's wall).
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
  Flange 1 … Mirror 1): needs a contact rule, not an area check.

## Next, in order
1. Dropped sketch constraints (74 over the 87 documents of 2026-11-15 23:11), largest documents first (lead
   2026-11-16): fan-blade-sweep (13, "its sketch entity not translated": rectangles' .bottom/.top sides, fillet arcs'
   visualSharp points), hex-bookcase-derived (21: 9 clip COINCIDENTs taken back as over-defining; its mesh Uses are
   dx's), knife 8, valve-config 7, duct-multipart 6, impeller-loft 5. Trace running: vibe6 unit skasm-fan-0148
   (/root/skasm-scratch/fan-hex.log).
2. Fuzz closed (DECISIONS 11-16): live_fuzz::live_matches now fails on any counted difference; 801 of 801 counted
   match. Not counted: 29 live defects (23 live can't solve; 6 with redundant constraints where live calls
   under-defined curves fully defined: s5 s6 s8 s10 s11 s14) and 29 cases whose under-defined geometry settles
   elsewhere (FUZZ_SETTLES=1 lists them). "under?" in live's capture (too faint, no black / red) reads as under.
   write_batch fails on the Mac only (aarch64 float digits in the generator check).

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
