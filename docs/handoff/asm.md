# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## State (2026-11-16 04:47 UTC)

### In flight
- Worktree /root/projects/noshape-wt/asm: 31 commits on main a2952b32, none landed (HEAD 0653696f). Since the 03:30
  rebase: 83e390d7 patterned-instance connector paths without the pattern id; 591fdd87 Origin connectors held in
  mates; f87eaa46 Derived mirror; 0474310b region-area trace; df5f994b refs.js SKETCH_REGION lines read with the
  sketches file's area (suspension 805e29 Knuckle); 0653696f **Thread length lists + defaults from live's q-stdthread
  table** (29 of 83 components so far; importer ~/agents/asm-keep/thr_import.py on this machine, rerun when live has
  all 83; `threadDefaults` where live's default isn't the first option; the capture's remembered-value artefact
  (ISO 4762 x 100, ANSI 7/8-9 x 10) corrected from the same size's neighbours).
- 60b13cba (`regions_by_sum`) stays **with the lead** (isaac-ball occurrences 113 -> 116/121, mates 97/103 -> 89/121).
- Binaries (~/agents/asm-keep/bin on vibe5/6/7): s70 = f87eaa46, base30 = main a2952b32 (A/B base).
- **vibe7 full run** (s70, full7.sh, started 04:27, log full/s70v7.log, "ASM-DONE" in full7.status). It read the live
  tree (before the snapshot ruling); no asm-* file changed after 04:27; snapshot ~/tmp/asm/models-s70v7 kept for
  A/Bs. full7.sh now snapshots at start. First 7 corpora all >= base29 (bjr-config mates 10/21 -> 31/31,
  config-assembly 21 -> 25/25 occurrences, corexy 58 -> 60/62).
- vibe6: q73 (base30 hand-drill trace, cfg/b30-asm-hand-drill-pinslot.log) then q74 (base30 and s70 on tvc-mount,
  snapshot ~/tmp/asm/tvc-0444, cfg/t74-*-tvc.log). Status lines in cfg/q6.status.
- Single corpora at s70/s72: keyboard 25/25, 4/4 mates, 16/16 BOM; suspension 27/30, 8/8, 21/21; hand-drill 40/40,
  51/67 mates (base29 67/67: 4ece4b Sweep 2 "Select entities for sweep path.", Helix 2 "made no single curve"; refs
  identical, so code: q73 compares); tvc-mount 41/43 (base29 43/43): sub ba3cc3 (avionics bay: Part 2, Rough PCB)
  placed 138 mm off, BOM top row "Rough PCB" where live has "Motor Mount" (7bc9d8 JHD; part matching is right) - q74
  tells code from data (41b795 / 7bc9d8 metadata.json are new since base29).

### Queued to land
- Nothing. Land after the full run: rebase, land.sh.

### Decision with the lead
- 60b13cba (above).

### Next, in order
1. Full run vs full/base29.sum (`cmp.py`, on vibe7 too); corpora that drop: A/B with base30 on a snapshot. Fix
   hand-drill (q73) and tvc-mount (q74). Land; SCORE.md "Assemblies" row (last 11-08); rescore corexy's BOM.
2. Thread lengths: rerun thr_import.py when live has all 83 components (live asked to clear the remembered value).
3. keyboard: done at s63 (4/4 mates).
4. welding-config 9bdeb5 Extrude 4 (Part 5 7/15/10 faces vs live 6/12/8): ours draws live's trimmed piece; cause open.
5. refs.js SKETCH_REGION areas in query() for models too: needs a model gate (10 model documents have such lines:
   c2-config-parts-*, c2-loft-guide-curves-*).
6. robot-relations a4e419 (Fixed Side 2873 vs 61001 mm³) with live2's refs; Extrude 2's booleanScope names a body of
   its Derived by the source's feature.
7. config-assembly 20e932: Revolve 1 / 2 "do not intersect" (configured dimensions taken back by the translator).
8. bjr-config (dx's 5d8bc1); pattern-std's 9 Fastened on AR-S1 / AR-S2 (36b90f parts off live's volume, also base29).

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
