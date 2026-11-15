# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## State (2026-11-15 19:41 UTC)

### In flight
- Worktree /root/projects/noshape-wt/asm (detached on 93346d03 = landed fd98e8a6), on top:
  - 60b13cba (not landed; **decision with the lead**, below): `regions_by_sum` takes the closest group of live's
    sketch regions (a sliver live splits off).
  - 70ae7e9b: in-context entities from live's `<element>.context.jsonl` (live, all assembly corpora: isaac-ball 21
    studios, corexy 2, keyboard-pattern 2, scissor-lift 2, suspension 2). `context_bodies` imports each context body:
    a planar sheet of one face as that face (`context_face`: edges chained into loops; lines, circles, arcs by `mid`,
    splines / ellipses through live's 10 `points`), any other body (wires, curved or multi-face sheets) as its 3D
    edges (`context_wire`); point bodies are skipped. A non-sketch feature's sub-feature maps to its first Import (an
    Extrude of the face: 1b88ff's Extrude 3 / 4). An in-context sketch sits on live's planar face when there is one
    (its edges split the sketch's regions as live's: with the 1 m square instead a2e4a2's Extrude 1 got 13 faces, not
    12), else on the square on live's sketch frame, live's other bodies besides. Loaders: features["contextEntities"]
    (studios/<element>.context.jsonl; a model dir's context.jsonl). Tests: T1
    `parity::an_extrude_of_an_in_context_face_extrudes_lives_face`, unit `parity::tests::an_in_context_face_is_rebuilt_from_lives_edges`
    (both pass on vibe6; Mac run going).
  - One-model results (ops-s32): 1b88ff 6 -> 12/12 features (matches to the end); a2e4a2 7 -> 9/64 (first difference
    now Extrude 5: 32/74/42 against live's 32/72/40, same faces, 2 extra edges); 3c5c58 unchanged 9/33 (Fillet 1 sent
    to blend 19:2x with the repro m3c).
- Runs: vibe7 isaac-ball s30 (finishing), then s29b (s29 on the new data, the A/B base) and s32, all through slot
  (~/agents/asm-keep/s01/<build>/asm-isaac-ball.log). vibe6: model gate s30 (gate24/s30.log; for models s32 = s30:
  no model has context data), then ctx32.sh: corexy / keyboard-pattern / scissor-lift / suspension, s29 vs s32
  (~/agents/asm-keep/ctx/<build>/<corpus>.log).

### Queued to land
- Nothing. Landed today: 790ecad5, 36ebccbc, 163918c9 (config-assembly 13 -> 18/18 mates), fd98e8a6 (GearLab runs),
  8ffdb51e (this file restructured).

### Decision with the lead
- 60b13cba: isaac-ball 113 -> 116/121 occurrences but as-live mates 97/103 -> 89/121: a2e4a2's part now builds, its
  later in-context sketches land off, so their mates fail and others overdefine. Model gate clean (s28 = s29 per
  model). My recommendation: hold until a2e4a2's later sketches are right; s32 (with the context bodies) re-measures it.

### Next, in order
1. Read the isaac-ball s29b vs s32 and the four corpora's s29 vs s32; the model gate (base24/s29 vs s30, per model).
2. Land 70ae7e9b (and 60b13cba if the lead says so, else rebase it out); update this file.
3. a2e4a2's Extrude 5 (2 extra edges), then its later features; isaac-ball's mates on a2e4a2.
4. isaac-ball 1e81ca / 90f78b (GearLab): core's curved-face loft queued 55e517e0; then the opFillet on the toothed
   solid (blend's). 3c5c58 Fillet 1: blend.
5. config-assembly 20e932 (optional; 25/25 occurrences, 18/18 mates): Revolve 1 / 2 "tools and targets do not
   intersect": the studio is built at an instance's configuration, and Sketch 1 / 2's configured dimensions move the
   geometry (0.92 / 0.80 mm), which the translator takes back; its setData / custDesc Feature Studios aren't fetched.

### Others' items touching mine
- dx keeps infento (1dc32b, 74d3d6) and bjr-config (5d8bc1). dx: derivedFrom + ORIGINAL_DEPENDENCY (top-level EDGE)
  queries resolve to faces / sketch regions in the translator; dx covered 1dc32b's Chamfer 4 / Fillet 5 with live's
  refs, not a general fix: tell dx before touching that path.
- core: 1dc32b's Chamfer 3 (dx) brings back 14 infento occurrences; Hole 2 (2cc476fe) is core's.
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
