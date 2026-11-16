# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## State (2026-11-16 00:50 UTC)

### In flight
- Worktree /root/projects/noshape-wt/asm (detached on 93346d03 = landed fd98e8a6), on top (none landed):
  - 60b13cba (**decision with the lead**): `regions_by_sum` takes the closest group of live's sketch regions.
  - 70ae7e9b in-context bodies (context.jsonl); 154e6c65 named views no undo step; 08d6feee configured refs;
    07f3351a + 0c676223 + fd230597 BOM scoring; cde9262e live-defect mates; 40205609 in-context POINT bodies;
    668e761a mate status from mate-messages.json.
  - f4159c6d Derived's picked source sketch + its regions; empty foreignId -> blobData.
  - 913fad5f sketch instances (type "Feature") inserted.
  - 64f53b0b user-set part properties from live's metadata.json (propertySource 3) incl. material (custom: density).
  - e62c08ba refs for sub-feature parameters (`subFeature:<id>.<param>`); Derived `location` / `placement` (index
    -1 -> AT_ORIGIN); the suite loads sketch-only studios; user-set Name; NOSHAPE_PARITY_TRACE "TRACE use" lines.
- Measured (single corpora, vibe7 cfg/): keyboard-pattern s52 25/25 occurrences, 16/16 BOM, mates 1/4 (3 mates on
  sketch instances, see below); clamp-pattern BOM 2 -> 13/14; gearbox-std 2 -> 8/8; hand-drill-pinslot 0 -> 13/20
  (s50; + names in e62c08ba); config-assembly 25/25, 18/18, 12/12; robotic-arm s52 32/32, 39/40 (Fastened 2: live2's
  file says ERROR with no message, asked live2 00:5x; 07d06e Derived fixed in s55, run pending).
- Gate (vibe6 g49/run.sh, slot): base29 done: 78/104 documents, 981/1194 features (g49/models-base29.log). Then
  ops-s49 (= s55, all commits) models, then the full assembly suite with it (full/s49.log). Then compare, land, SCORE.
- vibe7 must be idle 01:30-04:05 (lead): guard0125.sh stops my runs there at 01:25; builds go to vibe6 then.
- Next for keyboard: mates on sketch instances (connectors on a sketch's wire-body edges; ours resolves live's
  sketch-edge query in the Part Studio, which has no such edge: map the sketch entity to the instance's wire edge,
  e.g. assembly_entities `at` live's point after placing). a66ca5's context.jsonl landed (00:4x).
- Tried and reverted (s48): keeping a sketch moved by a configuration variable's dimension (20e932).
- dx: corexy's STEP names ("=>[...]" link names, io.cpp) in progress; rescore corexy's BOM when landed. dx's
  e541cc55 (EDGE / VERTEX queries aren't regions): tell dx if an asm-* studio moves.
- keyboard 7f71cc's Mate connector 1 owned by a sketch's wire body: live accepts, ours resolve_bodies has no sketch
  bodies (refgeom.rs, model-core); one feature status.

### Queued to land
- Nothing. Landed today: 790ecad5, 36ebccbc, 163918c9, fd98e8a6, 8ffdb51e / 72805bc5 (this file).

### Decision with the lead
- 60b13cba: isaac-ball 113 -> 116/121 occurrences but as-live mates 97/103 -> 89/121 (old data). The full run base29
  vs s37 shows it with the rest.

### Next, in order
1. Read full/base29 vs full/s41 per assembly (occurrences, mates; BOM rows for s41); land the commits (60b13cba per the
   lead); a SCORE.md "Assemblies" row (the last is 11-08).
2. BOM: standard content's material (insert standard-content instances as our catalogue's, or set live's material);
   then the rows that differ most in the full run.
3. a2e4a2's Extrude 5: 2 extra edges (ours splits a 0.451 mm arc at a vertex of JUm's edges and adds a 13 mm line);
   live's JUm / Sketch 5 / Extrude 5 edges are in studios/a2e4a2….jum-sketch5.json (live 20:0x).
4. isaac-ball 1e81ca / 90f78b (GearLab): core's loft 55e517e0, then the opFillet (blend's). 3c5c58 Fillet 1: blend has a
   dump (vibe7 ~/tmp/blend/m3cd), third in their queue.
5. config-assembly 20e932: Revolve 1 / 2 still "do not intersect" (configured dimensions taken back by the translator).
6. live's mate messages (isaac-ball's 7 ERROR, infento's 23) when they land: compare messages, not only statuses.

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
