# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-16 01:57 UTC. Read docs/DECISIONS.md first; history is in docs/handoff/archive/skasm.md (read it
only when a line here points there).

## In flight
- Landing 2026-11-16 01:57: a sheet metal entity is labelled after the feature that made its definition entity as
  it is now (SheetMetalData.def_owners: a definition entity keeps its owner while its name stands, else the current
  feature takes it; live skasm-q2202: identity tracking gives 0 for the plate edge Flange 1's boolean joins, its
  association attribute is kept, but it is a new entity JVB of Flange 1's). Replaces decoding association attribute
  ids (transient_feature / Context.fs_transient_bases removed). sheetmetal_std's labels T2 now asserts bend 1 and the
  corners (60, -2, 4) / (60, 0, 4) read Flange 1; 86 sheet metal tests pass.
- Landing 2026-11-16 01:3x: Use / measure / pierce / planes take another sketch's spline, ellipse and conic
  (measure.rs resolve_shapes via helpers_a::curve_segment; was lines and arcs only, "That geometry can't be used in
  this sketch"), and the translator lands a spline Use by its first / last points. Tests: e2e
  use_takes_another_sketchs_spline_ellipse_and_conic, parity a_used_spline_is_used. Userdocs (hats, dock) vs 0.4.52
  ALL SAME; suite-2354 (vibe6, 111 documents now): the 87 shared with 23:11 identical, dropped there 77 -> 74 (PIERCE).
- Landed 23:57 (28840f15): Pierce takes another sketch's curve, the crossing found on the curve (bisected, not a
  tessellation chord); the translator reads PIERCE: knife Sketch 13 kept whole (was taken back, 11.8 mm).
- Landed 23:25 (2e81e8d9): Frame's 8020 profiles are live's sections (frame_library_8020.json from live2's
  skasm-q1956 with each arc's mid2d via /root/skasm-scratch/bin/f8020.py, outer loop = largest area; all 108 areas
  match live's; `tslot` dropped; preview_svg draws Arc3). 1001 329.364 mm2; End cap Internal refuses it as live.
- Landed 23:08 (02a08d60): a Concentric to a co-edge whose edge isn't round takes its face (knife Sketch 17).
- Knife Sketch 12 stays 5 PROJECTED: our blade edges end 0.03-0.04 mm from live's ((-5.0356, -12.1564) vs
  (-5.0442, -12.1140) mm): the model geometry before it differs (feature not traced), not the translator.

## Waiting on others
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
1. Dropped sketch constraints (74 over the 87 documents of 2026-11-15 23:11), largest documents first (lead
   2026-11-16): fan-blade-sweep (13, "its sketch entity not translated": rectangles' .bottom/.top sides, fillet arcs'
   visualSharp points), hex-bookcase-derived (21: 9 clip COINCIDENTs taken back as over-defining; its mesh Uses are
   dx's), knife 8, valve-config 7, duct-multipart 6, impeller-loft 5. Trace running: vibe6 unit skasm-fan-0148
   (/root/skasm-scratch/fan-hex.log).
2. Fuzz closed (DECISIONS 11-16): live_fuzz::live_matches now fails on any counted difference; 801 of 801 counted
   match. Not counted: 29 live defects (23 live can't solve; 6 with redundant constraints where live calls
   under-defined curves fully defined: s5 s6 s8 s10 s11 s14) and 29 cases whose under-defined geometry settles
   elsewhere (FUZZ_SETTLES=1 lists them). "under?" in live's capture (too faint, no black / red) reads as under.
   write_batch fails on the Mac only (aarch64 float digits in the generator check).

## Current numbers
- Model suite 2026-11-16 00:01 vibe6 (suite-2354.log): 80/111 documents, 1019/1292 features (24 documents new to the
  set). Sketch constraints left to the solved geometry: 586 over all, 74 over the 87 of 2026-11-15 (16:10: 83).
- Sheet metal speed, quiet vibe6 19:00, ops binary of 17:43 (ours vs live ms): corner-break 1637-1782 / 1972,
  shaped-flange 809 / 1026, overcut-dogbone 210 / 270, hem 65 / 95. All four are faster than live; the suite's slower
  figures were host load.
- Exchange: model-plus STEP / IGES import in live as 58/151/94 (6f5b151e snap_coplanar); dx "live imports ours" 43/48.
- Sketch fuzz vs live: 801 of 801 counted (29 live defects, 29 known settles, of 830); probes 20 11/11.

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
