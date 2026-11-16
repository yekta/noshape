# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## From bool 11-16
- 21:06 UTC: 9d68c081 landed: a part's cylinder sectors on one axis merge where one sector's frame lies far along the
  axis (asm-pattern-std 36b90f Extrude 11: 16/40/26 -> live's 14/36/24 a part). Rerun pattern-std's suite pair; its 9
  Fastened on AR-S1 / AR-S2 may move with it.

## From core 11-16
- 13:52 UTC: your 57de87f07 (a region query's text checked against refs.js region lines' area too) turned
  c2-impeller-loft 26 -> 24/48: Extrude 4's two regions (JYS 12.03, JYW 53.29 mm²) have query text naming the whole
  region (imprinted segments dropped, our `whole` rule), whose area we can't measure, so the ids' fallback returned the
  two pieces the target's own imprint splits (keys >= 2^31); the 12 mm² sliver lies on the target's imprint and fails
  "up-to target does not fully bound". core's fix in crates/ops/src/parity.rs `query`: the ids' regions are not taken
  when they hold imprinted segments and the text names a whole region (impeller back to 26/48). Your area check is
  unchanged otherwise; rebase onto it and check your case still picks what you meant.
- 11-16 later: that rule turned c2-sm-rocker-enclosure-body 15 -> 9/28 (Extrude 3 wants its piece), so it is narrowed:
  the whole region is kept only when another of the feature's queries names the same whole region
  (`shares_whole_region`); impeller stays 26, rocker back to 15 (release pair: only rocker changed, 1190 -> 1196).

## State (2026-11-16 06:05 UTC)

### Landed today
- 57de87f0 (06:01, 33 commits): connectors on patterned instances / Origin connectors in mates; Derived mirror;
  refs.js regions read with the sketches file's area (assemblies and, via `add_sketch_refs`, models) and their area
  checked in `query()` too; in-context cylinder faces from two circles (corexy 771e5f Belt); standard content Thread
  length lists + defaults from live's q-stdthread (29 of 83 components; importer ~/agents/asm-keep/thr_import.py here,
  rerun when live has all 83; `threadDefaults`; the capture's remembered-value artefact corrected from neighbours);
  standard content found by any Thread length; refgeom.rs sketch-owned connectors; earlier 11-16 items.

### In flight
- fffe186b (closest-sum region match) **out** of the land (lead 06:0x): lands only if a per-mate diff (main vs it,
  same session, fresh data) loses no mate main has as live. Branch asm-with-fffe186b; binaries vibe6 bin/ops-s76
  (57de87f0) and ops-s77 (+fffe186b); q79 runs both on snapshot ~/tmp/asm/isaac-0603 in one slot hold
  (cfg/t79-s7[67]-isaac.log). Compare the not-as-live mate lists; land it (with the list to the lead) or fix lost mates.
- 3c5c58 sliver (lead asked): ours has it. As a model (vibe6 mi/m3c, ops-s76) Extrude 1's text gives our 1487.0 +
  0.141 mm² regions = live's 1487.1; base29's error was area matching on old data. Confirm in t79-s76 (no "3c5c58 ›
  Extrude 1").
- vibe7 full run s70 (with fffe186b, started 04:27; full/s70v7.log; 10 corpora by 05:49) -> cmp.py vs base29.sum.
  So far all >= base29 except hand-drill mates 67 -> 51/67, which base30 (main) shows too (q73): main or data, not my
  stack. q75 (base30 + 4ece4b's pre-11-16 refs) and q76 (base29 on today's data) tell which.
- tvc-mount 41/43 (base29 43/43: sub ba3cc3 avionics bay 138 mm off; BOM "Rough PCB" for root Motor Mount): q74
  (base30 vs s70 on snapshot tvc-0444) tells code from data.
- corexy on s73 (Belt fix): q77. Region-area gate (30 model documents with refs.js region lines, s73 vs s75): q78
  (cfg/g78-*.log); it's landed already: a document worse -> fix forward.
- vibe6 slot order (first come): skasm models, q74, blend, q79, then q75-q78.

### Queued to land
- Nothing.

### Mates lost, ranked (full run s70, done 06:4x; lost = not as live + "its connectors not translated")
- scissor-lift 49 (47 untranslated: all mate-held implicit connectors, BTMFeatureQueryWithOccurrence -> the mate's
  own mateConnectors; per-end cause from q81's trace), infento 43 (40 untranslated, q81 too), isaac 41 (16
  overdefined, 8 unresolved, 7 inconsistent, 10 untranslated; fffe186b A/B q79), hand-drill 17 (16 overdefined: dx's
  b4ef875a6 planeOp-before-refs fixes the 4ece4b Plane 3 -> Plane 2 chain, in dx's gate), pattern-std 11, corexy 11
  (Belt parts: q77), tvc 5, welding 5, xdev 5, config-assembly 4, keyboard 4, pinslot 3, suspension 2.
- "Failed to resolve the instance for mate." (robotic-arm Fastened 2 etc.): red in live, counted as a live defect
  (ruling 11-15): costs nothing.
- Full run totals (s70 = stack + fffe186b, before thread/cylinder/region commits): occurrences 1604/1709, mates
  669/737, BOM 521/612. Drops vs base29 to explain: pattern-std 161->160, robot-relations 139->138, scissor-lift
  215->212, tvc 43->41, welding 81->76 occurrences: q80 (vibe7, base30 vs s76 on snapshot drop-0642,
  full/q80-*.log) tells mine from main / data.

### Next, in order
1. q79 (vibe6) per-mate diff -> fffe186b decision; q80 (vibe7) drops; q81 (vibe5 q5.status) traced scissor-lift +
   infento -> fix the untranslated connectors (biggest). Land each fix as ready.
2. SCORE.md "Assemblies" row (last 11-08) once q80 says which drops are mine.
3. q78 region gate (vibe6): no model document worse. hand-drill: confirm after dx's b4ef875a6.
4. Thread lengths: rerun thr_import.py when live has all 83 components.
5. bjr-config (dx's 5d8bc1); pattern-std's 9 Fastened on AR-S1 / AR-S2 (36b90f parts off live's volume).
- Done: robot-relations a4e419 matches as a model (4/4, mr/ma4 on vibe5/6/7 and Mac ~/noshape-host/asm-models/mr;
  blend has its Fillet 1 time); welding 9bdeb5 Extrude 4 (core).

### Others' items touching mine
- dx keeps infento (1dc32b, 74d3d6) and bjr-config (5d8bc1). dx: derivedFrom + ORIGINAL_DEPENDENCY (top-level EDGE)
  queries resolve to faces / sketch regions in the translator; dx covered 1dc32b's Chamfer 4 / Fillet 5 with live's
  refs, not a general fix: tell dx before touching that path.
- core: 1dc32b's Chamfer 3 (dx) brings back 14 infento occurrences; Hole 2 (2cc476fe) is core's.
- blend (20:3x): 3c5c58 Fillet 1's input matches live; OCCT fails on the 14 edges together, the by-groups fallback splits
  two rounds; third in blend's queue.
- dx (19:3x): assembly studios take the Feature Studios in studios/<element>.fs their sources import (bjr-config
  5d8bc1); lands near my contextEntities loader lines in crates/ops/tests/parity.rs: whoever lands second rebases.
- psui FYI (11-12): saving a named view is no Undo step in live; the assembly's `self.edit("Save view : …")`
  (assembly_view.rs) still makes one. Not done yet.
- Lead ruling 11-15: don't copy live's unresolved-connector failure (DECISIONS.md).

## Scores (last measured)
- Assembly suite, same-base A/B (vibe7 ~/agents/asm-keep/s01): isaac-ball base29 113/121 occurrences, 97/103 mates
  as live; config-assembly 18/18 mates; suspension 6/6 mates; infento-subs 144/167 occurrences, 176/182 mates (61 -> 3
  parts our mates move off live's place); corexy, xdev-subs, clamp-pattern, robotic-arm, bjr-config,
  hand-drill-pinslot, keyboard-pattern unchanged by 36ebccbc.
- Model gate (all models, ops-s28 / s29): 76/87 documents, 881/947 features, custom 103/107 at base24; 80/87 and
  895/947 at s29 (the gain is main's hole fixes via the rebase).
- asm-* ui-parity: 213/214 (only live's crashed asm-cylindrical-limits take).

## How to run
- Builds and crate tests: on the Mac, `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p noshape-ops --test all
  parity::<name>` (one slot; rb.sh waits). No suites, ui-parity or app runs on the Mac.
- Suite binaries are Linux builds made on a vibe host: `NOSHAPE_RB_HOST=vibe6 tools/tiers/rb.sh bash -c
  'CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo test -p noshape-ops --test all --no-run >
  ~/agents/asm-keep/build-sNN.log 2>&1; f=$(ls -t ~/agents/asm/target/debug/deps/all-* | grep -v "\.d$" | head -1);
  cp $f ~/agents/asm-keep/bin/ops-sNN'`; copy between vibe hosts with `zstd -c | ssh … zstd -d`. Never cut cargo's
  output with `head` (SIGPIPE stops the build): log to a file, grep after.
- Heavy runs (assembly / model suites, gates) through `~/bin/slot <cmd>` on vibe6 / vibe7 (one per host, waits for
  10 GB free), never vibe3. Long runs started over ssh with `setsid nohup bash script.sh > /dev/null 2>&1 < /dev/null
  &` (systemd user units get oom_score_adj 100+ and earlyoom kills them first; `EXIT 143` = earlyoom, see
  `journalctl -u earlyoom`).
- One assembly: scripts in vibe7 ~/agents/asm-keep (isaac29.sh, cfg29.sh): `NOSHAPE_PARITY_TRACE=1
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
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe5 NOSHAPE_LAND_HOSTS="vibe5 vibe3" tools/tiers/land.sh`; `--status`.
- Live data: ask in /root/onshape-research/live/REQUESTS.md (times from `date -u`) and message live
  (b89af730-e8c6-4234-b83e-00a9e1ba6acc). live's scripts: asm/q-isaac/refs.js (refs at a sketch's rollback,
  USE_SOURCE=1), fsfetch.js (imported Feature Studios).
- ui-parity asm-*: `tools/ui-parity/run.py --app <noshape> --display :105 <names>` on vibe6/vibe7 (≤ 4 apps).

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

## Lead 11-16 14:23 UTC
The lead stopped your q91 (b84 vs s89, full-0910) and the queued hd-oldrefs trace on vibe6 to free the slot for blend while you are paused. Rerun the suites you need after rebasing onto main (core landed 55552e704 in your translator area, see "From core 11-16").

## Lead 11-16 16:05 UTC
Also stopped on vibe6: the q76/q77/q78 waiter chain (q76 = hd-now with ops-base29 had taken the slot at 16:01). Requeue what you still need after rebasing.
