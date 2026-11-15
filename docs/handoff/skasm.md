# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-15 22:25 UTC. Read docs/DECISIONS.md first; history is in docs/handoff/archive/skasm.md (read it
only when a line here points there).

## In flight
- Landing 22:2x: "a sheet metal face, edge or vertex is labelled after the feature that first made its definition
  entity" (each custom feature's transient base in Context.fs_transient_bases; CtxState::transient_feature decodes an
  association attributeId, "{ queryType : TRANSIENT , transientId : JAB }"; SheetMetalData.label_owners; the
  part-studio-ui labeler reads it) and "a second flange beside the first ends the first's bend where the plate does"
  (host sheetmetal.rs: no skew end when both free sides are square to the bend; sm-box now 23 faces as live; all 70
  sheet metal T2 tests and the 8 sheet metal suite documents match). T2 tests in sheetmetal_std.rs.
- Parked (patch /root/skasm-scratch/wip-8020.patch + /root/skasm-scratch/frame_library_8020.json): exact 8020 sections
  from live2's skasm-q1956 (108 profiles, converter /root/skasm-scratch/bin/f8020.py -> frame_library_8020.json,
  loader `exact_8020` in frame_library.rs; every listed size builds). Waits on arc midpoints (REQUESTS 21:46): 1001's
  18 semicircles can't be oriented from ends / centre / length (332.015 vs live 329.364 mm²). Then: regenerate the
  JSON with the midpoints, check every profile's area against live's (f8020.py prints AREA lines), update
  frame_library tests (1001 area 329.364444), drop `tslot`, land.

## Waiting on others
- live2: arc midpoints (21:46); identity tracking through a sheet boolean (22:02: live labels bend 1 and the corner
  vertices (60, -2, 4) / (60, 0, 4) after Flange 1, ours after Sheet metal model 1 because our tracking follows the
  plate edge to the merged bend edge, so std's assignSMAttributesToNewOrSplitEntities keeps its old association).
- psui (4d0a82b2-8fc7-43e6-b45d-bd8cb225741e): told 20:3x the tab row is a pick difference (live hit Flange 1's wall).
- dx: cf-spur-gear's per-host Sketch 2 failure = the near-singular sketch 62893b1ed takes back (told 21:3x).
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
1. The tracking answer (22:02): if Onshape's tracking drops a merged edge, make ours do the same (custom host query.rs
   tracking), check the 8 sheet metal suite documents + T2s, and restore the corner-vertex asserts in
   sheetmetal_std::sheet_metal_entities_are_labelled_after_their_definition_entitys_feature.
2. 8020 sections when the midpoints land (above).
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
