# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-15 20:30 UTC. Read docs/DECISIONS.md first; history is in docs/handoff/archive/skasm.md (read it
only when a line here points there).

## In flight
- Worktree /root/projects/noshape-wt/skasm (detached HEAD on main), not on main yet (landing 20:3x):
  - "parity: a sketch constraint on model geometry that over-defines the sketch is taken back": live's 14:58 refs
    (19:28) made exercise-chino's Sketch 3 (a slot's ends Coincident to a model line) and cf-spur-gear's Sketch 2
    conflict; now 14/14 of those documents match to the end and their dropped constraints went 33 -> 25.
  - "sketch Offset takes a model face" (live q-off, behaviours "Sketch Offset (O) on model edges"): op
    `sketch_offset` takes `face:…` (its outer loop as one chain), the app's Offset tool takes a hovered face
    (SketchEditor::offset_loop), out of the face by default (offset_out_of_face). e2e
    sketch_offset_of_a_model_face_offsets_its_outer_loop. sketch_use gained outer_edge_refs (edge_refs unchanged).
- Next: sheet metal "Face of … / Vertex of …" labels after live's rule (below).

## Waiting on others
- live2 (REQUESTS 19:56): every 8020 10 Series section's end-face loops (live/skasm-q1956/). Ours is an
  approximation (1001: ~520 mm² vs live 329.364; live has three closed 8-edge slot cavities + the bore = 5 loops, so
  End cap Internal fails there with "Only frame segments with one internal profile can be capped."; ours has 2 loops
  and builds). Rebuild frame_library.rs `tslot` from it.
- psui (4d0a82b2-8fc7-43e6-b45d-bd8cb225741e): tab-new-edit-reedit is a pick difference, not naming: live's click at
  (30, 35, 2) hit JrS = Flange 1's outer wall (30, -2, 14.5); ours the plate. Tell psui with the next report.
- Not mine (handed): knife / impeller geometry (core); fan-blade Boolean 2, gamma / wire-clip Boolean 2, shaped-flange
  Booleans, surface-pattern sphere cuts (bool); setback fillet (blend); hex-bookcase mesh Uses (dx).

## Sheet metal labels (live2 16:27 answer, live/skasm-q1627/)
- Live's rule: a face / vertex reads "… of <the feature that FIRST created its sheet metal definition entity>" (not
  the last to modify it). sm-box at (c): plate faces Sheet metal model 1, Flange 1's wall + bend 1 Flange 1, bend 2
  Flange 2, the 4 mm² corner face at (59, -1, 4) Flange 1, vertices (60, -2, 4) / (60, 0, 4) Flange 1.
- Ours: the corner face is named Flange 2 (fresh in Flange 2), and a vertex reads its faces' latest creator
  (part-studio-ui part_studio/mod.rs labeler), which includes bend 2's cylinder, so "Vertex of Flange 2".
- Plan (no persistent names change): every part entity carries its definition entity's SMAssociationAttribute,
  whose attributeId is a transient id ("JAB": a base-26 count running on across the regeneration's features). Record
  each custom feature's transient base in Context (host/mod.rs where transient_base is set), decode the attributeId to
  the feature that handed it out, store entity name -> that feature on SheetMetalData (serde default), and have the
  labeler use it for sheet metal faces, edges and vertices.

## Open questions (none blocking)
- s236 (fuzz probes 11): live solves three linked quads at once; per-case placement rules don't converge. Decision
  for the lead if fuzz work resumes there. s228 has its rule (grow_slot); s226 is 3 µm off (live's anchor drifts).
- Jog at its reduced angle: our flat lays two bend strips 0.73 mm over each other (features_c
  jog_preserve_material_off_and_errors' second case; live's doesn't): the jog's definition has two middle faces 0.79
  mm apart, so coplanar_rip doesn't join them.
- Sheet metal "Collision in sheet metal flat pattern." INFO for zero-gap contact at a tear (config-assembly 08590a's
  Flange 1 … Mirror 1): needs a contact rule, not an area check.

## Next, in order
1. Sheet metal labels (plan above): psui's bend-relief-* (5 takes) and tab rows.
2. 8020 sections from live2's 19:56 capture.
3. Knife's own sketch rows: Sketch 17's Concentric is on a co-edge query whose edge is a spline (a near-circle,
   r 2.5) and whose face JiS is a cylinder about Y (the Front sketch's normal): live is concentric with the
   cylinder; ours rejects (needs a cylinder-axis reference). Sketch 12's 5 Uses (blade B-splines), Ic / Id; PIERCE
   (tried and reverted 12:53: knife Sketch 13 moved 11.8 mm).
4. A full suite (vibe6, ~/bin/slot) for the new dropped total (83 at 16:10; the 14 refreshed documents 33 -> 25).
5. Fuzz P2 random cases (769 of 830, live/fuzz/skasm-sketch-probe-*.json).

## Current numbers
- Model suite 16:10 vibe6 (suite-1610.log): 76/87, 881/947 (the 4 Hole documents lost were studios' 8aea0c3c4, fixed
  253c08ee: 80/87 expected). Sketch constraints left to the solved geometry: 83; with live's 19:28 refs the 14
  documents re-run (pm14c.log, ~/tmp/skasm/pm14 on vibe6) went 33 -> 25.
- Sheet metal speed, quiet vibe6 19:00, ops binary of 17:43 (ours vs live ms): corner-break 1637-1782 / 1972,
  shaped-flange 809 / 1026, overcut-dogbone 210 / 270, hem 65 / 95. All four are faster than live; the suite's slower
  figures were host load.
- Exchange: model-plus STEP / IGES import in live as 58/151/94 (6f5b151e snap_coplanar); dx "live imports ours" 43/48.
- Sketch fuzz vs live: 769 of 830; probes 20 11/11, probes 11 s236 open.

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
