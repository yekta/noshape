# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-22 05:53 UTC. Hosts by load (DECISIONS 11-19): read
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

## NEXT (lead 11-22 06:2x)
1. Land the in-flight stack in order: wclip 6329aaae -> ruling 1d8788b5 (queued); arcpart/seambatch/bandbatch (bandbatch
   gate DONE 06:15 vibe2 +0 -0, models 128 -> 128, dogbone noise only; knurls RUNNING 06:2x unit bool-kn-bandbatch ->
   kn-bandbatch.out, then delete vibe2 ~/tmp/bool/kn-bandbatch, `git rebase --onto e6e55ac74 cbab7cfe5` -> land);
   usdvol (gate vibex7 running -> knurls -> land).
2. Then P1 BOOL-PER-PAIR-TOL (lead 06:2x: gamma Boolean 1 ~24 s cold vs live's doc ~0.25 s; mixed-tolerance imports
   common): per-pair tolerances in crates/brep, STAGED (data model first: each entity's own tolerance carried in
   BrepBody / the Boolean; then classification / merging by the pair's two), each stage through the full gate.
   Two-stage fuse tried 06:15 and FAILS (bool-stages worktree, uncommitted; repro PROBE_FAR=1 PROBE_SUBSET=6,17,23,45
   PROBE_RUST=1 on vibex8 BK/gm/rec/f68f43939fd850eb: a far 2.45 um pin still breaks the 0.2 um junctions).
   PLAN (survey 06:3x): Body already has vertex_tol/edge_tol/face_tol (lib.rs:267) and GFace/GEdge.arg, Pool.pair is
   per-entity. Global `tol` sites: get_sedge (boolean.rs ~6135 arcs GREY*tol, ~6158 others 100*tol; joins two edges
   of ONE argument between the same vertices: suspected culprit, runs even with no contact), arc->line ~5101,
   split_params regroup ~4376, own-edge split ~6348, seams ~5309/5329/5348/6642; cross-arg: plane_plane, cyl_cyl,
   same_surface, lies_on, intervals_on ~1789, boxes_meet GREY*tol; C++ build() gives every out vertex max(tol, own).
   Stage A: arg_tol Vec in Ctx + band(a,b); within-argument sites. Stage B: cross-argument sites. Worktree bool-ptol
   (on bandbatch dfbc7eef1; rebase on main once the stack lands): PROBE_SAMEARG print in get_sedge, T1
   bool_patterns::precise_parts_fused_with_a_far_tolerant_pin (case d596e8faa25b6a23, gamma clips + pin 1 m away,
   OCCT 448 faces), run pt-probe.sh (vibex8) -> pt-probe.out.
   06:45 PROBE_SAMEARG: get_sedge's same-argument joins are all 0 apart (an edge seen from its two faces): NOT the
   culprit. In the far case object + 6 + 17 + 23 are still SEPARATE arguments meeting each other at 4.9 um, so the
   cross-argument sites matter too: split_params regroup (~4376: two split points within `tol` along a curve -> one
   vertex group; use the band of the two vertices' args), arc->line (~5101), lies_on / same_surface / plane_plane /
   cyl_cyl / intervals_on. Next: add arg_tol + band() in Ctx, env switches per site, find which one makes the far T1
   pass, then do them all properly (stage A within-arg, stage B cross-arg). A PROBE_NM port (bool-hat brep) shows the
   unbalanced edges. In flight at 06:45: wclip 6329aaae + ruling 1d8788b5 CHECKING; bandbatch knurls unit
   bool-kn-bandbatch (vibe2 -> kn-bandbatch.out); usdvol gate vibex7 (chain-bool-usdvol.out).
3. SPEED-STANDOFFS after.
4. Decision b: NO local check after the second group merge (150 ms of ~2 s is under the bar; changes a result).

## In flight (11-21 21:35 UTC)
- Mac: usable via ONE 4-job nice-19 slot (DECISIONS a16b39297; mac-run.sh / NOSHAPE_RB_HOST=mac wait for it), never
  overloaded; default builds vibe2 / vibex8. Runner: /root/bool-scratch/vibe-run.sh <script> (H=vibe2 default,
  WT=<worktree>; host script run-<basename>, so two jobs need two script names; scripts see BK=~/agents/bool-keep).
  rb.sh keeps mtimes: touch edited files on the host before an A/B (cyl-ab.sh / sc-cur.sh do).
- GEAR SPEED / SHELL-HANG: history in archive (11-22 06:2x move). Gear: sweepcap landed (236 faces as live); end cap
  parked (END CAP RULED below). SHELL-HANG parked (lead OK): blockers are Part 3's own input defects (a corner
  B-spline hooking 38 um back; a 16 um step face); probes sh-pairs.sh (vibe2 BK/shh/tools17).
- bool-wclip 59fb7a789 (from main a597c1393) gate DONE 03:55 vibex8: replay +0 -0 (differing 2 -> 2), suite 129 -> 129,
  shells 6 -> 6, speed 0.99-1.01x, crate tests pass; knurls 05:26 vibex7 0 diff x5 (copies deleted). QUEUED 05:36 as
  6329aaae (rebased: gpat's test and wclip's both appended to boolean_groups.rs; the first queue job b28b8ef9 had the
  conflict); bool-ruling QUEUED 05:41 as 1d8788b5 (ruling e6e55ac74 on 6329aaae9). The arcpart / seambatch /
  bandbatch stack still sits on the OLD ruling cbab7cfe5: at its land `git rebase --onto e6e55ac74 cbab7cfe5`.
  Unify of wire-clip's Boolean 2 (case BK/wc/rec/678f4bf258e12c94 on vibe2): 2.9 -> 2.0 s on vibe2, same 889 faces;
  merge_cylinder_groups: a round's merges (and circle swaps) in ONE ReShape, a group touching a face changed this
  round waits (set `changed`); move_seams_off_holes: a loop, faces tried once unless changed, ef once a pass.
  T1 boolean_groups::stacked_pins_unify_as_one_piece_pins. A/B: wc-ab.sh in bool-hat (PROBE_NO_BATCH=1
  PROBE_NO_TRIED=1 = old). Left: 26 seam moves each a whole-part Apply (~0.6 s vibe2): batch them per pass next.
- NEXT for wire-clip/gamma (bigger; our Boolean declines the fuse -> OCCT 1.9-2.4 s on vibe2):
  1. FIXED (uncommitted, in bool-hat AND bool-wclip crates/brep/src/boolean.rs c2_of `ruling`): an imported 1 mm pin's
     B-spline seam at th0 = -90° mapped to either side -> not taken as a ruling -> free::mapped ignored the seam copy
     -> both copies one curve -> "two edges leave a vertex in one direction". Fix: angles compared modulo 2πr.
  2. Next decline: "a piece's point on another solid's face" (same case; boolean.rs classify ~4486, fallbacks
     tangent_side / lies_on at ~7110). Probe PROBE_ON added there (env PROBE_DIR); run wc-dir.sh -> wc-dir5.out
     -> wc-dir5.out: several pieces' points lie within tol on other tools' faces, 1-60 um inside their bounds
     (cyl faces arg 22 idx 8, arg 12 idx 0; planes arg 10 idx 23, arg 3 idx 6), all at y -0.03253 / z 0.29499:
     tools touching face to face (coincident faces) not paired as shared. Next: why the pairing misses them.
     00:40 FOUND: piece = a 10 um strip of a pin's cylinder (arg 22) on solid 3's floor plane 144 / its tangent round
     156 (the pin dips 1-9 nm in). tangent_side took only plane pieces; with Cyl pieces too AND only faces whose bound
     holds the point (the round's surface past its bound gave mixed signs) -> In, decided (bool-hat switch
     PROBE_CYL_TANGENT, wc-ct2.sh). NEXT decline "coincident pieces differ": piece 535 (cyl arg 5 idx 36, a um sliver,
     OnSame plane 528 of arg 10 by lies_on) kept, its mate 630 (plane 528) dropped, edges differ but 1434
     ([1429,1434,1699,1746] vs [1427,1434,1668,1745]): a fillet sliver within tol of a flush plane. Reproduces as the
     pair args 5+10 alone (wc_pin_probe). 00:55: args 5 and 10 are identical clips 2.5 um apart in y, overlapping
     50 um in z, rounds r 4 mm: arg 5's bottom arc 1429 (2.5 um, 2e-10 off arg 10's plane 528) isn't taken as on that
     plane (edge_on_surface (Plane, Arc) needs the circle in the plane) -> 528's piece 629 an L with the tab under
     sliver 535. Trying (switch PROBE_CYL_TANGENT): a short arc within tol all along on a plane, its 2D curve the chord.
     Pin T1's volume: ours 1.5e-7 below OCCT's (both ways' cut empty: the integral, not the shape) -> band 1e-6.
     01:00 the short-arc rule didn't change it: the input edge (1311) is the round's whole quarter arc; only its first
     ~40 um is within tol of 528 (sedge 1429, split at arg 10's vertex 1929). Needs edge-on-face over a PART of an
     edge (OCCT's EF common block over a range): edge_in_face clipped to where it's within tol. PARKED (multi-hour;
     3rd sliver in a row). Candidate to gate alone: tangent_side for Cyl pieces + bound filter (bool-hat switch) ->
     does it convert any of the replay's 12 "piece's point on another solid's face"?
  3. Seam fix + T1 (bool_patterns::a_cylinder_seamed_by_a_b_spline, case 8a96c79ddef0cbf5.bin exact, recorded by
     wc_pin_probe with NOSHAPE_RECORD_EXACT; the text .brep did NOT reproduce) committed as cbab7cfe5 on branch
     bool-ruling (worktree, stacked on wclip 59fb7a789; bool-wclip's tree is clean again). Verified in bool-hat: old
     rule declines, new passes. gate QUEUED: unit bool-ruling-gate (ruling-gate.sh: after kn-gpat's KN-DONE and 15 GB free on vibe2) ->
     chain-bool-ruling.out; then knurls, land after wclip.
- bool-ruling cbab7cfe5 gate DONE 02:34 vibe2 (replay +0 -0, shells 6 -> 6, crate tests pass); knurls 03:43 0 diff x5
  (k10-25, k10-25-12, k25-15, k25-25, k25-71-12), copies deleted. READY: land right after wclip.
- WIRE-CLIP / GAMMA Boolean 2 (case 678f4bf258e12c94; vibex8 BK/wc/rec, BK/gm/rec), ours 352-439 ms vs OCCT 1.5-2.4 s.
  STACK (land in this order): wclip 59fb7a789 -> ruling cbab7cfe5 -> arcpart e4347de68 -> seambatch 22f3c0c54
  -> bandbatch ef793aaf3 (worktrees bool-wclip / bool-ruling / bool-arcpart / bool-seambatch / bool-bandbatch).
  - arcpart e4347de68 (amended 04:40): arc_end_on_plane + arc_part_in_face; chord of an arc across a plane only where
    its sagitta |p1-p0|^2/(8r) <= the face's tol; tangent_side for Cyl pieces answers only In (a cyl strip standing off
    within the band -> None: shfall 5e2cededd0026ac2, 40 nm off a slab side, OCCT keeps the plane; ours had 75 faces vs
    74 = arcpart's first gate FAIL 04:24; the other "changed" doc cf-sheet-metal-overcut-dogbone is the known NOT BUILT
    noise). T1 bool_patterns::clips_a_few_micrometres_apart.
  - seambatch 9cd43bf41: move_seams_off_holes batches a pass's moves (BandMove, band_seam_face, seam_vertex_face,
    apply_band_moves): seams 405 -> 50 ms. 22f3c0c54: merge_cylinder_groups checks valid_changed(copied, fast) (the
    faces not in the BRepBuilderAPI_Copy + shells_valid), 230 -> 21 ms; kernel T1 566 pass both ways; the local check
    differs from the whole only where the INPUT was invalid (louver fuse, hyperbola-edges walls). Unify 1.42 -> ~0.95 s.
  - bandbatch ef793aaf3: merge_cylinder_bands collects pairs sharing no face (a, b, faces round b's far circle) and
    applies them with one ReShape + one check (apply_band_merges; fallback one at a time): wire-clip bands 184 -> 120
    ms, unify ~920 -> 840 ms, same 889 faces; kernel T1 566 pass both ways (bb-all.sh, PROBE_NO_BANDBATCH in bool-hat).
    NB the kernel build has -Dnear= -Dfar=: no identifiers `near` / `far` in C++ (local syntax check: add them).
  - seambatch gate DONE 05:33 (vibe2): replay +0 -0, models 128 -> 128, shells 6 -> 6, crates pass; only the dogbone
    noise changed (gate log /root/bool-seambatch-scratch/gate-22f3c0c54.log). Lead told 05:50 (with decisions (a) gamma
    per-pair tolerance vs two-stage fuse, (b) local check after the second group merge: louver 1173 -> 1131).
  - GATE: unit bool-seambatch-gate (vibe2, 04:42, chain-bool-seambatch.out) covers arcpart + seambatch; unit
    bool-bandbatch-gate (bandbatch-gate.sh) runs bool-bandbatch's after it (chain-bool-bandbatch.out). Then knurls
    (kn-run.sh vibe2 kn-bandbatch, k-br = ~/agents/bool-bandbatch-gate tree), land the stack after wclip + ruling.
  - bool-hat probes: PROBE_CYL_TANGENT (arcpart rules), PROBE_NO_ARCPART / _TSCYL / _TSBOUND / _CHORD, PROBE_CYL_OUT,
    PROBE_NO_SEAMBATCH, PROBE_GROUPS_WHOLE / _CMP, PROBE_BANDS (5 band pairs merged one at a time, bands.bands 178 ms:
    batch them next like the seams), PROBE_TS (tol + surf). Scripts sb-ab.sh, gc-ab.sh, sb-prof.sh, sf-ab.sh, sf-ts.sh.
    vibex8's ~/.slot-quiet (qa) was left set after 04:00: scripts export SLOT_QUIET_OK=1 (lead: vibex8 free).
  - Unify left (vibex8): bands.bands 178, patches ~330 (its own groups call), usd ~200 (build 113), check 35.
  Boolean 1 (f68f, 87 tools) still declines "a non-manifold result" (gm-nm.out 04:47): 12 edges used once, at two
    spots: args 6 + 17 (6 mm line + two arcs on 17's cylinder near (47.8, -35.5..-29.5, 295) mm) and args 6 + 23 (two
    plane lines y -185.6..-37.5). gm-st.sh: PROBE_SUBSET="6,17,23;..." (bool-hat shell.rs bool_case_probe).
  - 05:30 gamma bisect (PROBE_SUBSET): 6+17+23 alone, and with tools 1, 2 or 24, ours; with any of 45, 67, 87 (the
    81-86-face imported pins) "non-manifold" with the same 12 edges. The Boolean's tol = 2 x the largest tolerance of
    ANY input (rust_boolean.cpp `const double tol = 2 * (std::max(max_tol, 1e-7) + fuzzy)`): a pin's tolerance
    widens the band for 6/17/23 far away. gm-tol.sh (PROBE_MAXTOL in bool-hat rust_boolean.cpp): 6/17/23 <= 0.2 um,
    29 tools 1.5 um, 17 tools 2.45 um (45 among them) -> tol 4.9 um. Boolean 1 is 23.8 s cold in the recording run
    (gm/par.out; 0 ms warm = cached, why qa's SPEED-GAMMA lists only Boolean 2). Fix options (lead): per-pair
    tolerances in the Rust Boolean (big), or the high-tolerance tools fused in a second stage.
  - patches: dfbc7eef1 (bandbatch) decides each pair on the part, copies only to build (no change in results).
    Dropped: the local check of the USD after the second group merge (150 ms) changes the louver test (invalid input:
    1173 -> 1131 faces) -> decision for the lead.
  - patches probe (bool-hat): patches.patches/.groups/.usd/.valid, and patches.copy/.check/.volume inside
    merge_cylinder_patches (a whole BRepBuilderAPI_Copy per candidate pair + whole check + 2 whole volumes per merge).
- SPEED-STANDOFFS st-brep.sh (in gm-st.sh): PROBE_BREP_TIMES stage marks in brep boolean_in (bool-hat, by line no.):
  Revolve 2's unify usd.check 328 ms is usdvol's; each Boolean's rust.brep 185-230 ms, sections_ahead 65-85 of it.
- SPEED-STANDOFFS 01:20: the unify's usd.check is 70-307 ms a Boolean (B-spline thread faces): after the changed faces'
  integrals, a change in the middle band (1e-12 bv .. 1e-9 most) integrates the WHOLE solid again (usd.volume 163 ms
  in the thread common de7a, where all 30 faces changed). Branch bool-usdvol (worktree, from main def9ef908): a
  FaceVolumes cache in faces_volume; volume0 = the check's integrals + the rest's, about the check's own point (the
  box's middle instead changed the change's noise 4x and rejected merges: keep the vertices' middle). Measure with
  st-usd.sh (bool-hat, PROBE_USD): same changes bitwise, de7a unify 272 -> 92 ms, e714 154 -> 86 ms. Committed
  f0929e9df; gate RUNNING on vibex7 since 05:42 (moved from vibex8's queue; unit bool-usdvol-gate, usdvol-gate.sh,
  chain-bool-usdvol.out); then knurls + land.
- SPEED-STANDOFFS: recorded (vibex8 + vibe2 BK/st/rec): 330d53143e91af3e cut 171 ms, de7a7e61c78a33a1 common
  276 ms, e714e22170ae3a0d cut 274 ms, all our path; a 6-face part x a 4-face thread sweep: brep 464 ms of which
  sections_ahead (OCCT IntTools_FaceFace on B-spline pairs) 216 ms. OCCT alone 1.4-2.8 s. Profile/st-prof.sh.
- END CAP RULED (lead 11-22 03:2x): not at 218 faces (face counts are parity; current 240 is closer to live 236). Take
  the exact cap's 2 s only if it also gives live's face split (aim 236: each gap's root kept as two B-spline halves,
  not merged r7.75 cylinders); else keep the current cap. bool-hat keeps it behind PROBE_NO_ENDCAP; low priority.

## From bool2
bool2's branches and parked tail reasons: docs/handoff/archive/bool.md (end, "From bool2", moved 11-21).

## Share and decline reasons
- Share (main 548d795fb's replay): ours 3121/3477 (general 3026, direct 95), OCCT 356, differing 6.
- Ranking (summed over corpora): not tried 212, non-manifold 42, short section from a corner 30 (parked), near two
  vertices 21, inside-out tool 17, tiny arc 12, piece's point on another solid's face 12, fuzzy / mesh 11, face's
  vertex off its plane 11. Outside shfall every reason is 1-4 distinct cases.
- shfall (122 declined; one group in the share): recordings from 11-02 (slabs at OCCT's nominal 1e-4 / 1e-5); no
  source change moves them (retolerance test: 12 of 92 taken, 2 volume differ). Tight ones:
  /root/bool-hat-scratch/shfall-tight.txt.
- Non-manifold (15 distinct outside shfall): 6 OCCT non-manifold too; cdd336b4 fixed on bool-pole; 4a71ecd0 / 62565707
  touching only; 301238ec / 23a4e56c corners on the wall with horizontal tori; 9f262529 / b0c36850 tolerant mirror twins.
- Looked at and parked (diffs in /root/bool-scratch): parked-shell-tight-slabs.diff, parked-stub-corner.diff,
  tilt-split.diff, parked-void-boxes.diff, parked-tiny-arc-micro.diff, parked-rnd-decline.diff, parked-cyl-cyl.diff,
  parked-own-foot.diff, torus-cylinder-wip.diff, offset-cache.diff, parked-cones-apart.diff.

## Next (after the order above)
- core 19:00 (FYI, if cheap): c2-parting-line-draft Draft 2 with a 64-sample parting-line sheet (core-pl, not landed)
  makes boolean.rust.sections_ahead 646 ms of 1057 (main's 128 samples: 92 of 552); vibex7 ~/tmp/core/plprof-16.log.
- Pulleys (lead 11-19, 152 ms vs live 23): not a gap (11-20 05:00). Mac Circular pattern 1 45-55 ms, document 75-90 ms
  vs live 102; vibex8's 152 was host load.
- P1 SPEED-STANDOFFS: its Booleans all ours on land7. Mac document (ops xshell3::xpul, XPUL_DOC) 455 ms vs live ~200.
  The unify's volume checks were ~40% (usd_kept_volume: faces_volume_precise, volume0): both on all threads in the
  faces' order (bool-sh3 d494b222e): ~345 ms. bool-sh3 a0b85fef4: 1f27fa8ca's segmented-integral
  rescue only where those and OCCT's agree on the solid to 1e-8 (it broke kernel tube-chamfer 34 / knurl grooves 9). uv_of's fold retry (11-20 06:20,
  counted): 1463 retries a regeneration, 847 end on the surface at the 7.7th of 16 candidates; an early stop saves ~10
  ms (3%): dropped (345 vs live ~200 is under 2x).
- qa's backlog (TICKETS a3ff6d481): standoffs Revolve 2 / Boolean 1 / 2 (OCCT 450-980 ms, ours "a piece's point on
  another solid's face"), wire-clip / gamma Boolean 1 (87-tool fuse, "cylinders nearly coincident", OCCT 2-2.9 s) and
  Boolean 2. Cases Mac bool-keep/rec-<doc>.
- core's nut thread cut (c2-config-parts-nuts Sweep 1; Mac bool-keep/nuts): crest on the bore; waiting on core for a
  tool reaching into the hole; live 135.268 mm3 / 29 faces.
- c2-fan-blade-sweep Boolean 2 (P0 earlier, bool2's free-form side now mine): offset-surface evaluation is OCCT's 186
  ms; when ours takes it, narrow offset_grazes_a_circle (check rings-loft, sp-1113). Case Mac bool-keep/fan.
- Re-win the 50 um rule's declined cases (list above).
- c2-rings-loft Shell 1 (~550 ms vs live 235), learning-center-michael (cyl x torus), cf-sheet-metal-shaped-flange
  corner reliefs, rings-loft Circular pattern 1's splits (decision with the lead before touching features.cpp).
- DECISION ASKED (lead, 11-18 22:00): every Boolean's check as made_valid (changed faces + shells) instead of
  BRepCheck of the whole result (pulleys' 5-tool cut 13.1 -> 6.2 ms).
- Parked (lead): 129100d95, 846db3d0c, 13b8c4b8d's same-surface merge (no live capture).
- P2 (lead 11-20, TICKETS T2-NIGHTLY-22-FAILURES, vibex6 nightly T2):
  - corpora on vibex6 (11-20 03:00): ~/agents/bool-corpora/shellcorpus (422) and bool-corpus (1949); bool-ho's tests read
    $HOME/agents/bool-corpora/... (shell_corpus_time takes every case when `taken` is absent, faces 1-based); the
    knurl / gear tests to 1e-6 relative (bool-ho 5cfc4f676, queued with 42635088a 04:12). a_10_by_71_knurl (1e-7) read
    1.1e-7 off on the Mac binary: to 1e-6 too if the nightly shows it.
  - overlapping_free_form_parts_merge_like_onshape: FIXED on bool-sh3 1f27fa8ca (gate with sh3): the fuse is ours now;
    the unify's USD merge to live's 15 was refused (OCCT's integrals 4e-9 off on our B-spline bounded cylinder pieces,
    the segmented ones 9e-10); a change within 1e-8 of the box's volume is now decided by the segmented integrals of
    the changed faces (62 ms there). Mac u32: the test passes. Debug in bool-hat WIP: NOSHAPE_USD_DEBUG / _FACES.
  - a_25_by_10_knurl_at_30_tpi: FIXED on bool-sh3 d2a26a12a (gate with sh3). Declined since 4ce48b9a0 (our own
    plane x B-spline sections; bisect over crates/kernel + brep, /root/bool-scratch/bis.log): the end face meets each
    flank's 9.6 um section through the rim 3.3 um before its end, read as "a section touching a face's boundary"; a
    touch within the section's tolerance of its ends is now that end's pave. Ours again, 1.8 s.
  - knurl_b94_at_30_tpi_on_a_10_mm_rod (>15 GB): alone it passes, ours, 46 s (Mac, bool-hat); in the nightly it ran beside
    knurl_feature_b94_tooth_and_helix_angles (both >60 s in t2-2026-11-20.log) and the budget is the process's growth:
    the B94 knurl tests now take one lock (KNURLS, bool-ho 42635088a). Its Boolean recorded: Mac bool-keep/b94/e83dc201f84a455f.

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
