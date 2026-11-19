# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-19 08:27 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## Landed 11-18 (since the morning)
- b5c4ccc4 Corner / Bend relief side face; decal image. cfb07d1c pierce at a touching circle. 52c050e1 / e9e27edc:
  double-clicked sketch in its constraint colours, dots on top; Trim reach 6.5 px + tan; glyph hover margin 4 px /
  hold 6 px; length-edit test; sk-feel2 / sm-feel2 rows. 485943a9 CONCENTRIC anchors (manualblend Sketch 5).
  7d0eb11c anchors on a used circle's centre (rocker-body Sketch 6). a98d2543 double-click counts the OS click count
  (lead P0, Windows CI 44275458892; unconfirmed until the next Windows run). Ctrl+A row + test; rows filled.

- efd27713 sketch patterns linked first (rocker-enclosure-body 28/28); EDGE -> sketch-curve fallback only for a
  PIERCE's edge (parting-line back to 10/17; bool told).

## Landed 17:09 (270e489b)
Search highlight in the sketch toolbar; hover reach outside sketch mode; SM flip arrow (`CtxState::sm_built`); Title
text (carry_text_box); MIRROR of two lines = lines mirrored, ends free (ops "mirror"); query text kept for a named
sketch entity. Suite 1515 (vibe2): 90/126, 1322/1585 (vibe2's cf-sheet-metal-overcut-dogbone 2/10 is its model copy:
the custom feature doesn't load there; 10/10 on vibex8).

## Landed 18:28 (f1a1c756)
Alignment wakes on the first snap onto a point (no `ACQUIRE_REST`), a drag aligns with points it snapped onto;
Symmetric on two lines = one lines-only mirror (second line onto the first's mirror image). ≠ left: a midpoint's /
quadrant's snap wakes no alignment source (ours aligns with points only).

## Landed / queued 11-18 evening
- Landed: cce30a1a (welding Sketch 8 warns, 22/37), f6ea6d5c (Modify joint reuse: `sheetmetal::bend_joint_edit`),
  a535927a (hover outside sketch mode: curve 7.5 px / hold 10.5 `CURVE_HOLD`; a hovered point's 10 px disc in
  `midpoint_overlay`; psui narrows its midpoint rule), 303e9d91 (Symmetric: B's start at its foot; Trim near a chain's
  corner `trim_near` 1.5 px; a passed midpoint's alignment `woken_mids` / `link_woken_mids`). live rechecks on green.

- 7a8ca3b2 (23:38): error sweep P1 (live errors-1118): "Sketch could not be solved." follows the edited sketch's
  solve (`update_conflicts`, also after background regens); conflicts red whatever Show errors says; a
  Dimension-tool dimension placed driven takes a typed value as driving (`apply_dimension`); regeneration warns and
  keeps the last shape on any unsolved report. Solver `redundant_nearby` (singular redundancy kept: welding 9c7704
  Sketch 13); translator: Use'd B-spline ends by first pole (asm-suspension 805e29 Sketch 2), line-across-circle
  minimum distance r - d (`h.internal`, 995636 Sketch 4), `sketch_draw` `apart` + `loose_ends` (995636 Sketch 13);
  the Origin selectable in a sketch (qa session-1118). Suite A/B: identical (90/126, 1325/1585).
- Landed 65fd9adb (00:14): shown point reach 8.5 px (qa: lit at 10, live 9) + stale feel rows.
- asm-suspension still: Fillet 3 / 4, Body / Joint / Lower Control Arm offsets: our edges B-splines where live's are
  r40 circles (core's; asm routes). asm-tvc-mount ring: not cce30a1a (A/B identical); asm bisects.

## Closed 11-19: big-sketch feel (no row >= 100 ms and >= 2x live; live: open 2.6-25 s, edit 2.7-13 s, press 31-56 ms;
ours: press 3-25 ms, drag 4-15 ms/step, edit 14-20 ms). Probe: /root/skasm-scratch/probes/zz_sketch_speed.rs (REDRAW too).
Sketch keys (shortcuts-sk-1118, 6 rows) landed a4b5d946; live confirmed all six on green.

## State 08:27 (11-19)
- Landed today: 4104b148, 24792169, dceaee62 (flat view by the model's up; qa verified af11b6d5), cd1acebd (394e7c79:
  wave-washer Sketch 4 = live via solver `carried_onto_anchors` + translator anchors-last, clamp-guillem pin order,
  crossing line -> Vertical, bend face click -> Bends row + flat zone `sm_follow_pick`). Suite F 91/126, 1335/1585.
- CHECKING 2dbf7d61: conflict colours (live recheck: 50 typed opposite a 40 reds both, not the 30; solver
  `conflict_dims`: driving dims the sketch solves without join the conflicting set; fuzz write_batch needs the union).
- QUEUED 32f60584: lead's missing-functionality #1: flat DXF/DWG outline exact (`FlatCurve`, `outline_curves`, field
  `FlatPattern::outline`, serde default; empty -> loops as before); e2e `the_flat_dxf_writes_arcs_and_circles`.
- Lead's top 5 (missing functionality, sent to the lead 11-19): 1 flat DXF exact curves (queued); 2 flat export "Include visible
  sketches" (live default on) + "Splines as polylines" + sheetmetal_export_flat options (live capturing files:
  live/skasm/q-flat-dxf-1119/, sample 100x60 R5 corner, holes, slot, flange, hole across the bend); 3 pattern / mirror
  of active sheet metal with Form / SM hole (host sheetmetal.rs "tools of sheet metal holes / forms are not supported
  yet"); 4 extrude on the flat (opSMFlatOperation, native_args.rs); 5 corner break next to bends (sm_flat.rs:97).
- Open (low: geometry matches): infento Part grid's four 80 mm LENGTHs (seed side V2HoieiIDMV9, the pattern's
  direction lines len1 / len2, one more) end driven in ours (E, F and 07:2x builds alike; TRACE "set driven"): live's
  grid instances are tied corner to corner by 401 PATTERN COINCIDENTs, so the spacings follow from the seed's size and
  our solver calls the LENGTHs redundant; live keeps them driving and solves 80 -> 80.8. Needs live: what moves (seed,
  spacings, instances) when the 80 is edited. Data: vibe2 ~/tmp/skasm/inf1 (rebuilt 07:1x, Part grid only), pg.sh.
- welding Sketch 12: ours = live's geometry; its 2 PROJECTED are live's split Use (`.orphan` USE_END + MIDPOINT);
  translator could Use + split + point Use of the far vertex (low value).
- Waiting on live: rocker-enclosure sketches.json (REQUESTS 02:20); rechecks of sketch keys, errors 82-85.

## Form speed P0 (closed 17:4x): no regression; live2's app A/B overlaps (A 2.3–4.0 s, B 2.9–3.4 s, by load); model
regen no slower (release, debug). Binaries in /root/skasm-scratch/apps (delete when done).

## Next
- Midpoint alignment landed (303e9d91); live has no circle quadrant inference, nor do we: nothing left there.
- Handed: surf-intro-simple Loft 1 (core), surf-intro-examples Fill 1 overshoot (lead to route), welding-config
  Extrude 8 (core, fixing), manualblend Sweep 2 area / Move boundary (core).
- Form speed P0 binaries /root/skasm-scratch/apps: delete (done).

## Waiting on live (REQUESTS)
- Answered 11-18 13:xx: wave-washer sketches.json (Sketch 4 / 3 solved inward) and surf-intro Derived 1 sources
  (c2-surf-intro-*/derived/<id>/, live.jsonl by rollback): rerun wave-washer and the surf-intro docs.

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
psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (since 11-17 ~10:35), core d30002ca-d013-4435-b7d9-7da9cb564cec (11-18 new id),
blend de66e481-56e2-4005-a5d3-9fa897ca5fde (11-18 new id), bool 754a9804-feb1-4153-b32a-acf9b2a6d94d (11-19), dx f93a1292-5bb7-406f-9a25-59767e01878e,
qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm (me)
18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (moved 11-17 13:3x).

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
