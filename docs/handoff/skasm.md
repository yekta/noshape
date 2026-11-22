# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-22 12:53 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## In flight (11-22 12:53 UTC): re-landing c572df92's content + the SKETCH-LENGTH-LABEL residual (df6391b5a)
- c572df92 FAILED 12:28 (rebase conflict, renderer.rs / shaders.wgsl): asm took params.x bit 4 for on_top lines
  (80a580be3). Rebased by hand: unlit_only moved to bit 7 (128); over_lines stays bit 5; psui's on_top silhouettes
  (64, not on main yet) untouched. 53a54598 (47bfb052, tablet-stand content) still QUEUED; my next land carries all.
- Branch = 9 commits over main: sk/* hover combined states, SMFlatOp per live smflat-1122, BEND-FACE-RECLICK-EMPTIES,
  tablet-stand names, flat orientation, and df6391b5a (residual: a level/vertical/perp direction crossing a model edge
  beats its MIDPOINT; a drawn line's length box lies left of its direction, verticals left; tests
  a_level_line_onto_a_model_edges_midpoint_ends_where_the_level_meets_it, a_drawn_lines_box_lies_left_of_its_direction).
  Before the rebase: clippy + 416/416 sketch::ui_tests green (12:50).
- Run on the rebased tree: skasm-scratch/bin/rbv.sh on vibex6 (clippy render/scene/psui/custom, render tests,
  scene sketch_mode, sketch::ui_tests, e2e an_extrude_in_the_flat*), log /root/skasm-scratch/rbv-run.log. Green: land.
- TICKETS rebuilt vs main (SKETCH-LENGTH residual note on row ~318; other duplicate IDs are on main itself, qa's).
- Asked live (REQUESTS 12:32 "length box side and model-edge midpoint vs a crossing"); adjust to the answer.
- psui FYI (lead 12:5x): a_closed_sketchs_lit_end_goes_with_it broke from my Esc hold 438a9d313 (hover pick only on
  id change); psui's f8b0a2d8 (queued) fixes it. Nothing for me.

## P1 TABLET-STAND-FACE-NAMES: DONE 11-22 (acf660cb)
- Cause (not the fuse): a rolled bend's planar end caps with exact arcs get tighter boxes (x 67.999994 vs the B-spline
  arcs' 68.066990), so their geometric `Role::New` key fell before the bend cylinder's (68.000000): New 11/12/13 swapped.
- Fix: rolled_bend also thickens plain and keys each cap by its plain twin's box (NameKey); fresh_part_names uses it.
  Saved tablet-stand copy: all Ok, names identical to the plain build's (probe /root/skasm-scratch/probes/zz_smnames.rs,
  bin/smn.sh). T2 sheetmetal_std a_rolled_walls_faces_keep_their_names_with_exact_arcs.

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

## qa tickets waiting
- None open for me: all marked fixed / to verify in TICKETS (11-22 10:2x). Next: whatever qa / the lead files; else the
  "Next, in order" list below (glyph row off taken spots, hem-jog counts are optional).

## Next, in order
00. lead 11-20 SM suite models / qa 00d: all match but c2-sm-loft-basic (apex, above). Mac 12:30: 22/23, 259/260.
00b. qa P2 SUITE-INTRO-TAILFIN-INTERSECTED: LANDED 37638c76: an Intersection lands within 1 µm of live's ends (ours 0.19 µm
   off): no INTERSECTED left / no take-back in surf-intro boundary/fill/mutualtrim/ruled (A/B over the 7 INTERSECTED
   docs: nothing else changed). Left: Tail fin surface area 1.2e-5 off (3567.145 vs 3567.188 mm², ~0.6 µm of profile
   length, cause not found) and 3D fit spline 1's shape (starts x 32.70, live 33.94, same ends and start direction):
   core's (fit spline), told via the lead; Boundary surface 1 fails on it.
00g. A/B 11-21 18:50 (core's asks): asm-scissor-lift (asm P0) and c2-surf-intro-mutualtrim, parity.rs only.
   Studio 0139f2 Extrude 8: region_at tied Sketch 1's regions with Sketch 6's projected ones (same plane, point, area);
   by_geometry now takes the sketch the region query names. b582441a Extrude 1 KICB (as a Derived source, no refs):
   region_from_query's exact-boundary rule took a sliver beside one of three named pieces of the slot line; a region
   must have as many pieces of each named curve. mutualtrim: SWEPT_* of a Ruled surface (`<f>.….patch`) = its part,
   its face on the curve, its edge from the curve's start / end (ruled_piece_from_query), not the sketch edge.
   Plus curved rips with no pull (corner-break model F). A/B on vibe2: skasm-scratch/ab6.sh (~/tmp/skasm/scm/sc-<doc>:
   studio copies + refs.jsonl, mode 700, delete after). Default builds on vibe2; the Mac only via its one 4-job nice-19 slot (DECISIONS a16b39297).
0. USER P0 feel: rows of docs/parity/interaction-feel.md in my areas are "=" now; Hover timing is psui's.
0b. DONE 11-21 13:35: sketch-line-from-midpoint / sketch-bezier-polygon / sketch-spline-handle-arm match with the camera
   (3/3, 40/40 states, green 6241cf347 on vibe2; skasm-scratch/bin/uipg.sh); the 7 asm takes are asm's.
1. LANDING 11-21: a DISTANCE to a model circle in a sketch without live's frame takes our frame (loft-paths-guides
   Sketch 2 solved, Mac suite otherwise identical). loft-paths-guides left: Sketch 4's 3 PROJECTED; Sketch 6's take-back
   (0.5 in to JUV, Extrude 1's seam line at x -50.8, ours elsewhere: 390 mm move; older than this). (Sketch 5's PIERCEs on its
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
