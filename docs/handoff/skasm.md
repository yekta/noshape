# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-20 12:08 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## In flight (11-20 12:08 UTC)
- 5 new SM suite models (lead 11-20), before -> after: forms 2 -> 10/21, hem-flange 2 -> 7/8, jog-panel 0 -> 5/12,
  modify-joint 4/11, loft-basic 3/4 (also c2-cov-sm-corner 3 -> 6/6). c41f27d3: translator (Form's form-library studio ->
  `library:<form>` + config; SM wall CAP_FACE of `wallThicken` = start: off the line's sheet; sketch on its own in-feature
  MC -> plane on live's frame, P2 SKETCH-ON-IN-FEATURE-MC; CORRESPONDING_IN_FLAT sketch plane -> our flat face), harness
  (live's in-context "Reference N" wires no parts), sheetmetal.rs (a bend end square to one wall: no ruled end).
  Next commit: sheetmetal.rs `exact_bend_edges` (B-spline bend arcs / straight seams after the fuse rebuilt as circles /
  lines, rolled strips too; test a_jog_whose_bends_meet_keeps_exact_edges fails without it); 24 SM docs A/B 195 -> 201.
- Next differences: forms Sketch 3 frame (P2 ticket), Form 5/7 (circle-emboss / circle-knockout tools); hem Hem 1 status
  INFO "Collision in sheet metal flat pattern." vs live WARNING (message null; live's SM model 1 is WARNING too = cannot
  unfold; ours unfolds Hem 1 with a collision); jog Jog 3 ERROR "The bend line did not divide the sheet metal into at
  least two parts." vs live INFO; joint: our flat lies elsewhere (Sketch 1 on flat: frame (0, -113.5) vs live (120.02,
  0)), Extrude 2 "No faces meet flattened sheet metal bodies"; loft: live 24 plane + 8 B-spline faces, ours 30 + 2.
- modify-joint: which wall a THICKEN-of-faces model keeps fixed in its flat (live keeps the top, JHO, region 3 of 3;
  ours another, flat turned 90°): REQUESTS 'skasm 2026-11-20 11:40' to live; fix flat_place after the answer.
- c41f27d3 LANDED as b0475a22 (12:12); fd7b5d66 (exact_bend_edges) queued 12:15.
- Tools: /root/skasm-scratch/pm.sh (ONLY=, SAVE=, build + run), pmrun.sh (TRACE run of the kept binary), smlist.txt
  (24 SM docs), CLI batch on saved docs: ~/tmp/skasm/saved (copy .noshape + -wal), hemq*.sh / jq.sh / jogq.sh.
- Feel stack 4e51cce3 verified by qa (green f380b6928); P2 drawn_failed LANDED 8fd60c7a.
- cf-sheet-metal-model-plus P1 line (SUITE-REGRESSIONS-1108) closed: 318 ms vs live 917.

## Closed 11-19: big-sketch feel (no row >= 100 ms and >= 2x live; live: open 2.6-25 s, edit 2.7-13 s, press 31-56 ms;
ours: press 3-25 ms, drag 4-15 ms/step, edit 14-20 ms). Probe: /root/skasm-scratch/probes/zz_sketch_speed.rs (REDRAW too).
Sketch keys (shortcuts-sk-1118, 6 rows) landed a4b5d946; live confirmed all six on green.

## Next
- 850efe Extrude 6: bool's (P1 for them, 3b6725b2). Extrude 1-5 match live; Extrude 6's union leaves its r4 tool cylinders and
  Extrude 2's coaxial r4 ones unmerged (ours 33, OCCT 31 with a bad merge, live 28). Axes exact to 1e-12 mm: not the sketch.
- Midpoint alignment landed (303e9d91); live has no circle quadrant inference, nor do we: nothing left there.
- Handed: surf-intro-simple Loft 1 (core), surf-intro-examples Fill 1 overshoot (lead to route), welding-config
  Extrude 8 (core, fixing), manualblend Sweep 2 area / Move boundary (core).
- Form speed P0 binaries /root/skasm-scratch/apps: delete (done).

## Waiting on live (REQUESTS)
- Answered 11-18 13:xx: wave-washer sketches.json (Sketch 4 / 3 solved inward) and surf-intro Derived 1 sources
  (c2-surf-intro-*/derived/<id>/, live.jsonl by rollback): rerun wave-washer and the surf-intro docs.

## Next, in order
00. lead 11-20: 5 new SM suite models (forms, hem, jog, joint, SM loft): first difference each, before/after counts.
00b. qa P2 SUITE-INTRO-TAILFIN-INTERSECTED: "Profile for tail fin" 1 INTERSECTED not found (surf-intro boundary/fill/
   mutualtrim/ruled): Surface 2 area, Curve 1 start, Boundary surface 1 fails; qa-scratch/feel1120/car-one.log.
00c. dx 11-20 (suite speed, not urgent): c2-sm-rocker-enclosure-body › Mirror 3 160 ms, live 10 ms (doc under the bar).
00d. qa P2 SUITE-NEW-INDEX-1120-SKASM (TICKETS 7066344c1, on green 4e51cce3, before c41f27d3 / 566a4bc6): Form 1, jog
   Sketch 1, Corner 1, hem SM model 1 gone on my branch. Open: c2-cov-sm-bend-relief Bend relief 1 volume +3.9 mm3;
   c2-cov-sm-rip Rip 1 48/106/60 vs 46/100/56; c2-cov-sm-unfold-refold Unfold 1 56/128/74 vs 46/100/56; loft; joint.
0. USER P0 feel: rows of docs/parity/interaction-feel.md in my areas are "=" now; Hover timing is psui's.
0b. live's camera retakes (025a33f6): sketch-line-from-midpoint, sketch-bezier-polygon, sketch-spline-handle-arm and 7 asm
   takes now compare the camera; rerun them after the land. asm-menu-instance-properties' Pro menu (Where used…, no
   Release…) is already asm's on main; "Create task…" is CLOUD_ONLY in run.py.
1. loft-paths-guides now lists only Sketch 2's take-back and Sketch 4's 3 PROJECTED (Sketch 5's PIERCEs on its
   Intersection's ends are skipped as the tool's own). The other PIERCE take-backs (09:20, sec.log) all sit behind an
   upstream difference: surf-intro tail fins (Intersection face from the unbuilt Derived 1), manualblend Sketch 6 / 7
   (sections on Sweep 2, wrong after Sketch 5's CONCENTRIC), pipe (core's Loft first), knifehandle (Sweep 2 first).
   Unfound PROJECTED (45): mostly qCompressed queries with no ids (hunting-knife Sketch
   12, no feature lost); rocker-enclosure's wait on core's split-face fix.
2. Suite 11-18 05:45 vibex8 (~/agents/skasm-keep/suite-0545.log): 88/126 docs, 1292/1585; untranslated sketch
   constraints 121 (45 PROJECTED model geometry not found, 24 PIERCE rejected: loft-paths-guides, surf-intro tail fin,
   manualblend Sketch 6/7). surf-intro-examples Fill 1 (area 2.6x live) and -simple Extrude 1, -mutualtrim Title next.
3. core's sketch cases below. Rocker-enclosure Sketch 4's projected circles: bodies RXBD.. outside the part box
   (x 45-60 mm), waiting on core's split-face fix.

## core's sketch cases (2026-11-16), queue item 3
- c2-sm-rocker-enclosure-sm Extrude 2 (now its first difference once core's split-face attributes fix lands, 8/14):
  Sketch 4's four circles are PROJECTED (Use) constraints whose live external edges (refs.jsonl
  AjhQtHt4MbMf.{0..3}.project: x 45–60 mm, y ±78, z −91 / −21) aren't on our part. Our sketch comes out with two
  circles (both rows map onto one), so Extrude 2 names one region twice ([0, 19] twice) and cuts 2 holes of live's 4
  (ours 26/52/24, live 28/56/24). Probe: `NOSHAPE_PROBE_SKETCH="Sketch 4" NOSHAPE_PROBE_PARAM=entities` in
  parity::bodies_after_feature_probe (core's probe patch).
- c2-loft-guide-curves-wave-washer Loft 1 ("Could not create loft", 6/7): live says JHk / JH4 are Sketch 2's points
  (POINT bodies JHb / JHv at (0, ±25.4, 3.81) mm); our Sketch 4 now coincides with them but solves its rectangles
  outward (live's stored geometry is inconsistent): waiting on live's sketches.json (REQUESTS 06:2x).
- c2-surf-workshop-manualblend Sweep 2 (11/19): Sketch 5's CONCENTRIC to vertex JNE now kept (anchors_off counts a
  CONCENTRIC in the plane, within 10 µm when the stored was 0.1 mm+ off); our sweep now lies where live's does (x
  −60.9…−26.3), but it takes the path's name "Curve 2" and the wire becomes "Surface 3": Sweep's body identity (core).
- c2-sm-rocker-enclosure-body Extrude 6 (15/28): Sketch 6 now kept (its hexagon's pattern centre COINCIDENT with
  the Use of JRR's centre, which live's refs `center` has; the stored was 2.8 mm off). Extrude 6's counts now match
  (39/97/58); volume 379696 vs live 379830 mm³ (live cuts only the hexagon pocket, 66.81 mm² x 5 = 334.07 mm³; ours
  134 more = 26.8 mm² x 5). Extrude 6's ids JtC / JtG map by text (ids fail): our regions 110.97 + 22.94 mm², live
  84.16 + 22.94. Our first region runs past the hexagon (x -86.01 vs -84.04, y to 43.01 vs 41.04) to the pocket
  floor's imprinted corner arcs (boundary entities >= 2^30): how regions meet imprinted face edges differs.
  Sketch 5's take-back is right: its circles are stored on live's centres, our Use of JZB lands 6.43 mm off (our
  model's edge: core's split-face fix).
- Settled 11-18: config-parts links / pulleys / linear-rail / standoffs / sprockets match to the end; cpcurves has no
  sketch difference left (first: 3D fit spline 1, "Cannot evaluate tangency for end condition.").

## Agents
Thread ids only in /root/lead-scratch/roster.md (lead 11-19 23:00): read it before every message_agent; never copy ids.

## Waiting on others
- psui: told 20:3x the tab row is a pick difference (live hit Flange 1's wall).
- dx: cf-spur-gear's per-host Sketch 2 failure = the near-singular sketch 62893b1ed takes back (told 21:3x).
- Not mine (handed): knife / impeller geometry (core); fan-blade Boolean 2, gamma / wire-clip Boolean 2, shaped-flange
  Booleans, surface-pattern sphere cuts (bool); setback fillet (blend); hex-bookcase mesh Uses (dx).

## Open questions (none blocking)
- hat15b Sketch 1 (inserted in the doc's last microversion 1229, 11-17 ~17:30): its used point 19 (Vertex ref) was
  stored at (33.679, 45.182) mm but resolves to the vertex at x 34.75 in 0.4.43…main; nothing of the model lies at the
  stored x in any build. Not upstream geometry (not core's). Not reproduced: today's snap there stores 34.749999642 (the
  mesh's f32), now the vertex's exact point (`a_model_vertex_snaps_to_its_exact_point`); no edge or vertex of any part
  lies at the stored x. 8b481251 makes the editor take the solve on opening, so the doc edits right.
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
