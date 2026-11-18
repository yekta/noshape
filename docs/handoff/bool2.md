# bool2: current state (11-18 09:12 UTC)

Second agent on noshape's own Boolean (crates/brep, crates/kernel/cpp/rust_boolean.cpp, boolean.cpp's unify). A case
is taken only where it equals OCCT (or live, with a capture); results identical end to end (docs/DECISIONS.md 11-14).
History: docs/handoff/archive/bool2.md. Rulings: docs/DECISIONS.md (read it first).

## Unlanded work (11-18 08:45 UTC)
### Stack bool2-xscan = c5a70b8c5 (c26so e62420bb2 + 0d9abe23e + c9c3759aa + xscan 728c3d989 + usd fe3d4411f + warm
start bd0f00fc6 + 9c7f1e399 and its revert c5a70b8c5), worktree bool2-xs
- gate71 (unit bool2-gate71, vibex7, gate71.out, from 09:09), udoc12 hats/dock (unit bool2-udoc12, vibex7,
  udoc12.out), land unit bool2-landxs2
  (land-xs2.sh: gate71 PASS + udoc11 SAME -> land.sh, queue vibe2, all.rs out and back). NOLAND -> fix, re-gate.
- gate70 (c9c3759aa) FAILED: impeller 29 -> 27, cf-surface-pattern SP3 129 -> 116 (live 131), sp-cut 990256f0 74 vs
  77. Kernel bisect: 9c7f1e399 (drop_same_regions: a B-spline group 4 -> 1 with the same boundary edges within 1e-7
  but area 6e-7 larger skipped the volume check). Reverted (c5a70b8c5): sp-cut 77, impeller 29/48, SP3 129/386/259 =
  main. Revolve 2's unify speedup (200 -> 45 ms) is lost; redo with an exact test (pcurve-exact / area-equal) later.
- Gate68's shfall diffs and k25-15 slowdown fixed by 0d9abe23e; cf-spur-gear's by c9c3759aa (main 5738, c9c3759aa 5774 ms).
- Chamfer 26 (bool2-ch26x): stays gated. Fillet 3 ERROR after it is blend's (lead 03:30; our 33-face body = live's;
  dumps vibex8 ~/agents/bool2-keep/fillet3). f3bd95bc (impeller, OCCT chamfer 26's sliver) waits for our chamfer 26.
### Fan-blade fix: branch bool2-fanb2 = ebc58e0be (on c9c3759aa), worktree bool2-ab
- c2-fan-blade-sweep Boolean 2 (3e8c1e09) ours: see Gate exceptions. Gate it after the stack lands (rebase), vibex8.
  Then tell bool (narrows offset_grazes_a_circle) and core (nuts3 lands after it). Rebase on bool's bool-shared2 when it lands.
- Nuts (bool's hand-off, core's Sweep 1): ebc58e0be already takes bool's recorded nutcrest cut (135.2676 mm3, live
  135.268; OCCT 143.88 removes nothing); in the document (ops-fanb2) Sweep 1 ERROR -> OK with live's counts (one
  cylinder face is a B-spline: core's input). core's nuts3 T1 (59df02227) passes with and without the fix (vibe2):
  without, ours declines and OCCT cuts (570 ms); with ebc58e0be ours (105 ms). Told core 08:52 (old thread) and again its new thread e8af89fe (nuts3 needn't wait); tell e8af89fe when ebc58e0be lands.
### Exact unify drop: branch bool2-usd2 = 017fa3a86 (on bool2-fanb3 f9f8c11ff = ebc58e0be rebased on c5a70b8c5)
- drop_same_regions again, exact: a group (one non-closed free-form surface, handle + placement + orientation) is
  left out of the volume check only when its edges around use the very same pcurve handles over the same parameter
  stretches (joined where contiguous) on both sides. sp-cut 990256f0 77 faces (main); standoffs unify inputs (vibe2,
  5 rounds alternating, load 22-30) 88 -> 45, 236 -> 139, 103 -> 40, 212 -> 122 ms, identical faces/edges/volumes;
  Revolve 2 (usd-2/5, 250 ms) not dropped: USD approximates one merged end curve (two B-splines -> one) and splits
  the long iso-lines into new pieces. Kernel bool_/usd/unify tests 147 pass; fanb3: bool_ 140/140, brep 62.
- Gate fanb3 + usd2 together (017fa3a86) after the stack lands; scripts scratch/usd2.sh, usd2d.sh (BOOL2_DROPDBG).
- 11:15: gate71 VERDICT CHECK only for the knurls (vibex7 31 GB; replay 0 new differences, suite 90/90 and 1283
  features, crate tests pass, speed 1.02-1.05x); udoc12 13 tabs SAME; knurls ours-only main vs branch (unit bool2-kn71,
  vibex7 ~/tmp/bool2/kn, outside the slot: core's pair7 holds it); land unit bool2-landxs3 waits for it.
- 09:09 (lead): gate71 moved to vibex7 (main cache hit there), hats/dock as udoc12 on vibex7 (udoc-x7.sh: cli-base
  copied from vibe2, cli-br built in vibex7's slot, private docs mode-700 copies deleted on exit); landxs2 re-armed on
  gate71 + udoc12. vibex8/vibe2 entries cancelled (no tickets, vibe2 copies deleted).
- Scratch (uncommitted in bool2-ab): xprobe probe_case_box_edges / probe_case_unify_both / probe_case_diff; debug patch
  fanb2-debug.patch.

### bool2-c26b (39917ca1d, on main b87dd3dcf)
- c26 without chamfer 26: d082638e4 Illinois ray-crossing roots (sphere cuts 4-5 % faster), d267d27d3 + 5910f9967
  sprockets (Extrude 2's cut OCCT 169 ms -> 30-45, Circular pattern 1's 382 -> 80-97, as OCCT), 0008db3a2 standoffs'
  thread cut 5608f662 (same_surface one way), 39917ca1d gate.sh locks a host by `ssh $host hostname -s` (lead 11-17).
- Gate66 (old bool2-c26 with chamfer 26) FAILED: shfall commons 5fb3d16c, e8d9bacb, 3748a133 taken but an input edge
  Deleted where OCCT says Modified; c2-impeller-loft's Fillet 3 ERROR. Both chamfer 26's (now bool2-ch26).

### Branch bool2-so2b (f88694662, rebased on bool2-c26b 39917ca1d): standoffs' three thread Booleans
- Gated with c26b as bool2-c26so (above).
- 12d98f285: GFace::uv_of retries Newton stopped on the box edge off the surface from the nearest sample of each of the
  8 nearest rows each way (the root strip: 17 x 65 samples, 5 a turn, turns 0.5 mm apart). edge_in_face's trim past
  a free-form face's edge cuts where it comes nearest that edge (nearest_to_edge) when within the edges' tolerances;
  stretches_over / edge_crossing for an edge leaving the surface between its ends.
- f88694662: edge_over_free_face: an analytic face's edges go into a coincident free-form face in stretches (the
  bore's seam and end circles across the strip each turn); a distance bound skips projections; a sliver's side of a
  coincident face's boundary by points inside its edge (an end on a closed edge's vertex was read as either end).
- All three ours: cut dd85b48f 28/71/45 vol 1e-12, cut 5608f662 45/120/77 vol equal, common 2a11cb1b 30/83/55 vol
  1.1e-6 rel (gate-equivalent: 2.1e-14 m3 vs allowed 2.1e-11; two section edges' midpoints differ, same ends);
  history identical for the cuts. Alternating A/B (then vibe5, 6 rounds, load 31), ms, ours+fallback as the app:
  dd85 c26 1260 / so2a 151 / so2b 260 / OCCT 1133; 5608 154 / 186 / 285 / 1367; 2a11 2122 / 2288 / 336 / 1960.
  Lead 11-17: land as is; cuts skipping the bore-edge stretches is a later speed item.
- T1 (bool_patterns): a_thread_root_strip_cut_where_a_circle_crosses_its_run_out, a_circle_grazing_a_thread_run_out_
  edge_is_cut_where_it_crosses_it, a_thread_root_strip_on_a_bore_split_at_its_seam_each_turn (data in
  tests/data/booleans; copies vibex6 ~/agents/bool2-keep/so-rec2). Kernel 493/493, brep 62, clippy clean.

### Chamfer 26: branch bool2-ch26 (f8d871570 = 9545d4a03 on the old bool2-so2b 532da2deb; rebase later)
- The own-neighbour rule (a piece on another solid's face takes its own-solid neighbour's states across an edge with
  no section and no other solid's user) is now only for a cut's tool pieces: the shfall commons decline again as on
  main; chamfer 26 still ours (36/95/62, 1.2e-10). Kernel 494/494.
- But the impeller's Fillet 3 still ERROR with it (parity on c2-impeller-loft alone, vibex6: 33/86/54 vs live
  34/95/62). Chamfer 26's records: tool edges 7 and 16 Deleted in ours, Modified in OCCT (output edges at (1.5,
  +-2.759, -43.05) mm) = the two slivers (pieces 37, 45) the own rule calls Out; in the shfall commons too OCCT keeps
  the sliver's own edge. Experiment (uncommitted in the bool2 worktree): BOOL2_OWNIN (own rule gives In), BOOL2_OWNALL
  (commons too). Mac run 20:50 (ch26-mac.out): "In" makes chamfer 26 decline (non-manifold -> OCCT), so the impeller's
  Fillet 3 goes back to main's 34/93/60 (live 34/95/62): no fix, it only undoes chamfer 26. OWNALL: commons decline
  non-manifold too. Dropped.
- Mac vs Linux (lead 11-17): found and fixed. own_sections' crossings2d (rust_boolean.cpp, our C++; OCCT's
  Geom2dAPI_InterCurveCurve) found the profile of arg0 face 3 crossing the tool's circle (cyl faces 37/38) twice
  0.3 mm apart on Linux, once on macOS (shallow crossing at u 0.99999127), so face 3 x 37/38 had no section there.
  Fix: branch bool2-xscan (728c3d989 on bool2-c26so, with T1 a_profile_crossing_a_cylinder_twice_close_by_is_cut_at_both: fails on macOS without the scan, passes on both with it, worktree /root/projects/noshape-wt/bool2-xs): the B-spline's
  signed distance from the circle / line scanned (64 a span), missed sign changes bisected. Mac now 36/95/62 with
  Linux's volume to the last digit; Linux unchanged. Gate69 = xscan + usd + warm start (bd0f00fc6; unit bool2-gate69, vibex6, starts when gate68 ends).
  Debug dumps (BOOL2_DUMP: pool, split edges, pieces, states, sections, OCCT/own sections):
  /root/bool2-scratch/ch26-plat-debug.patch; scratch-docs/ch26-dump.sh runs it (Mac and vibex6).
- Tried and dropped: OCCT's ComputeState (an own edge's middle): those middles lie on the other face too.
- Debug patches: /root/bool2-scratch/ch26-history-dbg.patch (BOOL2_EH=arg,idx: an input edge's pieces, get_sedge
  merges), the DBGOWN prints (in git history of this session's experiments only).

### Both
- Uncommitted on purpose: crates/kernel/tests/all.rs registers the scratch probes (copy /root/bool2-scratch/all-with-
  probe.rs), crates/kernel/tests/{xprobe,sampler}.rs, scratch-docs/. Take all.rs out before land.sh, put it back after.
- Landed 11-17: bool2-chamfer as 3844e0a5 (valve Extrude 3 own sections, unify reordered tries).

## Model-suite speed list (lead 11-17): where each stands
- c2-loft-paths-guides Mirror 1: input differs (Loft 3 17 vs live 15 faces, core); Mirror 1 Ok but wrong (13/33/25,
  5.27e6 mm^3 vs live 27/67/47, 6.33e6): OCCT's fuse with the mirror (2.75 s; ours declines "an edge without its curve
  on a free-form face"). Told core. Case (were on vibe5, lost: record again) ~/agents/bool2-keep/lp-rec/61b92e93c065aeed.
- c2-parting-line-draft-parting-line Split 3: 1829 -> 599 ms on main (core's 85010cc40) but still "does not split"
  (live 13/34/24 x 2): the tool runs through the body's parting edges, which our drafted B-spline walls put a few um
  off it (body 25 mm^3 under live's). OCCT's Splitter with fuzzy 1e-5 splits (13/49/39 + 17/53/39). Draft is core's.
  Dumps (were vibe5 ~/tmp/bool2/spd, lost) (split-2-*), probes probe_split_pair / probe_split_ab / probe_split_fuzzy.
- c2-surf-workshop-knifehandle Split 1: 971 -> 98 ms on main (core's fix), live 7; OCCT's Splitter ~50 ms of it. A
  half-space split by our Boolean (extrusion surface swept across) isn't faster: shelved,
  /root/bool2-scratch/split-extrusion-halfspace.patch.
- c2-config-parts-sprockets: Extrude 2's and Circular pattern 1's cuts ours on bool2-c26 (0ffab0d52, bd9710453).
  Cases (were on vibe5, lost: record again) ~/agents/bool2-keep/spr-rec.
- c2-config-parts-nuts Sweep 1: bool's (nuts thread turn); c2-sm-rocker-enclosure-body: input differs at Extrude 6.

## Valve Circular pattern 1 (case rec-valve 52970b93111ac42b, ours)
- Cone pairs: the part's 60 deg ring (z axis, r 70 at z 0, two halves u 0-90 / 90-360 deg) x 8 hole cones on parallel
  axes at 71.3 mm (r 1.5 at z 3, -30 deg, 2.3 m long). 9 of 14 pairs have curves; bool's parked-cones-apart.diff
  (mine now, /root/bool2-scratch) proves none of the 5 empty ones (they reach the half's boundary angle): dropped.
  Only lever left: own parallel-axis cone x cone sections, matching OCCT's splits (bool's cyl x cyl parked on that).
- Raw cut: ours 47/143/94, OCCT 47/142/93 (OCCT merges two crossings 36 um apart into one vertex; ours keeps a 71 um
  edge). The document's part after the pattern: 46/122/72, live's exactly (vol 46868.68 vs 46868.59): ours is right.

## Other
- Impeller 38073a30d2ffbc3e (Boolean Flansch+Gehaeuse fuse): ours 20/57/40 raw, 5.913847e-5 m^3; live 19/56/39,
  5.913789e-5; OCCT 16/47/36, 5.911132e-5 (27 mm^3 off live): ours is right (told bool 11-17). 02f2f58d / 7cb79697 by
  an edge after the replay's unify, not looked at. Cases (were on vibe5, lost: record again) ~/agents/bool2-keep/imp-cur.

## Agents
- Roster (lead 11-17 10:30; only these ids): psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (11-17), core
  e8af89fe-d460-49b3-b0bb-9572da230fed (11-18; 6692ae35 stopped), blend 0672fa20-964d-4fde-a19c-c5b4363729ba (11-18; 0e6e9942 stopped), bool 7fa8ceab-4549-499f-9b01-
  3a289c2c95b4, dx f93a1292-5bb7-406f-9a25-59767e01878e, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, asm d8d597e0-675c-
  497b-a804-544ceab99bbd, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0,
  studios 58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-
  b43a-da61a4f92ad7 (11-17).

## Gate exceptions (lead 11-18 07:15, this case only; the gate's 1e-9 stays for everything else)
- 3e8c1e09636d4fd7 (c2-fan-blade-sweep Boolean 2, base cut by two bolts; ebc58e0be, T1
  bool_patterns::a_thread_crest_through_countersink_rims_cut_as_occt at 1e-8): ours 87773.867193 mm3, OCCT 87773.867457
  (3e-9 relative). Same 66/174/106, identical history (probe_case_records), every face's area within 1e-6 and centroid
  within 0.1 um; the gap is on the countersink cones, whose OCCT sections are 5 um-tolerance curves (offset surfaces).
  Live (bool 11-18, suite-0545 #48, /root/onshape-research/live/suite-truth-1118/c2-fan-blade-sweep.json): base
  56/143/88, V 87773.8660 mm3; ours/OCCT after unify 56/139/84, 87773.867 (1.4e-8 apart). Live's 4 more edges and
  vertices: created faces 2 B-splines (deg 1x2, 3x121 poles) 60.3612, 2 cylinders r 3.2 31.8332, 4 other 55.2344 /
  60.3844 x2. (The earlier 87773.636 was wrong.)

## Open decisions (with the lead)
- None open. parting-line Split 3 went to core (11-17).

## Next items, in order (lead 11-17)
1. Land bool2-c26so (gate68 + hats/dock udoc9; 5608 at 285 ms accepted, cuts skipping the bore-edge stretches is a
   later speed item).
2. Land bool2-xscan (gate69). Then chamfer 26 (bool2-ch26): its history (tool edges 7/16 Deleted, OCCT Modified) and
   the impeller's Fillet 3 ERROR remain.
3. Standoffs (dx 11-17): on c26so all three thread Booleans are ours (32/32, doc 13.5 s -> 3.7 s, hosts differ). Unify
   check speed: fe3d4411f on bool2-xscan (faces_volume per face on all threads, bit-identical sums; the fallback
   passes reuse the memo): Revolve 2 421 -> 237 ms, Boolean 1 204 -> 78, Boolean 2 503 -> 193 (vibex6 alternating,
   probe crates/kernel/tests/usdprobe.rs uncommitted in bool2-xs, inputs vibex6 ~/agents/bool2-keep/usd-stoff,
   scratch/usdab.sh; debug build /root/bool2-scratch/boolean-usd-debug.cpp). Left: Revolve 2's check counts all 30
   faces changed: fixed by 9c7f1e399 (faces on one free-form non-closed surface whose merged face has the parts'
   boundary left out of the integral; Revolve 2's unify ~200 -> ~45 ms, same results, kernel 506/506). Our Boolean
   (standoffs cases, 1 thread, vibex6: dd85 238, 5608 263, 2a11 303 ms): samples ~25 % stretches_over -> edge_crossing
   -> nearest_to_edge -> seg_param (Nurbs param_of, 70 a crossing), ~30 % occt_ahead (IntTools_FaceFace, its
   ComputeTolerance PSO). Tried: edge_crossing's min_by keyed once: no gain (dropped).
   Lead 11-17: keep the bore-edge passes (no skip for cuts; dd85's test stays at 1e-9). Warm start: bd0f00fc6
   (nearest_to_edge's golden steps find the B-spline foot from the last one's, NurbsCurve::param_from): 10-13 %
   faster (dd85 422 -> 379, 5608 452 -> 399, 2a11 533 -> 462 ms loaded), counts/history same, volumes within 1e-11.
   Tried: stretches_over's samples seeded from the last on-surface uv: bit-identical but no gain (dropped).
   A/B binaries vibex6 ~/tmp/bool2/ab (k-old = 9c7f1e399, k-warm = bd0f00fc6), scratch/boolab.sh, warmchk.sh.
4. (loft-paths-guides Mirror 1: moot, core's f824d120 reverted the loft change; back to live's volume.)
5. Sprockets end to end (document A/B) after the land; the next documents from qa's speed report whose inputs match
   live (re-run speed-gaps.py on the newest suite).
6. Parked: valve cone x cone sections (OCCT's splits), helical Sweep 1 (cylinders in analytic_freeform_sections give
   one curve where OCCT gives 10, slower), impeller Extrude 4 (OCCT internals).

## Recent facts
- qa's P0s: helical Sweep 1 0.4.48 -> 0.4.53 2.2x slower = 0.4.53 lacked 895c8e3b (fixed on main; vibe7 same run:
  0.4.48 341-557 ms, 0.4.53 873-977, main 257-326). Dock tab B Puck Huni: fixed by bool's 7ac4debfa + b27a78c40.
- core's housing unify: Rotieren 1 / Boolesch 1 / Bohrung 1 each a USD merge rejected at 4.9e-15 m^3 vs 1e-9 V
  (borderline), three memo-hit tries + one volume: no identical-result cut without moving that threshold.
- gate.sh landed: builds through ~/bin/slot with SLOT_MIN_GB=12 (19afd619), models snapshot per run (a2952b32).

## How to run
- Builds, crate tests, probes: vibex6 again after gate68 (lead 21:23; vibe2 only finishes gate68 + udoc9): `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c 'export CARGO_BUILD_JOBS=4;
  cargo test --release -p noshape-kernel --test all <f>'`; rb.sh drops env prefixes (env inside `bash -c`). Don't
  rb.sh from a worktree while a binary another unit still needs is being built from it (rb.sh resyncs the sources).
- Gate (vibex6, one of mine at a time; never kill a process tree from a computed pid, stop units with systemctl): `systemd-run --unit=bool2-gateNN --collect -p WorkingDirectory=/root/projects/
  noshape-wt/bool2 bash -c 'bash tools/bool/gate.sh <sha> --jobs 4 --host vibex6 > /root/bool2-scratch/gateNN.out
  2>&1'`. hats/dock: /root/bool2-scratch/udoc-v7b.sh <branch sha> (cli-base on vibex7 must be the branch's main).
- My own suite pairs: through ~/bin/slot on a `cp -al ~/parity/models` snapshot (NOSHAPE_GATE_MODELS for gate-host.py,
  DECISIONS 11-16); model files change under long runs otherwise.
- Land: clean gate + main green (`/tmp/lead/main-status`), all.rs out, `NOSHAPE_LAND_QUEUE_HOST=vibe2
  tools/tiers/land.sh`; `--status`.
- Probes (crates/kernel/tests/xprobe.rs, `--ignored --exact xprobe::<name> --nocapture`, PROBE_CORPUS=<dir>
  PROBE_CASE=<full id>): probe_case_repeat (PROBE_N, NOSHAPE_KERNEL_PROFILE=1, PROBE_SAMPLE=<file> -> /root/bool2-scratch
  samples.py, callers.py), probe_case_volumes (ours vs OCCT). DBG_AHEAD per-pair OCCT section
  times: /root/bool2-scratch/valve-ahead-dbg.patch. NOSHAPE_OWN_SECTIONS=0 turns our own sections off for A/Bs.
- Recordings: vibex6 ~/agents/bool2-keep/{hel-new (helical 9b68aa2b07dc2986), imp-new (impeller f10ce62e55f0fd49,
  f37371b831c5339f), so-rec2 (standoffs' three)}; vibe5's (rec-valve, spr-rec, imp-cur, lp-rec) are lost.
- Doc timing: qa's saved docs vibex6 ~/qa-parity/saved (copy + -wal/-shm each run); `noshape-cli feature list`,
  regen_ms; under load use user CPU (`/usr/bin/time -f %U`, RAYON_NUM_THREADS=1). Private docs (hats, dock): mode-700
  copies in ~/tmp/bool2, deleted after (udoc-cmp.sh does it). gdb only on a child (ptrace_scope 1): `gdb -batch -ex run
  -ex "thread apply all bt" --args ...` under `timeout -s INT`.

- qa 11-17: in the clean A/B (vibe7, load at most 0.06, v0.4.48 to v0.4.53, median of 3), c2-helical-sweep Sweep 1 took 157 ms on 0.4.48 and 396 ms on 0.4.53 (Ok in both). Your 895c8e3b / e47268f2 are meant to fix this on main. The 11-18 nightly pair (v0.4.55 vs v0.4.56, alternating) will confirm or reopen it.
