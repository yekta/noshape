# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-18 08:18 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## Landed 11-18
- 9c015658 (03:39): Rip -> Bend unfold failure, Corner / Bend relief boxes, offset zoom step, " radian", snap trace.
  Its 2f93c018c clash check flagged rolled-wall models (no flat): shipped in v0.4.58, fixed in 7b8bb249 (must-carry).
- 4b4e4915 sticky snap; circle / ellipse onto a held point keeps its size. 8b481251 an opened sketch's draft takes
  the solve (hat15b). cafe3c2a tangent on a circle. 4ca852b6 exact model vertex snap. 7b8bb249 rolled walls, unfold
  failure listed as failed, sticky target only against its rank, elliptical arc on its axis.
- b5c4ccc4: Corner / Bend relief take a rip side face as its edge (live sm-rip-1118 (b)); translator decal image.
- cfb07d1c: pierce_point finds where a curve only touches the plane.

## In the worktree (to land together)
- ccbc09bc9: test only: a free line's new length moves both ends equally, label kept (live sellat-1118; ours already
  did it through scale_for_edit about the box centre).
- 51d20f639 parity.rs: an EDGE ref whose part isn't known and that has live's length tries the sketch curve with live's
  midpoint / length first (`r.measure.is_some()`: the assembly mate-connector refs have a point only, which broke
  an_assembly / a_subassembly_is_rebuilt_as_lives; refs.jsonl never fills `body`); `anchors_off` counts a PIERCE
  edge's middle. parity:: 113/113; loft-paths-guides keeps Sketch 4 / 7 / 10's PIERCEs (16/29 unchanged: Loft 3 first).
- qa SELFEEL-DBLCLICK-FACE-SKETCH on 850ae7f3 (TICKETS 8f99bc5f7): the through-drawn sketch was grey because
  sketch_display gave every sketch not edited the Inactive state; now its solve's states (blue under-defined), the
  cache no longer keyed on editing; its points ON_TOP as well. Model test a_sketch_not_edited_carries_its_constraint_states.

- live2 sktools-1118 / sk-feel2-1118: Trim reach 6.5 px (`in_sketch_reach`) and tan #D6BD95 (`TRIM_PIECE`); glyph hover
  margin 4 px (`GLYPH_MARGIN`, lit to 14 px), 6 px once lit (live's 16 px slide hold); fillet drag and the Offset drag across
  the curve already live's; a new dimension's label has no snap (test). Rows filled in interaction-feel.

## config-parts (07:40 run, ut-0800.log): links 35/35, linear-rail 26/26, pulleys 22/22, sprockets 24/24, standoffs
32/32 match to the end; nuts 14/17 (Sweep 1 non-manifold: core); screws not run (bool's > 14 GB Boolean).
Our solver keeps links' waist arcs on live's branch (probe /root/skasm-scratch/probes/links-waist-probe.patch).

## Waiting on live (REQUESTS)
- 11:12 c2-surf-intro-mutualtrim "Title" (feature 4, its first difference): the text sketch is on face JHG (y = 0,
  normal +y); our face frame (origin 0, x -X, y +Z) leaves the text off the face (12 loose parts; live 1). On the -y
  faces (fill, ruled) ours matches live. Asked for live's frame of that sketch; its baseline COINCIDENT to JHV is
  untranslated (no text point of ours).
- 06:2x wave-washer sketches.json (Sketch 4's rectangles stored inconsistently; ours solves them outward).
- 06:4x surf-intro Derived 1 sources (5 docs stop at Derived 1 NOT BUILT).

## Next, in order
0. USER P0 feel: rows of docs/parity/interaction-feel.md in my areas are "=" now; Hover timing is psui's.
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

## Agents (lead's roster 11-17 10:30; only these ids)
psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (since 11-17 ~10:35), core e8af89fe-d460-49b3-b0bb-9572da230fed (11-18 restart), blend
0672fa20-964d-4fde-a19c-c5b4363729ba (11-18 restart), bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4, dx f93a1292-5bb7-406f-9a25-59767e01878e,
qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm (me)
18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (moved 11-17 13:3x).

## Waiting on others
- psui: told 20:3x the tab row is a pick difference (live hit Flange 1's wall).
- dx: cf-spur-gear's per-host Sketch 2 failure = the near-singular sketch 62893b1ed takes back (told 21:3x).
- Not mine (handed): knife / impeller geometry (core); fan-blade Boolean 2, gamma / wire-clip Boolean 2, shaped-flange
  Booleans, surface-pattern sphere cuts (bool); setback fillet (blend); hex-bookcase mesh Uses (dx).

## Open questions (none blocking)
- Sheet metal model arrow (live2 sm-feel2-1118: Thicken plate, white arrow at the plate's middle): std puts it on the first
  face of qCreatedBy(id, BODY); ours hides the definition so that is the solid, whose first face (OCCT fuse order) is a
  side face. A fix orders the built solid's faces (offset faces first, as Parasolid's) or lets qCreatedBy see the
  definition sheet: either touches std's other nth-element queries; needs a suite run.
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
