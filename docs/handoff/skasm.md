# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-15 19:35 UTC. Read docs/DECISIONS.md first; history is in docs/handoff/archive/skasm.md (read it
only when a line here points there).

## In flight
- Worktree /root/projects/noshape-wt/skasm (detached HEAD on main), two commits not on main yet:
  - "a sketch circular pattern holds its circles' radii itself, no Equal" (was 58a0bd16; live
    sketch-circular-pattern-click-commit: [CP, CP, H]).
  - "a sketch linear pattern draws live's direction line …" (a9bc748a): PatternLink holds circles' radii (no Equal);
    the tool adds live's direction line (construction, seed -> first instance, both ends Coincident, H / V on an
    axis, Length = spacing) into the first instance group, so an edit replaces it. T1
    sketch_mode::tests::a_linear_pattern_draws_its_direction_line_as_live.
  - Land failed 19:09 on parity::lives_sketch_patterns_are_linked (it expected the old Equal: now (4, 0, 3)); fixed
    in the worktree, not committed yet.
- Uncommitted with it: the translator maps a DISTANCE to a zero-length line (a rectangle side two sketch fillets used
  up, c2-fan-blade-sweep's Sketch 2) to that line's point (T1 parity::a_distance_to_a_line_of_zero_length_is_to_its_point,
  passes). Its Parallel / Tangent stay unmapped (no direction on a point).
- Next step: the whole ops test binary on the Mac, then land.sh (the three commits + this handoff).

## Waiting on others
- live (REQUESTS.md): 14:58 refs capture for ~30 "query not mapped (no live geometry)" constraints in 16 documents
  (rerun them with ut3t.sh, count with dropped.py); 15:30 sketch Offset of a face / chains / preselection / model
  change (edges are built: 215050df); 16:27 sm-box "Face of … / Vertex of …" labels at each rollback (psui's
  tab-new-edit-reedit and bend-relief rows: ours keeps a rebuilt sheet metal face's earlier name, keep_names in
  crates/custom/src/custom/host/sheetmetal.rs); 18:44 End cap → Internal on frame-l's 8020 1001 end face (live's
  Faces field turns red, ours builds; std endcap.fs fails on loopsCount != 2 or later in generateProfileInternal).
- psui (4d0a82b2-8fc7-43e6-b45d-bd8cb225741e) did its part 19:00: the exporter lists each sketch pattern twice as
  live; a white-space-click accept ends the pattern tool; the gusset edge pick keeps the first drawn (Part 1).
- Not mine (handed): knife / impeller geometry (core); fan-blade Boolean 2, gamma / wire-clip Boolean 2, shaped-flange
  Booleans, surface-pattern sphere cuts (bool); setback fillet (blend); hex-bookcase mesh Uses (dx).

## Open questions (none blocking)
- s236 (fuzz probes 11): live solves three linked quads at once; per-case placement rules don't converge. Decision
  for the lead if fuzz work resumes there. s228 has its rule (grow_slot); s226 is 3 µm off (live's anchor drifts).
- Jog at its reduced angle: our flat lays two bend strips 0.73 mm over each other (features_c
  jog_preserve_material_off_and_errors' second case; live's doesn't): the jog's definition has two middle faces 0.79
  mm apart, so coplanar_rip doesn't join them.
- Sheet metal "Collision in sheet metal flat pattern." INFO for zero-gap contact at a tear (config-assembly 08590a's
  Flange 1 … Mirror 1): needs a contact rule, not an area check.

## Next, in order
1. Land the pattern fix + zero-length distance (above).
2. When live's 14:58 refs land: rerun those 16 documents, map what's now named, report the dropped count (83 at
   16:10, external OFFSET 10 -> 0).
3. Offset of a face (15:30 capture): op + app + translator, e2e, userdocs check.
4. sm-box labels (16:27 capture): the naming rule in keep_names.
5. End cap Internal (18:44 capture).
6. Knife's own sketch rows: Sketch 12's 5 Uses of blade B-splines, Sketch 17's Concentric to a spline edge, Ic / Id
   (Sketch 6's later entities); PIERCE (tried and reverted 12:53: knife Sketch 13 moved 11.8 mm).
7. Fuzz P2 random cases (769 of 830, live/fuzz/skasm-sketch-probe-*.json).

## Current numbers
- Model suite 16:10 vibe6 (suite-1610.log): 76/87, 881/947 (the 4 Hole documents lost were studios' 8aea0c3c4, fixed
  253c08ee: 80/87 expected). Sketch constraints left to the solved geometry: 83.
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
