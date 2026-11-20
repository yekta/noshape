# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-20 00:50 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## In flight: stack QUEUED e0837465 (suite O = J on every document: 95/126, 1354/1585); P0 dim-place fix landing on top
Compare per document with suite J (vibex7 ~/tmp/skasm/suite-J-1119-1831.log.s.c: `sed` the "name: N/M features match"
lines to "name N/M", LC_ALL=C sort, join, awk $2!=$3). None worse -> land.sh. The stack:
- Extrude part order (3ff5480f): a New extrude's regions of one sketch go in drawing order (the lowest entity id of each
  region's outer loop; extrude.rs `in_drawing_order`), whatever the pick order. Live q-extrude-part-order-1119 (live2,
  REST-built sketches): A/B/D/E parts in drawing order, both pick orders; C disc + ring touching = one part. Pinned by
  regen `a_new_extrudes_parts_follow_the_sketchs_drawing_order`. Suite L (this alone) = J on every document. welding
  9c7704 is asm-welding-config (an assembly): asm checks the BOM after the land.
- Relief cut creators (fc917dc3): live q-sm-creators-1119 B1: two Bend reliefs -> the second creates the cut faces at
  both ends; ours kept Flange 1's names (keep_names on the old relief planes; predates 643d9675). Now `Piece::relief`
  (bend relief slots, corner reliefs); a face on a relief piece and on no non-relief vertex piece (a bend's end can
  share the cut's plane) takes this rebuild's fresh name, never renamed after its source. Pin
  `the_latest_bend_relief_creates_every_reliefs_cut_faces`; SM std T2 66/66, frames 24/24, ops e2e 259, SM matrix,
  clippy clean.
- Feel tests: drawing on a face (`a_rectangle_drawn_on_a_face_is_solved_in_the_closing_clicks_frame`), Insert image's
  dialog + aspect (sketch tools matrix row), session 2d iso dimension placement; interaction-feel rows updated.
- A-rect corner (40c9ed20): sm-box's automatic miter leaves the bends a thickness-long plate stub apart; evCornerType
  took the lone bend end as NOT_A_CORNER ("Sheet metal corner not found."). Now OPEN_CORNER with the stub's far end;
  a Rectangle Corner override there cuts both bends' corner (corner_reliefs `mate`, the far bend clipped from its own
  end); a moved bend end beside its kept bend takes the definition creator (Flange 1 / 2). Live's 28 / 62 / 36 and 8
  creator pins: `a_corner_reliefs_cut_faces_are_the_corner_features`. Only with a Corner override (live's sm-box has
  no default cut there: 23 faces both); whether a model default Rectangle cuts a mitred corner: not captured.
- Offset distance (81be4f10, qa P1 OFFSET-EDIT-SCALES-SKETCH): an OffsetPair's Distance is no first dimension; its
  edit moves the sources, copies stay (live psb 10-24 j (c)). scene `an_offsets_new_distance_moves_the_source_...`.
- Search tools Alt+C row test (e9643735, sketch tools matrix).
- Closed-box SM extrude (996d8438, qa P2 SMM-EXTRUDE-CLOSED-PROFILE): the 4 lines already built live's box (32 faces,
  12843.115 mm³, red "cannot be unfolded"); qa's failure was the whole sketch picked ("Edges of Sketch 1" -> host got
  the sketch's bodies, sheet included). custom run.rs: a Reference::Sketch in a curve-only field = its curves.
  Pin `an_extruded_closed_rectangle_is_a_box_that_cannot_unfold` (both picks). Not in suite N (no parity doc hits it).
  Left: sketch points passed through ops into sketchCurves say "Cannot compute tangent line." (the app's filter blocks them).
- Slot on an arc (44c3ffed, qa P1 SKETCH-SLOT-ON-ARC): slot_chains took the arc's offset sides as caps (they share its
  centre) -> Equal to the Ø cap -> "Ø0" and Esc committed the wreck. Sketch + scene tests; slot tool only (no suite doc).
- Sketch / face DXF (qa P1 SKETCH-DXF-EXPORT-POLYLINES; live rt-1119 live-sketch1-default.dxf): as live's: R14, the
  flat export's tables (flat_dxf `tables`), each curve exact through flat_dxf `put_curve` (LINE / CIRCLE / ARC / ELLIPSE
  / SPLINE by control points) on MODELSKETCH_VISIBLE (`curves_document` / `curves_dxf` / `curves_dwg`); no construction
  curves, points or external refs (sketch_export `sketch_curves`; a face: `outline_curves`, else sampled lines). SVG
  keeps the sheet path. ops e2e sketch_paste_export pins layer, kinds, radius, AC1014, construction left out.
- Dimension place click over a plane (qa P0 SKETCH-DIM-PLACE-CLICK-SELECTS, ui-parity sketch-dimension-variable): on Top
  the rectangle's bottom line lies in Front; with it picked, the place click 8 mm under it hovered Front (past the line's
  8.5 px reach since 24441238) and took it as a second pick. sketch.rs `plane_holds_lines`: a plane every picked line
  lies in is skipped (the click places). Pin `a_dimension_placed_over_the_plane_its_line_lies_in_opens_its_box`.
- Simulation connection removed (77fa48ed, lead; DECISIONS 11-16): mate dialog field + flyout row + model-data/ops
  fields, run.py drops the two labels, scenario asm-slider-simulation-connection deleted. ops e2e assembly 14, coverage,
  assembly-ui 97, clippy clean (vibex8).
- sketch-vertical-key: psui's (lead 21:2x): the start view is live's Trimetric; the 11-06 take's zoom differs. After
  cf70e676 lands psui reruns; a gap left is the reach measure, mine with psui. No re-baseline.

## Landed 11-19 evening
- 91df536c (21b41fd9): an exposed SM face takes its source's creator only beside a face that source kept
  (643d9675; sm-box vertex test); feel tests (sketch plane field "Face of Extrude 1", a new sketch changes nothing,
  Final on edit, the bend row hover's flat side). Suite K = J. 9a872f69: the 5 qa feel rows (qa verified 21:12).

## Landed 11-18 (since the morning)
- b5c4ccc4 Corner / Bend relief side face; decal image. cfb07d1c pierce at a touching circle. 52c050e1 / e9e27edc:
  double-clicked sketch in its constraint colours, dots on top; Trim reach 6.5 px + tan; glyph hover margin 4 px /
  hold 6 px; length-edit test; sk-feel2 / sm-feel2 rows. 485943a9 CONCENTRIC anchors (manualblend Sketch 5).
  7d0eb11c anchors on a used circle's centre (rocker-body Sketch 6). a98d2543 double-click counts the OS click count
  (lead P0, Windows CI 44275458892; unconfirmed until the next Windows run). Ctrl+A row + test; rows filled.

- efd27713 sketch patterns linked first (rocker-enclosure-body 28/28); EDGE -> sketch-curve fallback only for a
  PIERCE's edge (parting-line back to 10/17; bool told).

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

## State 09:42 (11-19)
- Landed today: 4104b148, 24792169, dceaee62, cd1acebd, 2dbf7d61 (conflict colours, live rechecked). live rechecked
  on green bfe09bcd: crossing row 67, conflict row 110, bend-face click row 1704 all =.
- Lead's missing-functionality list (order set by the lead): #1 / #2 export follow-ups, then #3, then #4 / #5.
  - LANDED 32f60584 (#1 flat DXF exact curves `FlatCurve` / `outline_curves`) as 1021a0c1; 0dae1e11 (ELLIPSE; #2
    "Include visible sketches" on MODELSKETCH_VISIBLE, export options) landed as debcf7d1. Live truth: live/skasm/q-flat-dxf-1119.
  - LOCAL, to land together (suite G 10:55: 92/126, 1336/1585 vs F 91 / 1335; per document only clamp-guillem 24 -> 25; G holds the first
    two; the last two are inert outside flats): a1a00fce4 bend centreline split around a hole (`FlatPattern::clip_to_flat`); 63eb8b124 #3 pattern / mirror
    of active sheet metal forms each copy (live q-sm-pattern-1119, 9638.003952 mm³ each; `body_moves`,
    `moved_form_tools` / `moved_tools`, FormTools.moved; T2 `a_pattern_or_mirror_of_sheet_metal_carries_its_form_and_hole`
    now also asserts one form per flat); 6f2318e4c a flat keeps a form beside a hole (`lone_face`: the wall's flat with
    holes cut was a compound, no plane, form skipped; T2 `a_flat_keeps_its_form_beside_a_hole`); 5bbbdc5e0 #4 step 1.
    SM T2 81/81, sm matrix 17/17, custom 16, ops e2e 252, clippy clean (10:2x).
  - #4 step 1 (5bbbdc5e0): Extrude with domain FLAT routes to std (`adds_to_active_sheet_metal`); `Context::flat_sketches`
    (sketch eval records a sketch on a flat face); host SM_FLAT_FILTER / PARTS_ATTACHED_TO / queryContainsFlattenedSheetMetal
    know flat sketch entities (`QueryCx::flat_part`); `opSMFlatOperation` (sheetmetal.rs): Remove = region ∩ each wall's
    flat face, mapped back by FlatPlace.walls' transform, cut from the definition sheet; Add = region off the flat, onto
    the touching wall, oriented as it, fused (unify). ops no longer calls always-hidden params ignored. e2e
    `an_extrude_in_the_flat_cuts_and_adds` (±200 mm³). Open for live (REQUESTS 10:31, q-flat-ops-1119): how the app
    picks domain FLAT (flat view focus? flat region?), regions across a bend, Add errors (SM_FLAT_OP_ADD_CROSSES_EDGE),
    the feature list entry; then the app side.
  - API audit (lead 11:0x) QUEUED 4d2730e3: sketch tools / options, SM table, flat export, SM features matched;
    added ui_preferences.sketch_options (Sketch dialog checkboxes, scene `save_sketch_options` pub) and sketch_draw
    `rho` (conic). AGENT_API regenerated (regenerate + scp in ONE rb.sh call: a later rb.sh sync overwrites it).
  - #4 / #5 to live2's q-flat-ops-1119 (behaviours 'skasm (11-19): extrude in the flat…' + Leftovers): LANDED d998d255 as e87a2b3e
    (flat Add: across a bend/outline SM_FLAT_OP_ADD_CROSSES_EDGE, inside a wall SHEET_METAL_REBUILD_ERROR, floating an
    INFO SM_FLAT_OP_PARTIAL_INTERSECT, part unchanged; corner break at a bend's end INVALID_INPUT in sm_flat.rs; e2e
    `a_corner_break_beside_a_bend` with live's Fillet 5 7034.757 / Chamfer 5 7020.487). QUEUED f1dd7113 (82be6b025):
    the flat Extrude form: schema hides all but flatOperationType + entities under domain FLAT; `extrude::domain_logic`
    (model-core) sets domain from the regions, run in the app (start_feature preselection, auto_operation on picks) and
    in ops feature_add when domain isn't given; UI test `extrude_on_a_flat_region_opens_the_flat_form`.
  - rocker-enclosure-sm (lead 14:3x: mine, core told): 14/14 to the end locally. LOCAL dc43d78a7 (translator
    `context_from_refs` / `self_made_edges`: in-context entities without context.jsonl from refs.jsonl's self-made
    edges; Sketch 4's 4 holes) and cf7a34648 (host: a split wall face's attributes go to its `TopoName::Split` pieces by
    name, query.rs `relocated_pieces`; ops.rs `split_off_pieces`: a subtraction's new piece of a definition sheet gets
    its body attributes + hidden + `splits` (NOT split_from: std's MODEL check then saw 2 bodies); T2
    `a_cut_across_a_sheet_metal_plate_leaves_two_parts`). SM 98/98, matrix 17/17, custom 16, clippy clean. Suite H
    (dc43 only) running on vibe2 (orphaned, ~/agents/skasm-keep/suite-H-1119-1511.log), suite I (both) queued after
    (/root/skasm-scratch/suitei.out); land both when I >= G (92/126, 1336/1585) per document.
  - Sketch fuzz 10x (lead 11:3x): 550 seeds, 0 panics / wrong solves; 800-step random webs (1300-2000 entities) solve in
    1-4 s: lead left it (not a real sketch). fuzz.rs reports SLOW steps (SKETCH_FUZZ_SLOW_MS; needs --nocapture).
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
