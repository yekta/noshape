# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## State (2026-11-17 09:25 UTC, restarted after the 11-16 pause)

### Committed on branch, NOT landed (worktree /root/projects/noshape-wt/asm, branch asm-partorigin)
- 11-18 ~11:00: the 28 code commits below rebased onto main, + 5ef547724 (BOM scorer, after the pair's binary); pair
  split vibe2/vibex8, then land. b8d2d867e held (bool's 8ba7c6dcc first; old branch asm-partorigin-old).
1. Mate connector on a mesh part's node (refgeom.rs origin_csys: MeshPoint as a vertex, world axes). corexy studios
   4a6a96 / 6d4503 / d8befc. Test features_b::a_mate_connector_on_a_mesh_node.
2. Feature pattern of a Boolean subtraction (pattern.rs boolean_tool_faces; core handed it over).
3. Nested geometric feature pattern composes its instances with ev.xform (Reapply of a pattern). Test
   features_a::feature_pattern_of_a_boolean_subtraction.
4. connector_at_frame: a flipped in-feature connector moves along -Y / -Z: corexy 21/22 -> 22/22 mates.
5. Same-named parts of ours go to the live part whose volume fits (tvc SG90 import): tvc 6/12 -> 7/12 mates.
6. region_profile_with (eval.rs, core's code; core told): arcs of one circle at the loop's two ends join (7bc9d8 Inner
   Ring). Test features_a::a_profiles_two_arcs_of_one_circle_extrude_to_one_face.
7. (bf90798e5) translator: live's parts -> ours by body id (alike names no longer collapse; robotic-arm 07d06e) and
   standard content at several sizes in one studio: configured Name + Material per size, material from live's BOM
   (the insert dialog's choice hides in @scp). Single runs (c03 vs c01, vibex7 ~/agents/asm-keep/one/*.log): BOM
   robotic-arm 16 -> 18/18, bjr-config 20 -> 22/23, robot-relations 33 -> 35/42, pinslot 19 -> 21/21. Test
   parity::alike_named_live_parts_take_two_of_ours (fails without). Gate: ~/agents/asm-keep/p02.sh runs assemblies
   with ops-c03 on p01's snapshot after p01/done -> p01/asm-c03.log (compare with asm-c01.log).
8. (71fe284ba) translator: a part only another configuration makes (xdev-subs "Bearings mount" JlD at v1_1_3_3):
   query_parts at the instance's configuration + part_by_creator. xdev 244 -> 256/266 occurrences.
9. (0953c1022) test standard_content_sizes_name_their_rows (fails without 7). (65346c38c) a live sheet body that's part
   of our solid -> our faces at the sheet's faces (robot-relations 0c3466 Thicken 1, lead's 11-17 ruling: we keep our
   valid solid); `merge_refs` keeps a line's Surface kind under refs.jsonl BODY lines (also the 3 model-runner sites in
   tests/parity.rs, so models need a pair too). Thicken then makes 11 tiny parts vs live's one 223.99 mm³ -> core
   (repro vibex7 ~/agents/asm-keep/m0c, model dir name holds 0c3466 for dx's LIVE_IMPORT_FAILURES).
- Gate 2 done (02:59, vibex8 p08/asm-c08all.log): better xdev 244->266/266 occ, tvc 8->12/12 mates, corexy 22/22,
  pattern-std mates 50->92, pinslot BOM 21/21; worse: (a) isaac mates 90->89, BOM 59->58 + "Part 2" from b8d2d867e
  (a04711's right inputs hit bool's Extrude 4 cut; bool fixed it in bool-shared2 8ba7c6dcc: b8d2d867e lands after it,
  with an isaac rerun); (b) pattern-std Sub 427d85 <1> > AR-S2-108.2 <1> 10.4 mm off (live's Fastened 2 there
  unresolved, Fastened 1 OK); (c) scissor-lift occ 214->212: Fastened 26/35 now resolve (live OK) but Sub aa9534 <1> >
  Output Coupler <1> 19 mm off and turned, Sub 6ed9b2 <1> > T8 Leadscrew <1> turned about its axis. Traces (ops-c23)
  on vibex7 after 04:00: ~/agents/asm-keep/tr2.sh -> one/ps23.log, one/sc23.log, one/tr2.done.
- scissor-lift c26 (vibe2 one/sc26.log): 212/242 (Fastened 35, Z aligned, PLUS_Y). Mapping (traces): our flip_primary
  = NOT live's primaryAxisAlignment (true = Z opposed). Corpus raw frames (/root/asm-scratch/bin/reorient_stats.py):
  opposed PLUS_Y +90° (45), aligned PLUS_Y −90° (8); live2's Q on pin-block (aligned) agrees. So base_rotation(flip on)
  = Rz(−k·90°), flip off unchanged (c27 had it backwards; its A/B stopped unrun). c28 = that fix + edited mates hold
  their free values (Q/A turn the part, live mate-keys-1118). A/B: vibe2 ~/agents/asm-keep/ab28.sh -> ab27/c28.log vs
  ab27/c26.log (done28). Open: live's A on that Revolute turns about the connector's Y (diag(−1,1,−1)); ours about X.
- P0 HEAVY-ASM-SOFTWARE-FRAME (qa, xDev): cause = angle display mesh density: 7.76 M tris (SCHUNK Gehaeuse 355 faces
  1.17 M, 7 mm bearing 67k, _787261002 691k ×5); chord mesh 1.05 M; headless render 15–18 s -> 2.95 s (vibex8
  ~/tmp/asm/perf, probe /root/tmp/render.rs.main). Handed to psui 11-18 12:3x; assembly side doesn't scale it.
- LANDED triad 89eca69c. QUEUED: A about Y in an edited mate 8887a5149 (asm-feel), fixed-drag cursor fb5e117d (asm-cursor).
- Suite pair main vs 28-commit stack (lead: land on the light 20 if none worse): ~/agents/asm-keep/pairlist.sh <tag>
  <dirs>: vibe2 tag a (9: clamp … keyboard; 8 done, none worse, corexy 21->22 mates), vibex8 tag b (linear-limits …
  robot-relations), vibe2 tag b2 (welding, tvc, suspension, robotic-arm; stop b on vibex8 before those) -> pair/
  {mainbase,stack}-<tag>.log, done-<tag>; bjr done (BOM 20->22/23). Heavy 4 tag c 02:00 on vibe2 (night.sh). On the
  stack after the pair's binary (UI / scorer only): 5ef547724 BOM scorer, ddf4bab7a Insert sort menus (live insert-sort-1118).
  Insert from a release (▲ view, live i9–i10): live shows it on company-owned docs only; ours has none -> lead asked.
- Thread ids (11-18 lead): core e8af89fe-d460-49b3-b0bb-9572da230fed, blend 0672fa20-964d-4fde-a19c-c5b4363729ba,
  qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b (old core 6692ae35 / blend 0e6e9942 stopped: never message them).
- LANDED 11-18: asm-feel3 52c050e1, asm-views2 916b2d59, qa's save switch b2473af7, solver reorient d9fce0c0,
  snap retake + Display states c7d388e8.
- scissor-lift: Fastened 35 (Sub 6ed9b2 > T8 Leadscrew edge centre, align face, ↔ Sub aa9534 > Output Coupler edge
  centre) now resolves (main: b end unresolved) but our X turns 45° (live X a (0,1,0), b (0,0,-1)): leadscrew and
  coupler turned. Trace with each OK mate's end angles: ops-c25 vibex7 one/sc25.log (slot).
- Dogbone: c23 on vibex7 5/5 custom features (as base09): vibe2's run environment, not the branch (something read at
  run time from vibe2's ~/agents/asm/src; not chased).
- Validity pair done (local /root/tmp/asm-v2): no new invalid solid (base's 38 lines are c2-config-parts-screws,
  unscored in both; branch's 1, singularities' Thicken, also in base). Dogbone WORSE was vibe2's environment (above).
- Single traced runs: vibex7 ~/agents/asm-keep/one.sh <bin> <asm dir> <tag> -> one/<tag>.log.

### Assembly feel (P0, lead 11-17 23:xx; live asm/feel-1117, qa TICKETS 709c64f5a)
- 9b300ef4e drag with nothing fixed in the group: the group translates (least move, mates' null space), no turn
  (ASMFEEL-DRAG-GROUP, -ROTATES; test solver a_drag_with_nothing_fixed_moves_the_mated_group_without_turning). A
  fixed part in the group keeps the old drag (live's drag probes, A fixed).
- 98f57c48b connector to a face's dot within 50 px else centroid, an edge's end within 15 px (CONNECTOR-REACH;
  a_faces_connector_takes_a_dot_within_50_px_else_its_centroid); row hover outlines, no tint (ROW-HOVER;
  a_hovered_instance_row_outlines_the_part_without_a_tint). Land with the branch; ask qa to rerun its asm-feel rows.
- 5a988f63f triad drags (live manip-feel-1118): Shift frees the snap, other handles fade, line/arc dimension, Esc
  drops the box. Arrow step: live's rounded manipulator rule in the document's unit (live2 triad-step-1118).
- c27dac76d live2's feel3-1118 (beyond Fastened): all matched except a NEW mate's typed limit, which the zero holds
  ignored; new_mate_holds now clamps into the limits (tests a_typed_limit_turns_the_new_mates_preview_into_range,
  a_revolute_drag_stops_at_its_limits); limits preview on Enter / blur, not per keystroke. Open, minor: a Revolute
  range across ±180° (atan2 wrap) untested in live.

### Decided
- fffe186b (closest-sum region match): DROP. q79 (s76 vs s77, isaac) gives a04711 Extrude 4 / Sketch 5 / Loft 1 / 2 /
  Extrude 13 / 14 / 16 / 26-31 / Chamfer 1 new errors. Branch asm-with-fffe186b kept only for reference.
- dx told the q84 / q85 numbers (11-17 09:2x). q80 (old base) superseded by p01.

### Others' items touching mine
- Agent ids (lead 11-17 10:30, only these): psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (old 4d0a82b2 stopped), core
  e8af89fe-d460-49b3-b0bb-9572da230fed, blend 0672fa20-964d-4fde-a19c-c5b4363729ba, bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4,
  dx 2108c2b7-2f48-43a7-87a0-3f652e3cda6d (old f93a1292 stopped), qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, asm (me) d8d597e0-675c-497b-a804-544ceab99bbd,
  skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
  58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (old 3b9b1e66 stopped).
- P0 SCISSOR-LIFT-BOM-ROWS: culprit core 41f62884a (pattern/Mirror Add joins surface copies; fed22d Fillet 18 fails; bool corrected 11-17). core
  reverted it (ddfea0eb, verified: BOM 59/87, Fillet 18 OK). When it is on main: rerun asm-scissor-lift (BOM back to 59/87, no "Motor Mount" row)
  and asm-pattern-std (one.sh on vibex7).
- c2-impeller-loft regression from my 57de87f0: fixed by core on main (`shares_whole_region` in parity.rs query).
- bool 9d68c081 (36b90f Extrude 11 cylinder sectors) landed 11-16 21:06: p01 shows pattern-std's effect.
- blend: gearbox Fillet 2 (6a912b / a470167 feature 5, 12 mm³, repro vibex6 ~/agents/asm-keep/mgb); scissor-lift fed22d
  Fillet 3 / 4 / 5 / 15 (repro vibex7 ~/agents/asm-keep/ms).
- core: 7bc9d8 Hole feature 8 (Motor Mount 10/12/3 vs live 8/8/0) and feature 22 (Inner Ring), then Mirror 3; repro
  vibex6 ~/agents/asm-keep/m7b.
- dx keeps infento (1dc32b, 74d3d6) and bjr-config (5d8bc1); tell dx before touching the derivedFrom /
  ORIGINAL_DEPENDENCY path in the translator.
- robot-relations 0c3466 Import 1: live makes 6 SHEET bodies (no solid), ours one invalid solid (28 free edges); dx took
  it to the lead as a decision (import an open shell as sheet pieces), tells me when it lands -> rerun robot-relations
  (BOM 35/42 rest).
- isaac on main + dx's GearLab fix (c07, rebased stack): 117/120 occurrences, 59/62 BOM (Helical 40T in), mates 91 -> 90:
  Fastened 34 (Part 1 <38> / <36>) "cannot resolve mate connectors" new vs c03 (old base). Check in the gate whether c05
  vs b01 loses it (then mine), else main's (find the studio of Part 1 <36>). Left: 1e81ca GearLab fillet (blend), Helical
  13T.
- bjr-config OPG006: Wire clip FS imports element 90442c40… not in the corpus; isaac 8f5622's GearLab imports
  76ad54d4…: asked live to fetch (REQUESTS.md 11-17 10:29). isaac 2d7dc1 has GearLabFeature.fs without those 16 imports
  (unknown feature type, its mates untranslated): live2 copied 8f5622's 02:22 (17 studios): rerun isaac on main after landing. isaac 1e81ca GearLab's opFillet FILLET_FAILED -> blend
  (repro vibex7 ~/agents/asm-keep/m1e).
- pinslot washers in Bearing Assembly 0.3 mm off along the axis (4 occurrences): live's occurrence differs from the
  sub's own placement; unexplained, low.
- isaac (c16 11-17 21:2x: 117/120 occ, 90/122 mates, 59/62 BOM). Our instance names differ from live's (duplicate
  live names renumbered): map by assembly_list sources, not live names. 8 unresolved ends by studio (22:3x):
  a2e4a2 (Ball 7, Fastened 33; also the 7 inconsistent + ~16 overdefines): Chamfer 2 TWO_OFFSETS 5.5/6 removes 608 mm³
  more -> blend (repro vibex8 ~/agents/asm-keep/ma2). e54410 (Ball 13/14, Revolute 17): Chamfer 1 TWO_OFFSETS 2/5
  fails -> blend (me5); its Extrude 22 µm sliver -> bool (lead + live capture). a04711 (Revolute 6/11, Ball 13): fixed
  ef18dc749 / b8d2d867e (region sums: slivers, nearest group); next Extrude 4 cut non-manifold: core says our
  Boolean's OCCT path (plain OCCT gives live's), sent to bool; main passes it only with a wrong Extrude 1 (watch isaac
  in gate 2). 850efe (Fastened 34): Extrude 5 union 6 faces more at live's volume, Fillet 2 garbage, Extrude 7/8 270 s
  / 61 s (P0 speed) -> bool (m85). c3fdd4 (not in isaac's failures) Extrude 7 fixed, Fillet 1 -> blend.
- tvc-mount now 10/12 mates (Cylindrical 1 / Parallel 1 wait on 7bc9d8, core).
- pattern-std: 36b90f far from live (no live volume bands; parts collide in mapping); geometry first (bool/core).

- REGRESSION on main: asm-rack-pinion mates 8/8 -> 4/8, bisected to core 747c2bf0d (sketch on a Transform-moved face
  keeps the face's old frame): ab25c5 Sketch 4's frame rotated 22.5°, Extrude 2 misses the tooth gaps (22 -> 9/29).
  Sent to core as P0 11-17 ~15:00 (repro vibex7 ~/agents/asm-keep/mab); core's fix queued eb3c2bd4 (ab25c5 9 -> 18/29;
  11-16 had 22/29: Extrude 6 centroid 0.16 mm off now, check). Rerun rack-pinion mates when it lands.

- isaac 8f5622 GearLab (live fetched its 16 Feature Studios 11-17): compiles; fails at run time "Precondition of Fill
  failed" -> dx fixed qLoopEdges of a region (queued bc58c02b; then 1 part 363/842/480 vs live 283/762/480, 8.1 s vs
  1.15 s: dx next). Rerun isaac when it lands.
  Repro vibex7 ~/tmp/asm/mfs (links to ~/agents/asm-keep/m1e/8f5622). bjr 5e35bf Wire clip: imports a Feature Studio of
  a document live can't open (403): a data gap, not fixable.
- robotic-arm on refetched assembly.json (live2 16:19; "nắp sensor" partId "", a live defect): c07 31/31, 37/37, 18/18
  as live. Left: studio 21ab69 Fillet 1 and eed029 Draft 1 errors (old, also in b01); repro vibex7
  ~/agents/asm-keep/mra (run.sh <bin tag>): 21ab69's custom Timing Belt Pulley 14200 vs live 21160 mm³ -> dx;
  eed029 Fillet 1 two edges short (then Draft 1 fails) -> blend (both told 11-17 16:5x).
- pattern-std 50/92 mates: 36b90f's AR-S1 / AR-S2 / AR-UP-LID all went to our Part 9 (live.jsonl volumes are
  numbers, no band, so by_maker gave none). Fixed on branch: by the line's volume (test
  parts_of_one_extrude_are_ours_by_live_lines_volume, fails without). vibex8 ops-c08 (one/ps8.log): mates 50 -> 92/92,
  BOM 10 -> 13/17, occurrences 162 -> 161 (Sub's AR-S2-108.2 <1> 10.4 mm off: our AR-S2 is 10470 mm³ short of
  live's 332112, a studio tail). Its Extrude 12 (17 faces vs live 14, same volume): not the Boolean (bool, OCCT-only too) -> core 11-17 (repro vibex8 ~/agents/asm-keep/mps).

### Next, in order
0. live2 truth (behaviours "asm (lead 11-18 ~13:32)", live/asm/truth-1118/), worked 11-18: pattern-std BOM: live's
   Linear pattern 5 ERROR drops its instances from live's BOM -> scorer live defect (5ef547724). pinslot washer 0.2985
   mm: MIRROR fixed by skasm (270e489b); next rollback in std studio 26bb0b Sketch 1 is CENTERLINE #ClearanceHole 1.632
   mm (its config / setData not in the corpus: our std variables, mine; vibex7 asm-keep/pinslot-main.out). bjr OPG006 Description: 5e35bf's "Wire clip 1" imports an FS missing from the corpus -> REQUESTS
   13:55. bjr grubs (UJ <2> 4 mm): live's 103_06_ASM flexible inside each flexible UJ (per-copy angles); ours keeps a sub
   holding a flexible sub rigid (eval.rs prepare_subassembly), `nested` one level -> nested flexible = work item (lead).
   robot-relations ×6: 6 sheets vs our solid, as ruled (11-17). scissor Top <3>: fed22d model side (lead; vibe2 asm-keep/trace-scissor.out). isaac
   Helical: GearLabFeature unregistered -> FS owner. infento Wheellock: f0eedd sketch/fillets, model side. welding
   "Part 2"/"Part 6": ours one part fewer in 9c7704, tracing (vibex8 asm-keep/trace-welding.out; one.sh <bin> <dir> <out>).
2. Lost mates still mine: tvc (c10 on vibex8: 11/12 mates, 26/26 BOM; alike-volume parts now settled by their mate
   connectors, 62043ea70). Left: Cylindrical 1's edge on ".2_Peça <1>" is on no SG90 body of ours; endcheck/find_ent in vibex8 ~/tmp/asm),
   xdev-subs (c16: 262/266 occurrences, BOM 74/74 and no extra rows). 59e7fd03d: a nested flexible subassembly
   whose parts agree on one frame is placed by them (SFRRGA3-29-P25 turned over in Latch pusher linkage <1>); live's
   subassembly metadata (studios/<element>.asm-metadata.json, live 11-17) gives exclude-from-BOM and BOM behavior.
   Test a_nested_subassembly_is_placed_where_its_parts_land. c541261fd: three levels deep (FXAA3-7-F5-SET <2> in
   linkage in Latch pusher linkage) the parts' frame now comes from the holding tab's own nested places, deepest paths
   first: xdev-subs c17 266/266 occurrences, 7/7 mates, 74/74 BOM (all as live). The 3-level case in that test fails
   without it. Configuration checked: live's features list is linkage's Economic_version=true one; suppressed
   Cantilever pins drop their mates' ends.
   isaac (16 overdefined, 8 unresolved, 7 inconsistent, 10 untranslated), infento-subs 3, scissor-lift 6 (after
   core's revert), rack-pinion 4 (after core's eb3c2bd4). pattern-std, keyboard, config-assembly, pinslot: all as live.
   endcheck.py (vibex6 / vibex7 ~/agents/asm-keep, /root/asm-scratch/bin) says whether each failing mate end's entity is
   on its instance in a kept document.
3. Thread lengths: done on the branch: option lists for all 83 components (6f95a9bc0) and live's rule (14b6458be): keep
   the previous Thread length while offered, else the first option; threadDefaults removed (they were that rule's
   leftovers). Remembered picks (ebf597c8e): prefs::StdMemory in standard_content.json, each level's last child and
   each component's last spec, kept as a picker changes (not while editing an instance); the dialog reopens on it.
   Also cccd03972: a massproperties-at-rollback capture's volume counts on its last row only (robotic-arm's 4 studios).

### Tools (main machine /root/asm-scratch/bin, copies in vibex6 ~/agents/asm-keep)
- feats.py <features.json> <from> <to>: features with op / pattern / boolean summary.
- mate_ends.py <assembly.json> <mate names…>: a mate's connector ids (grep them in a traced log).
- endcheck.py <traced log> <kept .noshape> <noshape-cli> <mate>: each end's entity on its instance or not.
- dupnames.py (run in ~/parity/models): studios with same-named live parts.
- noshape-cli on vibex6: ~/agents/dx/target/debug/noshape-cli (batch; feature_rollback exists in the source but not
  in that older CLI build).

### Others' items touching mine
- dx keeps infento (1dc32b, 74d3d6) and bjr-config (5d8bc1). dx: derivedFrom + ORIGINAL_DEPENDENCY (top-level EDGE)
  queries resolve to faces / sketch regions in the translator; dx covered 1dc32b's Chamfer 4 / Fillet 5 with live's
  refs, not a general fix: tell dx before touching that path.
- core: 1dc32b's Chamfer 3 (dx) brings back 14 infento occurrences; Hole 2 (2cc476fe) is core's.
- blend (20:3x): 3c5c58 Fillet 1's input matches live; OCCT fails on the 14 edges together, the by-groups fallback splits
  two rounds; third in blend's queue.
- dx (19:3x): assembly studios take the Feature Studios in studios/<element>.fs their sources import (bjr-config
  5d8bc1); lands near my contextEntities loader lines in crates/ops/tests/parity.rs: whoever lands second rebases.
- Lead ruling 11-18 (supersedes DECISIONS' 11-15 line on unresolved connectors, not yet recorded there): a reference
  failure live shows fails in ours too.

## Scores (last measured)
- Assembly suite, same-base A/B (vibex7 ~/agents/asm-keep/s01): isaac-ball base29 113/121 occurrences, 97/103 mates
  as live; config-assembly 18/18 mates; suspension 6/6 mates; infento-subs 144/167 occurrences, 176/182 mates (61 -> 3
  parts our mates move off live's place); corexy, xdev-subs, clamp-pattern, robotic-arm, bjr-config,
  hand-drill-pinslot, keyboard-pattern unchanged by 36ebccbc.
- Model gate (all models, ops-s28 / s29): 76/87 documents, 881/947 features, custom 103/107 at base24; 80/87 and
  895/947 at s29 (the gain is main's hole fixes via the rebase).
- asm-* ui-parity: 213/214 (only live's crashed asm-cylindrical-limits take).

## How to run
- Hosts (lead 11-17 17:00): builds, crate tests and assembly suites on vibex8 (`NOSHAPE_RB_HOST=vibex8`; scripts and
  repro dirs copied to vibex8 ~/agents/asm-keep); vibe3 and vibe5 are gone; vibex7 holds the land queue (the p01/p02 gate
  there runs out). Mac only for speed numbers.
- Suite binaries are Linux builds made on a vibe host: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c
  'CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo test -p noshape-ops --test all --no-run >
  ~/agents/asm-keep/build-sNN.log 2>&1; f=$(ls -t ~/agents/asm/target/debug/deps/all-* | grep -v "\.d$" | head -1);
  cp $f ~/agents/asm-keep/bin/ops-sNN'`; copy between vibe hosts with `zstd -c | ssh … zstd -d`. Never cut cargo's
  output with `head` (SIGPIPE stops the build): log to a file, grep after.
- Heavy runs (assembly / model suites, gates) through `~/bin/slot <cmd>` on vibex8 (one per host, waits for
  10 GB free), never vibe3 (shut down 11-17 17:15). Long runs started over ssh with `setsid nohup bash script.sh > /dev/null 2>&1 < /dev/null
  &` (systemd user units get oom_score_adj 100+ and earlyoom kills them first; `EXIT 143` = earlyoom, see
  `journalctl -u earlyoom`).
- One assembly: scripts in vibex7 ~/agents/asm-keep (isaac29.sh, cfg29.sh): `NOSHAPE_PARITY_TRACE=1
  NOSHAPE_PARITY_ASSEMBLIES=<dir with one symlinked corpus> ops-sNN parity::parity_assemblies --ignored --nocapture`.
- One studio as a model: `<dir>/<name>/` with features.json, live.jsonl, sketches.json, mass.json, refs.jsonl
  (copied from the corpus' studios/<element>.*) and a `studios` symlink to the corpus' studios; then
  `NOSHAPE_PARITY_MODELS=<dir> ops-sNN parity::parity_models --ignored --nocapture` (ma2 = a2e4a2, mb1 = 1b88ff,
  m90 = GearLab 90f78b, m3c = 3c5c58).
- Model gate: `NOSHAPE_PARITY_MODELS=$HOME/parity/models ops-sNN parity::parity_models --ignored --nocapture`, base
  vs change, per-model diff. The base must be built from the change's own base: land.sh rebases the worktree, so
  rebuild the base after a land (check out HEAD's files, touch, build, restore).
- Traces: NOSHAPE_PARITY_TRACE=1 (each feature as sent, external refs, region picks); NOSHAPE_FS_TRACE=1 (FS
  operations and why a custom feature failed).
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` (lead 21:23; vibex7 finishes what was queued there); `--status`.
- Live data: ask in /root/onshape-research/live/REQUESTS.md (times from `date -u`) and message live
  (b89af730-e8c6-4234-b83e-00a9e1ba6acc). live's scripts: asm/q-isaac/refs.js (refs at a sketch's rollback,
  USE_SOURCE=1), fsfetch.js (imported Feature Studios).
- ui-parity asm-*: `tools/ui-parity/run.py --app <noshape> --display :105 <names>` on vibex6/vibex7 (≤ 4 apps).

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over /root/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, then each level (subassemblies first, each its own "Sub <eid>"
  tab) inserted, fixed and placed as live's occurrences, mates between the same connectors (root mates reach nested
  parts by path; instance ids are unique per level, so maps key on (level, id)); top-level instances held at live's
  places, compared, freed: our mates mustn't move them; mate status as live's. Not yet: patterns, groups, relations,
  mate limits, configurations, BOM.
- Connectors: implicit (MID_POINT / CENTER / POINT on a face with a second id = the edge / vertex), explicit Mate
  connector features (flipPrimary, secondaryAxisType, transform -> the mate end's flip / reorient / move; not when
  realigned to other entities), live's matedEntities CS paired with picks by occurrence path.
