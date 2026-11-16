# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## State (2026-11-16 03:30 UTC)

### In flight
- Worktree /root/projects/noshape-wt/asm: my 25 commits **rebased onto main a2952b32** (HEAD 31ba7ca6; the pre-rebase
  head was 4fe1d880). dx's `__liveConfiguration` constructor change conflicted with each commit adding a Translator
  field (resolved: dx's loop + my field list; a leftover TRACE-use conflict kept main's line). None landed:
  - 60b13cba (**decision with the lead**): `regions_by_sum` takes the closest group of live's sketch regions.
  - in-context bodies; named views no undo step; configured refs; BOM scoring; live-defect mates; in-context POINT
    bodies; mate status from mate-messages.json; Derived's picked sketch + regions, empty foreignId; sketch instances;
    user-set part properties (metadata.json); sub-feature refs, Derived location / placement; connectors on sketch
    instances, refsx FACE -> sketch region; vertex-only in-context bodies as points, no metadata for standard content;
    found:false refs lines skipped; second-id connector edge along live's Z; live's wire parts out of part matching
    (hand-drill's "Curve 1"); a sub-feature's query by its text first (hand-drill 9fbc05's second Spur gear); an
    instance with partId "" a live defect; standard content named with its thread length found in our catalogue;
    **model-core refgeom.rs**: a Mate connector owned by a sketch -> the sketch's body (core told); traces for
    untranslated mate ends; **model-data / assembly**: a partly threaded screw named "… x L x b" (live 02:50).
- Since the rebase: 83e390d7 a connector on a patterned instance read without the pattern's id live puts first in its
  path (pattern-std 24, scissor-lift 13, bjr-config 5, keyboard 2 such paths); 591fdd87 an Origin connector a mate
  holds built as for an Origin Mate connector feature (keyboard's Fastened 1); f87eaa46 a Derived mirror's source
  studio built and mirrored (corexy fc2fa8 / 9e085d).
- Binaries (~/agents/asm-keep/bin on vibe5/6/7): s70 = HEAD f87eaa46, base30 = main a2952b32 (same base, for A/Bs of
  a corpus that moves). Full assembly run: **vibe7** ~/agents/asm-keep/full7.sh (s70 over ~/parity/models) queued in
  ~/bin/slot behind studios' shards since 04:07; log full/s70v7.log, "ASM-DONE" in full7.status. (vibe6's g49 asm run
  was cancelled before it started: it set no NOSHAPE_PARITY_ASSEMBLIES, and the default /root/parity/models is
  unreadable there.) hd.sh (hand-drill + keyboard, cfg/s63-*.log, file ops-s63 = s70) still queued on vibe6 behind qa.
- Then: compare full/s70v7.log to full/base29.sum (cmp.py) per corpus (base29 = old refs: hand-drill 67/67, corexy 11/11, pattern-std
  31/40, robotic-arm 40/40, tvc-mount 5/11 mates); land (rebase onto main: dx's 19afd619 STEP names -> rescore corexy
  BOM; dx's planeOp-before-refs change, which I dropped from mine); SCORE.md "Assemblies" row (last 11-08).
- Single corpora: keyboard s59 25/25, 1/2 mates, 16/16 BOM; pattern-std s59 161/162, 67/76 mates, 11/17 BOM;
  hand-drill s57 40/41, 50/67 (fixed since), 17/20 BOM; tvc-mount s57 41/43, 6/11; corexy s57 56/62, 7/7.
- Asked live / live2 (REQUESTS 02:2x-02:4x): pattern-std 36b90f sketch refs (done, 622 lines: PROJECTED unused 27 ->
  10, several sketches then try metre moves and are taken back); scissor-lift b7e3814f lines / mass (done); ISO 4762 /
  DIN 912 thread lengths and names (P2, live's queue; our rule gives 30 at M12x60, live 36, names "… x 60 x 36").
- pattern-std's 9 Fastened mates on AR-S1 / AR-S2 (studio 36b90f, parts off live's volume) were in base29 too.
- keyboard: 6 mates "connectors not translated" (sketch instance ends, pattern-instance paths?): d171c160's trace
  will tell in the next keyboard run.
- vibe7 idle 01:30-04:05 (lead); my runs and builds on vibe6.
- keyboard left: sketch-instance mates (connectors on the instance's wire edges), 7f71cc / a66ca5 Mate connector
  owned by a sketch (refgeom.rs owner check, model-core).
- Tried and reverted (s48): keeping a sketch moved by a configuration variable's dimension (20e932).
- robotic-arm Fastened 2: live2's mate-messages says ERROR without a message (asked live2).

### Queued to land
- Nothing. Landed today: 790ecad5, 36ebccbc, 163918c9, fd98e8a6, 8ffdb51e / 72805bc5 (this file).

### Decision with the lead
- 60b13cba: isaac-ball 113 -> 116/121 occurrences but as-live mates 97/103 -> 89/121 (old data). The full run base29
  vs s37 shows it with the rest.

### Next, in order
1. Full run (s70) vs full/base29.sum per assembly (`cmp.py`); a corpus that drops: A/B it alone with base30 (same
   main, same data) to tell code from data. Land (60b13cba per the lead's decision); SCORE.md "Assemblies" row (the
   last is 11-08); rescore corexy's BOM (dx's STEP names are in the rebase).
2. Standard content Thread length as live (live's q-stdthread, behaviours.md "Standard content: Thread length…"): ISO
   4762 / DIN 912 b = 2d+12 (ours ISO 888's 2d+6), ISO 4762 defaults to b, DIN 912 to L; the full per size × length
   table asked of live (P2, 03:0x) goes into Size.threads as data.
3. keyboard: Fastened 2 / Slider 2 / Slider 4 on sketch instances: read hd.sh's s63-asm-keyboard-pattern.log traces
   ("sketch-at … nearest", "end … path / entity").
4. scissor-lift fed22d Mirror 1: dx's planeOp-before-refs change (dx's queue) fixes it; then fed22d's next difference.
5. welding-config 9bdeb5 Extrude 4 (Part 5 7/15/10 faces vs live 6/12/8); its Sketch 4's Uses were trimmed in live:
   ours already draws live's piece (a free curve: our Split drops a Use's link, sketch editor, not asked yet).
6. bjr-config (dx's): side triangle connectors (5d8bc1) and a subassembly instance id missing from the instance lists.
7. suspension 805e29 Extrude 1 (Knuckle 707632 vs live 406039 mm³): ours 8 regions (the centre disc "0,23" twice), live
   6 (qSketchRegion(…, true) JGO JGS JGW JGa JGi + JGe the disc; JGi a ring whose point is the centre); no region_at
   trace. Cause (s71 trace): live's region lines come from refs.jsonl (refs.js, bodyType SKETCH), not the sketches
   file, so `query()`'s area check (live_area needs `sketch_file`) is skipped and our qSketchRegion(…, true) text wins.
   Fix after this land: take refs.js SKETCH_REGION areas too; needs a model gate (10 model documents have such lines:
   c2-config-parts-*, c2-loft-guide-curves-*).
8. robot-relations a4e419 (Fixed Side 2873 vs 61001 mm³): live2's refs (02:2x, 41 lines) once synced; Extrude 2's
   booleanScope names a body of its Derived by the source's feature.
9. config-assembly 20e932: Revolve 1 / 2 still "do not intersect" (configured dimensions taken back by the translator).

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
