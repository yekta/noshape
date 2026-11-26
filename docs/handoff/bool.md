# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-26 03:50 UTC. Hosts by load (DECISIONS 11-19): read
`ssh <h> cut -d" " -f1 /proc/loadavg` on vibex7 / vibex8 / vibe2 / vibex6 (crate tests only there); the Mac is back
(lead 21:36: headless, ~/noshape-host only, 2 slots): builds + probes there (NOSHAPE_RB_HOST=mac from worktree
bool-hat, inside an rb.sh slot, Mac paths ~/noshape-host/bool-keep); gates through ~/bin/slot, one per host, as systemd-run units
(a background task dies at 2 h); land queue vibe2 (`NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`); private docs
only vibex7 ~/lead-private. Never kill a process tree from a computed pid: `systemctl stop <unit>`. Agent ids: only
/root/lead-scratch/roster.md (read before messaging).

## Order (lead 11-21)
P0 impeller speed (c2-impeller-loft, blend measures per feature); P1 asm's SPEED-ASM-OPEN (infento). Gates + knurls
for every brep / Boolean / Shell land; list the corpora each gate ran. End a turn after each land.
Knurls: /root/bool-scratch/kn-run.sh <host> <dir> (systemd unit) with k-main / k-br = the gate trees'
all-96c035059fc28abb copied to ~/tmp/bool/<dir>; OCCT side ~/agents/bool-keep/knocc; delete the copies after.
Daily report: share as "ours N/M (general, direct), OCCT, differing" (DECISIONS 15c2c6406).

## NEXT (state 11-24 14:55 UTC)
- P0 11-25 05:55 UTC (blocks v0.4.69): macOS CI a_lofts_added_vertices_stay_for_good still "shell took too long" after
  1b9ab96c. Cause (CI diag runs on scratch branch bool-macdiag, deleted): deterministic on macOS 26 only, not a race;
  by_quadrics' corner (10,30,0) solve3 started from a foot hint left by the last corner, which Newton took to the
  loft's far side, the solve circled, OCCT offset 31 s. Fix 5692207b2 (worktree bool-mf, branch bool-mac): the
  corner's feet found afresh (has_hint = false) before each start. macOS 16/16 pass (was 0/16), Linux kernel 594 ok.
  Gate clean (132->132, replay =, speeds 1.00x; impeller "changed" = timing text only). QUEUED cf23398d, lead told.
  FLOW-SHELL-AFTER-FILLET done 11-25: qa's start part has concave R10 ends -> four zero-angle tips; the convex
  obround shells (2194 mm3); test + ticket note QUEUED 64255e03 (worktree bool-sf), lead told.
  0037 LANDED ebc674fc (lead installed libs everywhere). Corner decline PARKED again (lead 11-25; probe result: 25/30
  other late declines, 5 differ; worktree bool-corner has the uncommitted probe, delete it).
  P0 (lead 11-26 01:00, qa TICKETS BOOL-ADD-FLUSH-CYLINDER-SHELLED-BOX): FIXED 8b52d16a6 (worktree bool-flush, branch
  bool-flush-p0 on origin/main 11-26 01:45). Causes: (a) OCCT's section of the shell's inner corner round (R3,
  vertical) and the tool cylinder (R3, horizontal, axes 5 mm apart) is EMPTY, even seeded: occt_sections now asks
  edges_piercing when the boundaries meet 0 times too (was == 1), and seeded_sections' midpoint projection runs 400
  steps (50 left it > 1e-7 off: no seed) -> the widened retry gives the loop; (b) OCCT's outer-round section has a
  43 um curve along the tool's seam (0.18 um off): a curve along a straight edge of ONE face is now that edge's
  piece InFace on the other face (else two edges between the same vertices, a sliver); (c) C++ chord_pcurve allows
  tol + the edge's vertex tolerance (0.18 um > 0.15); (d) the earlier seam-piece snap (of_face). All 5 recorded cases
  (vibex8 ~/tmp/bool/p0/rec) taken; volumes = slice integration (x0-20 15587.68099, x1-20 15559.81505, x4.9-20
  15468.10594 mm3; x5-20 check 15465.83). T1 bool_patterns::a_cylinder_along_a_shelled_boxs_wall_fuses_through_its_corner_rounds
  (data/booleans 7bf937 / 1582a3 / b81914). T1 PASSES, small cases 78/78 0 differ (t1.sh, vibex8 ~/tmp/bool/p0/kt).
  GATE (03:38, P0 + item 1 as ONE stack: 2c051fbee cherry-picked onto bool-flush-p0 as 1a04f549d; the 2c051 gate had
  waited 00:51-03:37 for vibex8's slot (qa's parity held it), stopped): unit bool-gate-p0s -> /root/bool-hat-scratch/
  chain-1a04f549d.out. Then knurls (chainf16.sh pattern: copy both gate trees' all-96c035059fc28abb to vibex7
  ~/tmp/bool/<dir>, kn-run.sh), land from bool-flush (NOSHAPE_LAND_QUEUE_HOST=vibe2 land.sh: lands both), tell qa
  (roster id, qa moved 11-26) + lead. OCCT stays wrong on these (fallback only). bool-slow (2c051fbee) then obsolete.
  UNIFY SCOPE TRY (11-26 03:40, PARKED, /root/bool-scratch/unify-clean-full.patch): every op_unify result marked
  clean (unify twice = same on all 19 readable impeller dumps), solids inherit it (sub_shapes/list_get), t_built.scoped
  for free-form results. Impeller: only 4 of 24 unifies scoped, Bohrungssitz still whole (the mark is lost elsewhere),
  no gain; NOSHAPE_UNIFY_CHECK aborts at Fillet 4 (whole unify_in's merge changes volume, op_unify reverts it: the
  check is stricter than op_unify). Don't resume without finding where the mark is lost.
  UP-TO EXTRUDES (item 6 Extrude 4, item 9 valve Extrude 3): prism_until.split (OCCT Splitter) 128-142 ms; both targets
  free-form (impeller: B-spline + offset faces; valve: revolved + Bezier): no quick win, needs our Boolean on free-form.
  Next after the land: knurls, land from bool-flush, tell qa + lead; then item 2' SP3 or item 7.
  MAIN REGRESSION (told lead 03:0x): psui 8a62f4834 failing_alone reruns a failed fillet per edge: impeller "Fillet
  Ansaugflansch zum Gehaeuse" (fails either way) 1.1 s -> 60 s.
  SLOW-FEATURE LIST (lead 11-25; re-measured 11-25 12:55 on main 47b49528b, vibex8 ~/tmp/bool: cli-main,
  fl-main.jsonl all features, sel-main.jsonl 60 rows >= 100 ms and >= 2x live, prof-main.jsonl their kernel profiles;
  scripts /root/bool-scratch/featlist.py profsel.py fl-main.sh, prof1.sh <cli> <doc> <feature> one profile,
  rec1.sh <cli> <doc> records a doc's Booleans to ~/tmp/bool/rec-<doc>, rp1.sh <kt> <rec> <case> replay+profile,
  perf1.sh flat perf of a replay). Kept (Boolean/Shell/section time), ours/live ms, sorted by ours - live:
   1 impeller Extrude Bohrungssitz Ansaugflansch 1534/152: ours brep 873 (trimmed 747) + unify 239
   2 cf-surface-pattern Surface Pattern 3 1674/497: ours brep 1376 (sections_ahead 283), 85-tool cut
   STACK (branch bool-gear2, worktree bool-slow; probes uncommitted there, NEVER commit: /root/bool-scratch/probes-slow.patch):
   a) 43bea0b8f gear (cf-spur-gear Spur Gear 1518/344: ours declined the 20-tool cut, OCCT 1.4 s -> ours 0.17 s):
      cyl_cyl takes cylinders along each other within tol over the faces' common box as on one axis, ONLY with axes
      within 1e-4 rad (without: fillets meeting at a corner, boxes barely overlapping, passed: corpus5 55a240914f323d6f
      / 7a6675388bf2449a non-manifold; fixed); edge_on_surface (Cyl, Arc) takes a circle a hair off the axis. Test
      a_gears_tooth_gaps_turned_a_hair_off_its_axis_cut_its_rim compares history with OCCT's run (records within 2 um,
      brep_replay::same_records made pub(crate)).
   b) 41ec4bcc4 fix A: pattern's checked glue with < 16 copies reaching in: our plain fuse at once (rings 930 -> 100 ms).
   c) 44368f219 fix B: nurbs_pair's moved sections from the family's first: none + the first pair's boxes apart ->
      the pair's own sections (impeller seats: seat 1 at x -19 past the flange round face 48 (x 6..82), seats 2/3 cross
      it; moved empty -> seat walls unsplit, non-manifold). Impeller Bohrungssitz fuse a80ba977 ours, 0 differ, 405-490
      ms vs OCCT 640. Test a_row_of_seats_on_a_rod_past_the_first_cross_it (declines without the fix). parity56
      513/593, bool-corpus 919/972 0 differ (same); corpus5 873/929 0 differ (3cca316c2fd10dc9 declined "coincident
      pieces differ" with or without fix B: check vs main in the gate).
   d) c2e8a6c24 unify: apart pairs on cylinder/cone frames of opposite hands (Direct() differs) get no reordered
      retries (impeller Remove Bohrung Ansaugflansch: unify 0.49 -> 0.19 s, 57 faces either way; feature was 670 ms
      vs live 54). Unify inputs dumped by NOSHAPE_DUMP_OPS=<dir> NOSHAPE_DUMP_ONLY=unify (cli feature list); probe test
      bool_patterns::probe_unify_dumps (NOSHAPE_PROBE_UDUMP=<dir>, uncommitted) + /root/bool-scratch/ud.sh time each.
      Verified only on the impeller's 22 dumps (face counts same): the gate's model suite is the real check.
   e) aefd02cba stretches_over: an edge's 257 points rejected by the free-form face's patch boxes (off_patches) before
      uv_of (hulls() is None for multi-span NURBS, so clearly_off never fired): impeller Boolean 1 (730b0fe5, offset
      fit 210 x 58 poles deg 9) 5911 projections -> 61, replay 0.48 -> 0.16 s; result-preserving; parity56 /
      bool-corpus / corpus5 same counts, 0 differ. Gate c+d+e together.
   Impeller A/B 11-25 16:55 (cli-fit = a-d + a fit-cache try, 3 rounds alternating, ms main -> branch): Bohrungssitz
   2356-3056 -> 674-810, Remove Bohrung 744-962 -> 331-392, Boolean 3 236-269 -> 111-127. TRIED + REVERTED: offset fit
   cache reusing a fit whose range holds the face's (Extrude 3 convert 81 -> ~0): Boolean 1 then declined in the
   model (another fit, history-dependent) -> OCCT 1 s. Don't retry without finding that decline (rec-impfit 730b0fe5
   replays fine alone). Degree 7 fit (1000 segs): within noise. Boolean Flansch+Gehaeuse (79058fda): ours 20 f /
   59.1385 cm3 vs live 19 f / 59.1379 vs OCCT 16 f / 59.111: OCCT wrong, ours right (replay shows "differ": ignore).
   recfeat.py <cli> <doc> <recdir> names each recorded case's feature. knife (skasm 11-25, lead routed it to bool): Revolve 1
   vertices 18 vs live 19: shown_topology joined OCCT's two halves of the sphere's ring round the loft (a B-surface closed,
   not periodic: clamped knots) into a ring with none; live keeps one (Parasolid's edge can't cross the closure; live's
   sphere is 1 face / 1 edge, the loft 13 edges, no seam). Rule e77b7625e (common.cpp crosses_closure, loop case only;
   T1 features::a_ring_round_a_lofts_closure_keeps_a_vertex_there): knife 10/27/19 as live. NOT for single closed edges
   (sweeps' end rings would gain vertices): extend only with a live case. Model suite must show no regression.
   sm-rocker Extrude 11 (item 8) 0c7a80c70: unchanged() returns changed when as many edges moved > 1e-4 diag (187-231
   ms -> < 40, live 12). Stack on bool-gear2 now: a b c d e + e77b7625e + 0c7a80c70 (cherry-picks of bool-kvert in
   worktree bool-p0); gate it with /root/bool-scratch/chain-0c7a.sh after gate a+b.
   GATE 0c7a80c70 (whole stack) FAILED 19:06: bool-corpus 58c61d5ddca8047a (Fuse, 9 tools, recorded trimmed) ours
   65 faces vs recorded 56. My earlier c58.sh "pass" was WRONG: it set NOSHAPE_BREP_VS_OCCT=1 (compares with OCCT run
   on the branch, also 65); the gate's r0 shards compare with the recorded result. The 9 tools are APART (no box
   pairs meet: probe bool_patterns::probe_tool_overlaps, /root/bool-scratch/ov.sh); fix A sent them to the plain fuse
   (faces split; unify gives 56, features unify after, so not user-visible, but the op differs). FIX 336da1a71: plain
   fuse at once only when tools_may_meet(t) (two tool boxes meet; rings / impeller seats do); apart -> trimmed as main.
   Shard replay (sh58run.sh, shard rebuilt by shard58.py from gate-host.py) 0 differ with it.
   STACK now: a..e + e77b7625e + 0c7a80c70 + 517e18df2 (Nearest) + 98c9d3aa6 (SP3 kept samples, 1292 -> 1081 ms) +
   336da1a71 (tools_may_meet) + 7ebcce87e (unify: new_faces_valid also checks KEPT faces sharing an edge with USD's
   new ones: USD changes shared edges in place; c2-impeller-loft's rod (cylinders along x at y 0 z -34) came back with
   open wires (BRepCheck NotClosed), op_unify's final check then dropped EVERY merge: a80ba977 (Bohrungssitz
   Ansaugflansch fuse, ours since fix B) unified 97 vs OCCT's 83 -> now 82 vs 83 (left: one Extrusion + Offset pair
   merged in ours, apart in OCCT's: USD face-order artefact, geometry/topology of both inputs identical). Not
   scoreable vs live: our model there has 10 parts vs live 1 (main too). Not in the gate corpora (VS_OCCT replay of
   rec-c2-impeller-loft a80ba shows it).
   QUEUED 11-25 23:35: ea3241c8 = 7ebcce87e dd95fa415 5854fa73f bb76110c9 4ab734c5c f16a4b4ea 8e66fb5c0 (gates 4ab +
   f16: replay 0 new differ + the 58c61d exception, parity56 0, features +2, knurls 5x 0 each; small-cases fail fixed
   by 8e66fb5c0, run alone: 78/78 0 differ). bool-slow is probe-free now (probes saved: /root/bool-scratch/
   probes-slow2.patch). Lead to tell skasm: Tail Fin landed with it.
   LANDED/QUEUED 11-25 21:20: 3f289aef = a..e + e77b + 0c7a + 517e + 98c9 + 336da1a71 (gate clean: replay 0 differ,
   features 2114 -> 2116/2668; knurls 5x 0). On bool-slow after it: 7ebcce87e (unify new_faces_valid checks kept
   faces beside USD's merge; its gate FAILED only bool_patterns::an_invalid_fuse_result_unifies_as_usd 1174 vs 1173),
   dd95fa415 (Tail Fin), 5854fa73f (item 4: USD merged no face -> no other order), bb76110c9 (louver fix: a face
   invalid in the input already passes), 4ab734c5c (AT-ONCE RULE, lead 11-25 DECISIONS 3ab79c766: a pattern of < 16
   copies fused at once, apart too; tools_may_meet removed; bool-corpus 58c61d is 65 faces at the Boolean (recorded
   56): REPLAY_EXCEPTIONS in tools/bool/gate-host.py; T1 bool_patterns::nine_seats_apart_fused_at_once_unify_to_the_
   recorded_faces pins 56 faces + volume after unify (data tests/data/booleans/58c61d5ddca8047a.*)). Conditions: model
   suite + parity56 at 0 differences. GATE 4ab734c5c RUNNING: unit bool-chain-4ab -> /root/bool-hat-scratch/
   chain-4ab734c5c.out, knurls (after kn7eb.out's CHAIN-ALL-DONE) -> /root/bool-scratch/kn4ab.out.
   GATE 4ab734c5c 22:04: replay 0 new differ (58c61d listed as the exception), parity56 0, features 2114 -> 2116;
   FAILED only brep_replay::brep_small_cases_match_occt (58c61d is in tests/data/brep too, compared with OCCT):
   FIXED 8e66fb5c0 (git rm the brep/ copy; kt5.sh: small cases 78/78 0 differ, both T1s pass). Suite CHECK
   c2-surf-workshop-manualblend: only the untranslated note's text (Sketch 7's undone solver edits "moved by up to
   686 -> 702 mm", taken back on both; all features' results identical): from dd95's section (Sketch 7 on Plane 4).
   Not in the 7eb gate. Tell the lead with the land.
   Then f16a4b4ea (op_boolean's WHOLE unify leaving the shape IsSame marks myUnifyClean: op_unify "known"; SP3 ~140 ms
   less): unit bool-chain-f16 gates it after 4ab's CHAIN DONE -> chain-f16a4b4ea.out, knurls -> knf16.out. Land 4ab
   if clean at once; f16 after its own gate (model suite must stay 0 differences: the clean mark skips op_unify).
   PLAN: f16's gate will fail the same small-cases test (8e66fb5c0 not in it); if otherwise as 4ab's, land the whole
   stack to 8e66fb5c0 (knurls kn4ab.out / knf16.out must be 5x 0).
   Callgrind of e917cf0b (item 4 Remove Bohrung's cut, convert 108 ms cold, 1.6 warm): 89 % GeomConvert_ApproxSurface
   of an offset surface (to_body's offset fit), as Extrude 3's: the exact-key fit cache misses on each feature's
   fresh copy. Next for items 4/6: find why the range-reusing fit made Boolean 1 decline (see TRIED + REVERTED).
   Land when clean: strip probes in bool-slow (git checkout crates/brep/src/{boolean,curve2,freeform,nurbs}.rs
   crates/brep/src/boolean/free.rs crates/kernel/cpp/{boolean,rust_boolean}.cpp crates/kernel/tests/bool_patterns.rs
   crates/model-core/src/regen/features/sketch.rs), `git diff --stat` empty, NOSHAPE_LAND_QUEUE_HOST=vibe2 land.sh,
   tell lead (+ ask the lead to tell skasm Tail Fin landed).
   TAIL FIN (skasm -> lead, 11-25): "Profile for Tail Fin" in surf-intro ruled/fill/mutualtrim/boundary 200-300 ms vs
   live ~20: callgrind (local, cli copy): 96 % IntTools_FaceFace::ComputeTolReached3d -> FindMaxDistance (no pcurves,
   so it projects points on the B-spline face, an Extrema_GenExtPS grid each). dd95fa415: section_plane sets
   ComputePCurveOn1/On2 -> 24 ms; sampled section points within 3 um (fin-fill doc: vibex8 ~/tmp/bool/models/fin-fill,
   copy of skasm's vibex6 ~/tmp/skasm/fin). Tell skasm via the lead when landed.
   ITEM 1 after at-once (cli 21:35, loaded): Bohrungssitz feature 878 ms (op_boolean 609: TWO brep runs, the checked
   fuse declines "pattern instances overlapping" only at classification, then the plain one; unify 258). No vertex of
   a seat is inside the impeller or back (probe): the early vertex test can't see it. DONE 2c051fbee (bool-slow,
   on the ea3241c8 stack rebased to main 11-25 23:48): flag 8 / with_plain_if_overlapping: for < 16 tools the checked
   run goes on as the plain fuse where they overlap (only if no free-form edge_only pair was taken), a later decline
   reported as "overlapping" so C++ runs the plain one as before. Replay a80ba: one brep run, op_boolean 380/280 vs
   551/392 ms, same 82 faces. GATE running (unit bool-gate-2c051, /root/bool-hat-scratch/chain-2c051fbee.out), then
   knurls (chainf16.sh pattern). Next: the feature's unify 258 (check 81, patches 66, bands 65): scope? (rust path
   scopes only if the target is myUnifyClean).
   SP3 USD (NOSHAPE_PROBE_USD): op_boolean's WHOLE unify (no scope) rejects 129 -> 116 three times (change 1.4e-6 of
   v0), then op_unify runs the same whole unify again (137 ms loaded). TRY (uncommitted in bool-slow): op_boolean's
   whole unify leaving the shape IsSame -> myUnifyClean: DONE f16a4b4ea (A/B loaded 2250/2361 -> 1977/1682 ms).
   Probes (uncommitted, NOSHAPE_PROBE_USD: USD tries / per-stage invalid faces / op_unify check faces;
   bool_patterns probe_face_list: faces + edges + solids of NOSHAPE_PROBE_UDUMP's breps). Dumps vibex8
   ~/tmp/bool/ua80 (ours/OCCT a80ba results), u20 (Remove Bohrung's unify).
   ITEM 4 (Remove Bohrung Ansaugflansch): Boolean 40 ms (OCCT 324); its unify (udump/unify-20, 57 -> 57) 137 ms:
   USD merges only edges and breaks the rod faces -> rejected 3x (usd.check 43, build 16) + patches 25 + bands 25 +
   op_unify check 18 (7ebcce87e adds ~15 ms here: 3 rejected tries; old path: final check dropped all). Cold feature
   profile (cli-fit, prof1.sh with ABSOLUTE cli path): kernel 214 ms = unify 111 + Boolean 99 (convert 54). Next: stop after the first reject when USD merged no face (edge-only), then profile the
   feature's other ~200 ms. Remaining SP3 evaluations (atp8.out): roots 3.9M, nearest 2.3M, area gauss 1.5M.
   A/B bins vibex8 ~/tmp/bool kt-a/b/c/d (delete after).
   2' Surface Pattern 3 = rec-cf-surface-pattern/fc1cdfae35b350aa (cut, 85 tools): boolean_with runs 3 rounds: round 0
     (0.6 s) finds 378 vertex groups (13 find_tol, 365 split points nearer than the band), merged to 116; round 1
     (0.18 s) finds 39 more splits; round 2 (0.8 s wall) classifies with 125 733 ray_state calls (2 s CPU). Tried:
     joining pieces across uncut own edges (one piece's rays for all): only -13 % rays (pieces are split by sections
     anyway), reverted (/root/bool-scratch/prop-full.patch). Perf: NurbsCurve::at 19 % + sincos 6 % + derivs 6 %
     (revolved free-form evaluation) in free::ray_hits; perf call graphs don't resolve on the 280 MB kt binary. Next
     ideas: round 0's split groups found transitively (round 1's 39 are pairs made near by round 0's merges); cheaper
     rays on revolved faces (profile curve + angle, not the NURBS surface).
   4 impeller Remove Bohrung Ansaugflansch 511/54: op_unify 393 (unify.reordered 224, patches 101)
   5 rings Circular pattern 2 364/149: ours trimmed 283
   6 impeller Boolean 1 271/67 (e), Extrude 3 171/20 (convert 81 = offset fit 67 ms, see above), Extrude 4 184/38 (prism_until split 122),
     Boolean 3 202/90 (unify 137), Extrude 1 118/23
   7 standoffs Revolve 2 232/27, Boolean 2 170/25, Boolean 1 156/26: thread x revolve, ours 263 ms vs OCCT 2.7 s,
     perf flat (NURBS evaluation spread): no single cut. 11-25: not e's pattern (stretches_over ~1000 projections a
     case, replay 120-190 ms each, brep 95-160, sections_ahead 26-61); NOSHAPE_PROBE_INV (probes-slow.patch) counts them.
   8 DONE (0c7a80c70, gating) sm-rocker Extrude 11. sm-rocker Mirror 3 (144 ms profile, live 8): spread, no one cut:
     7 Booleans 25 + unify 23, props_surface 28 (74 calls) + props_volume 20 (7), face_project_point 11 (145): next
     would be finding the props callers (model-core checks); skipped for now. (Run docs WITH their -wal: rocker.sh.)
   9 smaller: spur-gear Boolean 1 168/32, sm-rocker Mirror 3 134/8, helical-sweep Sweep 1 130/27, pulleys CP1
     112/23, tpd Fillet 1 114/28, nuts Sweep 1 152/67, valve Extrude 3 132/48 (prism_until split 120)
     Branch kernel profiles 11-25 18:40: helical 100, pulleys 95, tpd 115 (at-once try 45 declines, then one_by_one
     44), nuts 128 (< 2x live now). Spur-gear Boolean 1 = rec-cf-spur-gear/58f58c0e424cef4a (fuse, 186-face gear + 1
     tool, 242 faces): ours 88-139 ms (brep 51, build 31, check 29) + unify 66; flat perf: free::Nearest::of 28 %
     (uv_of's k-d tree seeds, 2550 of 2941 nodes a call): FIXED 517e18df2 on bool-gear2 (NOT in the running gate:
     gate it next): Nearest prunes on each range's box; replay 135 -> 108 ms; parity56/bool-corpus/corpus5/corpus3
     same, 0 differ; no gain on helical/nuts/pulleys/tpd (noise). Nearest::of now 1.7 % of the fuse.
   CHECKED: impeller Delete face 2 = core's P0 IMPELLER-DELETE-FACE-2-SLOW (ours fails 2.35 s, live Ok): not ours.
     workshop-split Thicken 1 = OCCT offset of a free-form sheet (op_thicken 484 ms, no own path): free-form, skip.
   NOW item 2 Surface Pattern 3 (replay ~2.6 s loaded): perf NurbsCurve::at 18 %. Probe (NOSHAPE_PROBE_AT,
     nurbs.rs probe_at + track_caller in curve2.rs, UNCOMMITTED, /root/bool-scratch/atp.sh -> atp4.out top 24):
     13.25M at() from curve2 roots() DIRECT sample evals (not bisection) = ray_hits_by called WITHOUT SpanBoxes
     (with boxes the kept sample points answer); nearest_sample 8.3M, sample_points 4.5M, chords 4.3M. Box-less
     callers: arrange.rs inside_of (~179, region inside point) and deeper (~231: builds spans, doesn't pass them).
     atp4.out names which. Fix idea: pass/keep SpanBoxes there (results equal: kept points = same evals).
   CHECK free x free?: impeller Delete face 2 1436/11 (OCCT defeature 1474), workshop-split Thicken 1 285/29.
  SKIPPED: free x free (impeller Fillet 3 (blend's too), Boolean Flansch+Gehaeuse, loft-paths Mirror 1 / Loft 3 / 4,
  knifehandle); parked (parting-line Split 3 / Draft 1-2 / Shell 1, rings Shell 1, Chamfer Stevenrohr, valve CP1);
  spread, nothing >= 100 ms (impeller Shell Gehaeuse 318/27); not Boolean (blend fillets/chamfers, min_distance,
  face_project_point, props_volume rows: bracket03 Chamfer 1, stef-radijator Chamfer 2).
  Cleanup: vibex8 ~/tmp/bool {cli-rel, models, prof, fl-work, p0 (after the P0), kt}; bool-p0 is an old worktree.

## How to run
- Worktrees: bool-hat (bool-pole; Mac builds, `bash .bin.sh <name>` inside rb.sh builds xprobe into
  ~/noshape-host/bool-keep/bin/<name>; uncommitted probe lines in crates/kernel/tests/all.rs, xprobe.rs, sampler.rs,
  the Shell 3 debug; never commit them), bool-tube (gate tree + bool-land7), bool-docs (handoff), bool-split (old).
- Gate: `systemd-run --unit=bool-gate-<sha>-x8 --collect -E HOME=/root -E PATH=$PATH -E NOSHAPE_GATE_HOST=<h>
  /root/bool-scratch/gate-chain-own.sh <sha>` (detaches bool-tube at the sha) -> /root/bool-hat-scratch/chain-<sha>.out
  ("VERDICT", "CHAIN DONE rc N"; rc 3 = CHECK only, e.g. knurls not run). Knurls: kn-s7.sh pattern. Private docs:
  /root/bool-hat-scratch/udoc-ab.sh <commit> pattern (bool2's udoc85.sh), copies mode 700, deleted after.
- Land: `git diff --stat $(git merge-base HEAD origin/main) HEAD` first; `NOSHAPE_LAND_QUEUE_HOST=vibe2
  tools/tiers/land.sh`; `tools/tiers/land.sh --status`.
- g++-13: /root/bool-scratch/gcc13-hat.sh <file.cpp>.
- Recordings: NOSHAPE_RECORD_BOOLEANS=<dir> on a CLI / parity run; replay `NOSHAPE_BOOL_CORPUS=<dir>
  NOSHAPE_BREP_VS_OCCT=1 NOSHAPE_BREP_WHY=" " <kernel test bin> brep_replay::brep_corpus_replay --exact --ignored
  --nocapture --test-threads=1`. Corpora: vibex8 / vibe2 ~/agents/bool-corpora, Mac ~/noshape-host/bool-keep.
- Probes (xprobe.rs): probe_case_timed, probe_case_sample, probe_case_nonmanifold, probe_case_unify, probe_shell_dump,
  probe_split_dump, probe_who_is_right. Debug switches: NOSHAPE_FREE_SECTIONS=0, NOSHAPE_OWN_SECTIONS=0,
  NOSHAPE_SPLIT_DEBUG, NOSHAPE_SHELL_DEBUG, NOSHAPE_BOOL_SLAB.
